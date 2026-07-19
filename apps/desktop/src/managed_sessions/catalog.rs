use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use antennabench_core::BundleDiagnosticSeverity;
use antennabench_storage::{BundleStore, BundleStoreError};
use sha2::{Digest, Sha256};

use crate::open_session::{SessionErrorKind, SessionErrorPayload};

use super::{
    is_supported_bundle_name, CatalogDiagnostic, CatalogStatus, ManagedCatalogEntry,
    ManagedEntryOrigin, ManagedEntryStatus, ManagedSessionCatalog, ManagedSessionProblem,
    ProblemSeverity, MAX_CATALOG_CANDIDATES, MAX_ENTRY_PROBLEMS, MAX_PROBLEM_TEXT_BYTES,
};

#[derive(Debug)]
pub(super) struct CatalogBuild {
    pub(super) catalog: ManagedSessionCatalog,
    pub(super) records: Vec<LocatorRecord>,
}

#[derive(Debug, Clone)]
pub(super) struct LocatorRecord {
    pub(super) bundle_name: String,
    pub(super) identity: DirectoryIdentity,
    pub(super) fingerprint: Option<[u8; 32]>,
    pub(super) activatable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectoryIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    length: u64,
    modified_nanos: Option<u128>,
}

pub(super) fn build_catalog(root: &Path) -> Result<CatalogBuild, SessionErrorPayload> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "The AntennaBench sessions directory is not a safe local directory.",
                "the managed root cannot be a symlink or non-directory entry",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(CatalogBuild {
                catalog: ManagedSessionCatalog::complete(Vec::new()),
                records: Vec::new(),
            });
        }
        Err(error) => return Err(root_read_error(root, error)),
    }
    let read_dir = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => return Err(root_read_error(root, error)),
    };

    let mut candidates = Vec::new();
    let mut scan_failures = 0_u64;
    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                scan_failures += 1;
                continue;
            }
        };
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !is_supported_bundle_name(&name) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                candidates.push(Candidate::Unreadable {
                    name,
                    path: entry.path(),
                    message: format!("The directory entry could not be inspected: {error}"),
                });
                continue;
            }
        };
        if file_type.is_dir() {
            candidates.push(Candidate::Directory {
                name,
                path: entry.path(),
            });
        } else if file_type.is_symlink() {
            candidates.push(Candidate::Unsafe { name });
        }
    }
    candidates.sort_by(|left, right| left.name().cmp(right.name()));

    let observed = candidates.len();
    let was_truncated = observed > MAX_CATALOG_CANDIDATES;
    candidates.truncate(MAX_CATALOG_CANDIDATES);

    let mut projected = candidates
        .into_iter()
        .map(project_candidate)
        .collect::<Vec<_>>();
    projected.sort_by(|left, right| catalog_order(&left.entry, &right.entry));
    apply_same_id_warnings(&mut projected);

    let (entries, records): (Vec<_>, Vec<_>) = projected
        .into_iter()
        .map(|projected| (projected.entry, projected.record))
        .unzip();
    let mut catalog = ManagedSessionCatalog::complete(entries);
    if scan_failures > 0 {
        catalog.status = CatalogStatus::Incomplete;
        catalog.diagnostics.push(CatalogDiagnostic {
            code: "managed.directory_entry_unreadable".into(),
            message: format!(
                "The managed session catalog is incomplete: {scan_failures} directory entries could not be inspected."
            ),
            limit: None,
            observed: Some(scan_failures),
        });
    }
    if was_truncated {
        catalog.status = CatalogStatus::Incomplete;
        catalog.diagnostics.push(CatalogDiagnostic {
            code: "resource.desktop.managed_catalog_candidates".into(),
            message: format!(
                "The managed session catalog is incomplete: {observed} candidates exceeded the limit of {MAX_CATALOG_CANDIDATES}."
            ),
            limit: Some(MAX_CATALOG_CANDIDATES as u64),
            observed: Some(observed as u64),
        });
    }
    Ok(CatalogBuild { catalog, records })
}

pub(super) fn revalidate_available(
    root: &Path,
    record: &LocatorRecord,
) -> Result<PathBuf, SessionErrorPayload> {
    let path = revalidate_direct_child(root, record)?;
    if !record.activatable {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "This saved session is not available to open.",
            "the catalog entry is invalid, unsupported, or unreadable",
        ));
    }
    let current = inspect_directory(&record.bundle_name, &path);
    let Some(fingerprint) = current.record.fingerprint else {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Verification,
            "The saved session no longer matches its catalog entry.",
            "the current bundle could not be verified as available",
        ));
    };
    if record.fingerprint != Some(fingerprint) {
        return Err(stale_locator_error());
    }
    Ok(path)
}

pub(super) fn revalidate_direct_child(
    root: &Path,
    record: &LocatorRecord,
) -> Result<PathBuf, SessionErrorPayload> {
    if !is_plain_name(&record.bundle_name) || !is_supported_bundle_name(&record.bundle_name) {
        return Err(stale_locator_error());
    }
    let path = root.join(&record.bundle_name);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The saved session is no longer available at its managed location.",
            error.to_string(),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(stale_locator_error());
    }
    let identity = directory_identity(&metadata);
    if identity != record.identity {
        return Err(stale_locator_error());
    }
    Ok(path)
}

pub(super) fn record_created(
    root: &Path,
    path: &Path,
) -> Result<LocatorRecord, SessionErrorPayload> {
    let bundle_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| is_plain_name(name) && is_supported_bundle_name(name))
        .ok_or_else(stale_locator_error)?
        .to_string();
    if path.parent() != Some(root) {
        return Err(stale_locator_error());
    }
    let projected = inspect_directory(&bundle_name, path);
    if !projected.record.activatable {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Verification,
            "The new session could not be registered as a managed session.",
            "the created bundle did not pass managed catalog inspection",
        ));
    }
    Ok(projected.record)
}

fn project_candidate(candidate: Candidate) -> ProjectedEntry {
    match candidate {
        Candidate::Directory { name, path } => inspect_directory(&name, &path),
        Candidate::Unsafe { name } => ProjectedEntry {
            entry: ManagedCatalogEntry::problem(
                name,
                ManagedEntryStatus::Unsafe,
                "managed.unsafe_indirection",
                "This suffix-matching entry is a filesystem indirection and cannot be opened or revealed by AntennaBench.",
            ),
            record: LocatorRecord::unavailable(),
        },
        Candidate::Unreadable {
            name,
            path,
            message,
        } => {
            let record = fs::symlink_metadata(path)
                .ok()
                .filter(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
                .map_or_else(LocatorRecord::unavailable, |metadata| LocatorRecord {
                    bundle_name: name.clone(),
                    identity: directory_identity(&metadata),
                    fingerprint: None,
                    activatable: false,
                });
            ProjectedEntry {
                entry: ManagedCatalogEntry::problem(
                    name,
                    ManagedEntryStatus::Unreadable,
                    "managed.unreadable_entry",
                    &message,
                ),
                record,
            }
        }
    }
}

fn inspect_directory(name: &str, path: &Path) -> ProjectedEntry {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => metadata,
        Ok(_) => {
            return ProjectedEntry {
                entry: ManagedCatalogEntry::problem(
                    name.into(),
                    ManagedEntryStatus::Unsafe,
                    "managed.unsafe_indirection",
                    "The managed entry is not a direct directory.",
                ),
                record: LocatorRecord::unavailable(),
            }
        }
        Err(error) => {
            return ProjectedEntry {
                entry: ManagedCatalogEntry::problem(
                    name.into(),
                    ManagedEntryStatus::Unreadable,
                    "managed.unreadable_entry",
                    &error.to_string(),
                ),
                record: LocatorRecord::unavailable(),
            }
        }
    };
    let identity = directory_identity(&metadata);
    let store = BundleStore::new(path);
    let schema_version = match store.schema_version() {
        Ok(version) => Some(version),
        Err(error) => {
            return failed_inspection(name, identity, None, error);
        }
    };
    let inspection = match store.inspect() {
        Ok(inspection) => inspection,
        Err(error) => return failed_inspection(name, identity, schema_version, error),
    };
    let Some(current) = inspection.current() else {
        let mut entry = ManagedCatalogEntry::problem(
            name.into(),
            ManagedEntryStatus::Invalid,
            "managed.invalid_bundle",
            "The bundle could not be projected through compatibility inspection.",
        );
        entry.schema_version = schema_version;
        append_diagnostics(&mut entry, inspection.report().diagnostics());
        return ProjectedEntry {
            entry,
            record: LocatorRecord {
                bundle_name: name.into(),
                identity,
                fingerprint: None,
                activatable: false,
            },
        };
    };

    let bundle = &current.bundle;
    let mut bands = inspection.planned_bands().to_vec();
    for slot in &bundle.schedule.slots {
        if !bands.contains(&slot.band) {
            bands.push(slot.band);
        }
    }
    let antenna_labels = bundle
        .antennas
        .antennas
        .iter()
        .map(|antenna| antenna.label.clone())
        .collect::<Vec<_>>();
    let fingerprint =
        projection_fingerprint(schema_version, current, inspection.report().diagnostics()).ok();
    let mut entry = ManagedCatalogEntry {
        locator_id: None,
        bundle_name: name.into(),
        origin: ManagedEntryOrigin::Managed,
        origin_label: "Saved by AntennaBench".into(),
        status: if fingerprint.is_some() {
            ManagedEntryStatus::Available
        } else {
            ManagedEntryStatus::Invalid
        },
        session_id: Some(bundle.manifest.session_id.clone()),
        callsign: Some(bundle.station.callsign.clone()),
        created_at: Some(bundle.manifest.created_at),
        lifecycle: current.session_state.as_ref().map(|state| state.lifecycle),
        schema_version,
        revision: current.session_state.as_ref().map(|state| state.revision),
        mode: Some(bundle.schedule.mode),
        bands,
        antenna_labels,
        antenna_count: Some(bundle.antennas.antennas.len()),
        same_session_id_count: 1,
        problems: Vec::new(),
    };
    append_diagnostics(&mut entry, inspection.report().diagnostics());
    if fingerprint.is_none() {
        push_problem(
            &mut entry,
            "managed.fingerprint_failed",
            ProblemSeverity::Error,
            "The inspected bundle could not be fingerprinted safely.",
        );
    }
    ProjectedEntry {
        record: LocatorRecord {
            bundle_name: name.into(),
            identity,
            fingerprint,
            activatable: fingerprint.is_some(),
        },
        entry,
    }
}

fn failed_inspection(
    name: &str,
    identity: DirectoryIdentity,
    schema_version: Option<u16>,
    error: BundleStoreError,
) -> ProjectedEntry {
    let status = classify_storage_error(&error);
    let message = match status {
        ManagedEntryStatus::Unsupported => {
            "This bundle uses a schema version that this AntennaBench build does not support."
        }
        ManagedEntryStatus::Unreadable => {
            "A required bundle member could not be read within the local resource policy."
        }
        ManagedEntryStatus::Unsafe => {
            "The bundle contains an unsafe filesystem entry or path layout."
        }
        _ => "The bundle did not pass bounded compatibility inspection.",
    };
    let mut entry = ManagedCatalogEntry::problem(
        name.into(),
        status,
        match status {
            ManagedEntryStatus::Unsupported => "managed.unsupported_bundle",
            ManagedEntryStatus::Unreadable => "managed.unreadable_bundle",
            ManagedEntryStatus::Unsafe => "managed.unsafe_bundle",
            _ => "managed.invalid_bundle",
        },
        message,
    );
    entry.schema_version = schema_version;
    ProjectedEntry {
        entry,
        record: LocatorRecord {
            bundle_name: name.into(),
            identity,
            fingerprint: None,
            activatable: false,
        },
    }
}

fn classify_storage_error(error: &BundleStoreError) -> ManagedEntryStatus {
    match error {
        BundleStoreError::UnsupportedSchemaVersion { .. } => ManagedEntryStatus::Unsupported,
        BundleStoreError::Read { source, .. }
        | BundleStoreError::CreateDirectory { source, .. }
        | BundleStoreError::Write { source, .. }
            if source.kind() == io::ErrorKind::PermissionDenied =>
        {
            ManagedEntryStatus::Unreadable
        }
        BundleStoreError::InvalidBundleRoot { .. }
        | BundleStoreError::InvalidBundleFilePath { .. }
        | BundleStoreError::InvalidBundlePath { .. } => ManagedEntryStatus::Unsafe,
        BundleStoreError::Resource(_) => ManagedEntryStatus::Unreadable,
        _ => ManagedEntryStatus::Invalid,
    }
}

fn projection_fingerprint(
    schema_version: Option<u16>,
    current: &antennabench_core::v2::CurrentBundleContents,
    diagnostics: &[antennabench_core::BundleDiagnostic],
) -> Result<[u8; 32], serde_json::Error> {
    let mut writer = DigestWriter(Sha256::new());
    serde_json::to_writer(&mut writer, &schema_version)?;
    serde_json::to_writer(&mut writer, &current.bundle)?;
    writer
        .0
        .update(format!("{:?}", current.record_provenance).as_bytes());
    serde_json::to_writer(&mut writer, &current.adapter_records)?;
    serde_json::to_writer(&mut writer, &current.session_state)?;
    for diagnostic in diagnostics {
        writer.0.update(format!("{diagnostic:?}").as_bytes());
    }
    Ok(writer.0.finalize().into())
}

struct DigestWriter(Sha256);

impl Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn append_diagnostics(
    entry: &mut ManagedCatalogEntry,
    diagnostics: &[antennabench_core::BundleDiagnostic],
) {
    let visible = if diagnostics.len() > MAX_ENTRY_PROBLEMS {
        MAX_ENTRY_PROBLEMS.saturating_sub(1)
    } else {
        diagnostics.len()
    };
    for diagnostic in diagnostics.iter().take(visible) {
        push_problem(
            entry,
            &diagnostic.code,
            if diagnostic.severity == BundleDiagnosticSeverity::Error {
                ProblemSeverity::Error
            } else {
                ProblemSeverity::Warning
            },
            &diagnostic.message,
        );
    }
    if diagnostics.len() > MAX_ENTRY_PROBLEMS {
        push_problem(
            entry,
            "managed.problems_truncated",
            ProblemSeverity::Warning,
            &format!(
                "Only the first {MAX_ENTRY_PROBLEMS} of {} inspection problems are shown.",
                diagnostics.len()
            ),
        );
    }
}

fn push_problem(
    entry: &mut ManagedCatalogEntry,
    code: &str,
    severity: ProblemSeverity,
    message: &str,
) {
    if entry.problems.len() >= MAX_ENTRY_PROBLEMS {
        return;
    }
    entry.problems.push(ManagedSessionProblem {
        code: clipped(code),
        severity,
        message: clipped(message),
    });
}

fn clipped(value: &str) -> String {
    if value.len() <= MAX_PROBLEM_TEXT_BYTES {
        return value.into();
    }
    let mut end = MAX_PROBLEM_TEXT_BYTES.saturating_sub(3);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &value[..end])
}

fn apply_same_id_warnings(entries: &mut [ProjectedEntry]) {
    for index in 0..entries.len() {
        let Some(session_id) = entries[index].entry.session_id.as_deref() else {
            continue;
        };
        let count = entries
            .iter()
            .filter(|candidate| candidate.entry.session_id.as_deref() == Some(session_id))
            .count();
        entries[index].entry.same_session_id_count = count;
        if count > 1 {
            if entries[index].entry.problems.len() == MAX_ENTRY_PROBLEMS {
                entries[index].entry.problems.pop();
            }
            push_problem(
                &mut entries[index].entry,
                "managed.same_session_id",
                ProblemSeverity::Warning,
                &format!(
                    "{count} saved bundle paths report this session ID; they remain separate copies."
                ),
            );
        }
    }
}

fn catalog_order(left: &ManagedCatalogEntry, right: &ManagedCatalogEntry) -> std::cmp::Ordering {
    match (left.created_at, right.created_at) {
        (Some(left_time), Some(right_time)) => right_time
            .cmp(&left_time)
            .then_with(|| left.bundle_name.cmp(&right.bundle_name)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.bundle_name.cmp(&right.bundle_name),
    }
}

fn is_plain_name(name: &str) -> bool {
    Path::new(name).components().count() == 1
        && !matches!(
            Path::new(name).components().next(),
            Some(std::path::Component::ParentDir)
        )
}

fn directory_identity(metadata: &fs::Metadata) -> DirectoryIdentity {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    DirectoryIdentity {
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
        length: metadata.len(),
        modified_nanos: metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos()),
    }
}

fn stale_locator_error() -> SessionErrorPayload {
    SessionErrorPayload::new(
        SessionErrorKind::Verification,
        "The saved session changed after the catalog was loaded.",
        "refresh Saved sessions and choose the current entry",
    )
}

fn root_read_error(_root: &Path, error: io::Error) -> SessionErrorPayload {
    SessionErrorPayload::new(
        SessionErrorKind::Filesystem,
        "The AntennaBench sessions directory could not be read.",
        error.to_string(),
    )
}

enum Candidate {
    Directory {
        name: String,
        path: PathBuf,
    },
    Unsafe {
        name: String,
    },
    Unreadable {
        name: String,
        path: PathBuf,
        message: String,
    },
}

impl Candidate {
    fn name(&self) -> &str {
        match self {
            Self::Directory { name, .. }
            | Self::Unsafe { name }
            | Self::Unreadable { name, .. } => name,
        }
    }
}

struct ProjectedEntry {
    entry: ManagedCatalogEntry,
    record: LocatorRecord,
}

impl LocatorRecord {
    fn unavailable() -> Self {
        Self {
            bundle_name: String::new(),
            identity: DirectoryIdentity {
                #[cfg(unix)]
                device: 0,
                #[cfg(unix)]
                inode: 0,
                length: 0,
                modified_nanos: None,
            },
            fingerprint: None,
            activatable: false,
        }
    }
}

impl ManagedCatalogEntry {
    fn problem(bundle_name: String, status: ManagedEntryStatus, code: &str, message: &str) -> Self {
        Self {
            locator_id: None,
            bundle_name,
            origin: ManagedEntryOrigin::Managed,
            origin_label: "Saved by AntennaBench".into(),
            status,
            session_id: None,
            callsign: None,
            created_at: None,
            lifecycle: None,
            schema_version: None,
            revision: None,
            mode: None,
            bands: Vec::new(),
            antenna_labels: Vec::new(),
            antenna_count: None,
            same_session_id_count: 0,
            problems: vec![ManagedSessionProblem {
                code: clipped(code),
                severity: ProblemSeverity::Error,
                message: clipped(message),
            }],
        }
    }
}
