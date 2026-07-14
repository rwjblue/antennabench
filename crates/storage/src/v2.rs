use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use antennabench_core::{
    codes, validate_bundle_report, AdapterInput, AttachmentReference, BundleDiagnostic,
    BundleDiagnosticCategory, BundleDiagnosticLocation, BundleDiagnosticSeverity, BundleFileRole,
    BundleFilesV2, BundleManifestV2, BundleRecordKind, BundleV2Contents, BundleValidationProfile,
    BundleValidationReport, StreamCheckpointV2, ALL_TYPED_OPERATIONS, SCHEMA_VERSION_V2,
    V2_BUNDLE_SUFFIX,
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

use super::{
    create_directory, ensure_bundle_root, ensure_directory, inspection::scan_duplicate_members,
    BundleInspection, BundleStore, BundleStoreError,
};

#[derive(Debug, Clone)]
pub(super) struct ResolvedBundlePathsV2 {
    pub(super) manifest: PathBuf,
    pub(super) session_state: PathBuf,
    pub(super) station: PathBuf,
    pub(super) antennas: PathBuf,
    pub(super) schedule: PathBuf,
    pub(super) events: PathBuf,
    pub(super) observations: PathBuf,
    pub(super) adapter_records: PathBuf,
    pub(super) rig: PathBuf,
    pub(super) propagation: PathBuf,
    pub(super) analysis: PathBuf,
    pub(super) attachments_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleAttachment {
    pub reference: AttachmentReference,
    pub bytes: Vec<u8>,
}

impl BundleAttachment {
    pub fn new(
        bytes: Vec<u8>,
        media_type: impl Into<String>,
        encoding: Option<String>,
        container: Option<String>,
        source_locator: Option<String>,
    ) -> Self {
        let reference = AttachmentReference {
            sha256: sha256_hex(&bytes),
            byte_size: u64::try_from(bytes.len()).expect("usize fits in u64"),
            media_type: media_type.into(),
            encoding,
            container,
            source_locator,
        };
        Self { reference, bytes }
    }
}

impl ResolvedBundlePathsV2 {
    pub(super) fn root_files(&self) -> [&Path; 11] {
        [
            &self.manifest,
            &self.session_state,
            &self.station,
            &self.antennas,
            &self.schedule,
            &self.events,
            &self.observations,
            &self.adapter_records,
            &self.rig,
            &self.propagation,
            &self.analysis,
        ]
    }

    fn ensure_unique(&self) -> Result<(), BundleStoreError> {
        let mut seen = HashSet::new();
        for path in self
            .root_files()
            .into_iter()
            .chain([self.attachments_dir.as_path()])
        {
            if !seen.insert(path.to_path_buf()) {
                return Err(BundleStoreError::DuplicateBundlePath {
                    path: path.to_string_lossy().into_owned(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn ensure_readable(&self) -> Result<(), BundleStoreError> {
        for path in self.root_files() {
            if matches!(fs::symlink_metadata(path), Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file())
            {
                return Err(BundleStoreError::InvalidBundleFilePath {
                    path: path.to_path_buf(),
                });
            }
        }
        ensure_directory(&self.attachments_dir)
    }
}

impl BundleStore {
    /// Recomputes the static v2 plan and stream heads before a new destination write.
    /// Live checkpoint promotion remains the responsibility of the persistence layer.
    pub fn refresh_v2_checkpoint(bundle: &mut BundleV2Contents) -> Result<(), BundleStoreError> {
        let station =
            serialize_json(&bundle.station).map_err(|source| BundleStoreError::SerializeJson {
                path: PathBuf::from("station.json"),
                source,
            })?;
        let antennas =
            serialize_json(&bundle.antennas).map_err(|source| BundleStoreError::SerializeJson {
                path: PathBuf::from("antennas.json"),
                source,
            })?;
        let schedule =
            serialize_json(&bundle.schedule).map_err(|source| BundleStoreError::SerializeJson {
                path: PathBuf::from("schedule.json"),
                source,
            })?;
        let station_sha256 = sha256_hex(&station);
        let antennas_sha256 = sha256_hex(&antennas);
        let schedule_sha256 = sha256_hex(&schedule);
        bundle.session_state.active_plan.station_sha256 = station_sha256.clone();
        bundle.session_state.active_plan.antennas_sha256 = antennas_sha256.clone();
        bundle.session_state.active_plan.schedule_sha256 = schedule_sha256.clone();
        bundle.session_state.active_plan.root_sha256 = sha256_hex(
            [station_sha256, antennas_sha256, schedule_sha256]
                .join("\n")
                .as_bytes(),
        );

        let events =
            serialize_jsonl(&bundle.events).map_err(|source| BundleStoreError::SerializeJson {
                path: PathBuf::from("events.jsonl"),
                source,
            })?;
        let observations = serialize_jsonl(&bundle.observations).map_err(|source| {
            BundleStoreError::SerializeJson {
                path: PathBuf::from("observations.jsonl"),
                source,
            }
        })?;
        let adapter_records = serialize_jsonl(&bundle.adapter_records).map_err(|source| {
            BundleStoreError::SerializeJson {
                path: PathBuf::from("adapter-records.jsonl"),
                source,
            }
        })?;
        let rig =
            serialize_jsonl(&bundle.rig).map_err(|source| BundleStoreError::SerializeJson {
                path: PathBuf::from("rig.jsonl"),
                source,
            })?;
        let propagation = serialize_jsonl(&bundle.propagation).map_err(|source| {
            BundleStoreError::SerializeJson {
                path: PathBuf::from("propagation.jsonl"),
                source,
            }
        })?;
        bundle.session_state.streams = [
            (
                "events".to_string(),
                checkpoint_for_bytes(
                    &events,
                    bundle.events.len(),
                    bundle.events.last().map(|record| record.event_id.clone()),
                ),
            ),
            (
                "observations".to_string(),
                checkpoint_for_bytes(
                    &observations,
                    bundle.observations.len(),
                    bundle
                        .observations
                        .last()
                        .map(|record| record.observation_id.clone()),
                ),
            ),
            (
                "adapter_records".to_string(),
                checkpoint_for_bytes(
                    &adapter_records,
                    bundle.adapter_records.len(),
                    bundle
                        .adapter_records
                        .last()
                        .map(|record| record.record_id.clone()),
                ),
            ),
            (
                "rig".to_string(),
                checkpoint_for_bytes(
                    &rig,
                    bundle.rig.len(),
                    bundle.rig.last().map(|record| record.record_id.clone()),
                ),
            ),
            (
                "propagation".to_string(),
                checkpoint_for_bytes(
                    &propagation,
                    bundle.propagation.len(),
                    bundle
                        .propagation
                        .last()
                        .map(|record| record.record_id.clone()),
                ),
            ),
        ]
        .into_iter()
        .collect();
        Ok(())
    }

    pub(super) fn v2_paths(
        &self,
        files: &BundleFilesV2,
    ) -> Result<ResolvedBundlePathsV2, BundleStoreError> {
        let paths = ResolvedBundlePathsV2 {
            manifest: self.bundle_path("manifest.json")?,
            session_state: self.bundle_path(&files.session_state)?,
            station: self.bundle_path(&files.station)?,
            antennas: self.bundle_path(&files.antennas)?,
            schedule: self.bundle_path(&files.schedule)?,
            events: self.bundle_path(&files.events)?,
            observations: self.bundle_path(&files.observations)?,
            adapter_records: self.bundle_path(&files.adapter_records)?,
            rig: self.bundle_path(&files.rig)?,
            propagation: self.bundle_path(&files.propagation)?,
            analysis: self.bundle_path(&files.analysis)?,
            attachments_dir: self.bundle_path(&files.attachments_dir)?,
        };
        if files.manifest != "manifest.json" {
            return Err(BundleStoreError::InvalidV2Bundle {
                message: "v2 manifest must use the bootstrap path manifest.json".into(),
            });
        }
        paths.ensure_unique()?;
        Ok(paths)
    }

    pub(super) fn inspect_v2(
        &self,
        mut report: BundleValidationReport,
    ) -> Result<BundleInspection, BundleStoreError> {
        let (bundle, paths) = self.load_v2_bundle()?;
        report.extend(validate_v2_bundle(&bundle, &paths, true));
        let current = bundle.into_current();
        report.extend(validate_bundle_report(&current.bundle).into_diagnostics());
        let current = report
            .allows(BundleValidationProfile::CompatibilityRead)
            .then_some(current);
        Ok(BundleInspection { current, report })
    }

    pub fn read_v2(&self) -> Result<BundleV2Contents, BundleStoreError> {
        let (bundle, paths) = self.load_v2_bundle()?;
        let mut report = BundleValidationReport::new(validate_v2_bundle(&bundle, &paths, true));
        report.extend(
            validate_bundle_report(&bundle.clone().into_current().bundle).into_diagnostics(),
        );
        if !report.allows(BundleValidationProfile::CompatibilityRead) {
            return Err(antennabench_core::BundleValidationError::from_report(report).into());
        }
        Ok(bundle)
    }

    fn load_v2_bundle(
        &self,
    ) -> Result<(BundleV2Contents, ResolvedBundlePathsV2), BundleStoreError> {
        let manifest: BundleManifestV2 = read_json(&self.bundle_path("manifest.json")?)?;
        if manifest.schema_version != SCHEMA_VERSION_V2 {
            return Err(BundleStoreError::UnsupportedSchemaVersion {
                actual: manifest.schema_version,
            });
        }
        let paths = self.v2_paths(&manifest.files)?;
        paths.ensure_readable()?;

        let bundle = BundleV2Contents {
            manifest,
            session_state: read_json(&paths.session_state)?,
            station: read_json(&paths.station)?,
            antennas: read_json(&paths.antennas)?,
            schedule: read_json(&paths.schedule)?,
            events: read_jsonl(&paths.events)?,
            observations: read_jsonl(&paths.observations)?,
            adapter_records: read_jsonl(&paths.adapter_records)?,
            rig: read_jsonl(&paths.rig)?,
            propagation: read_jsonl(&paths.propagation)?,
            analysis: read_json(&paths.analysis)?,
        };

        Ok((bundle, paths))
    }

    /// Writes a newly authored schema-v2 bundle to a new neutral-suffix destination.
    pub fn write_v2(&self, bundle: &BundleV2Contents) -> Result<(), BundleStoreError> {
        if bundle
            .adapter_records
            .iter()
            .any(|record| matches!(&record.input, AdapterInput::Attachment { .. }))
        {
            return Err(BundleStoreError::InvalidV2Bundle {
                message:
                    "use write_v2_with_attachments when adapter evidence references attachments"
                        .into(),
            });
        }
        self.write_v2_files(bundle)
    }

    pub fn write_v2_with_attachments(
        &self,
        bundle: &BundleV2Contents,
        attachments: &[BundleAttachment],
    ) -> Result<(), BundleStoreError> {
        self.write_v2_files(bundle)?;
        let result = (|| {
            for attachment in attachments {
                let written = self.write_attachment(
                    &attachment.bytes,
                    attachment.reference.media_type.clone(),
                    attachment.reference.encoding.clone(),
                    attachment.reference.container.clone(),
                    attachment.reference.source_locator.clone(),
                )?;
                if written != attachment.reference {
                    return Err(BundleStoreError::InvalidAttachmentReference {
                        message: "provided attachment metadata does not match content".into(),
                    });
                }
            }
            let inspection = self.inspect()?;
            if inspection.bundle().is_none() {
                return Err(BundleStoreError::InvalidV2Bundle {
                    message: "written bundle did not reopen after attachment verification".into(),
                });
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(self.root());
        }
        result
    }

    fn write_v2_files(&self, bundle: &BundleV2Contents) -> Result<(), BundleStoreError> {
        ensure_v2_suffix(self.root())?;
        if bundle.manifest.schema_version != SCHEMA_VERSION_V2 {
            return Err(BundleStoreError::InvalidV2Bundle {
                message: "manifest schema_version must be 2".into(),
            });
        }
        if fs::symlink_metadata(self.root()).is_ok() {
            return Err(BundleStoreError::DestinationExists {
                path: self.root().to_path_buf(),
            });
        }
        let paths = self.v2_paths(&bundle.manifest.files)?;
        create_directory(self.root())?;
        let result = (|| {
            write_json_bytes(&paths.manifest, &bundle.manifest)?;
            write_json_bytes(&paths.session_state, &bundle.session_state)?;
            write_json_bytes(&paths.station, &bundle.station)?;
            write_json_bytes(&paths.antennas, &bundle.antennas)?;
            write_json_bytes(&paths.schedule, &bundle.schedule)?;
            write_jsonl_bytes(&paths.events, &bundle.events)?;
            write_jsonl_bytes(&paths.observations, &bundle.observations)?;
            write_jsonl_bytes(&paths.adapter_records, &bundle.adapter_records)?;
            write_jsonl_bytes(&paths.rig, &bundle.rig)?;
            write_jsonl_bytes(&paths.propagation, &bundle.propagation)?;
            write_json_bytes(&paths.analysis, &bundle.analysis)?;
            create_directory(&paths.attachments_dir)?;

            let diagnostics = validate_v2_bundle(bundle, &paths, false);
            if !diagnostics.is_empty() {
                return Err(BundleStoreError::InvalidV2Bundle {
                    message: diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("; "),
                });
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(self.root());
        }
        result
    }

    pub fn write_attachment(
        &self,
        bytes: &[u8],
        media_type: impl Into<String>,
        encoding: Option<String>,
        container: Option<String>,
        source_locator: Option<String>,
    ) -> Result<AttachmentReference, BundleStoreError> {
        ensure_bundle_root(self.root())?;
        let digest = sha256_hex(bytes);
        let reference = AttachmentReference {
            sha256: digest.clone(),
            byte_size: u64::try_from(bytes.len()).expect("usize fits in u64"),
            media_type: media_type.into(),
            encoding,
            container,
            source_locator,
        };
        let manifest: BundleManifestV2 = read_json(&self.bundle_path("manifest.json")?)?;
        let paths = self.v2_paths(&manifest.files)?;
        let digest_dir = paths.attachments_dir.join("sha256");
        create_directory(&digest_dir)?;
        let path = digest_dir.join(digest);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => file
                .write_all(bytes)
                .map_err(|source| BundleStoreError::Write {
                    path: path.clone(),
                    source,
                })?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = fs::read(&path).map_err(|source| BundleStoreError::Read {
                    path: path.clone(),
                    source,
                })?;
                if existing != bytes {
                    return Err(BundleStoreError::AttachmentMismatch { path });
                }
            }
            Err(source) => {
                return Err(BundleStoreError::Write {
                    path: path.clone(),
                    source,
                })
            }
        }
        Ok(reference)
    }

    pub fn read_attachment(
        &self,
        reference: &AttachmentReference,
    ) -> Result<Vec<u8>, BundleStoreError> {
        let relative = reference.relative_path().ok_or_else(|| {
            BundleStoreError::InvalidAttachmentReference {
                message: format!("invalid SHA-256 digest {}", reference.sha256),
            }
        })?;
        let manifest: BundleManifestV2 = read_json(&self.bundle_path("manifest.json")?)?;
        let paths = self.v2_paths(&manifest.files)?;
        let path = paths.attachments_dir.join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|source| BundleStoreError::Read {
            path: path.clone(),
            source,
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(BundleStoreError::InvalidAttachmentReference {
                message: format!("attachment is not a regular file: {}", path.display()),
            });
        }
        let bytes = fs::read(&path).map_err(|source| BundleStoreError::Read {
            path: path.clone(),
            source,
        })?;
        if u64::try_from(bytes.len()).ok() != Some(reference.byte_size)
            || sha256_hex(&bytes) != reference.sha256
        {
            return Err(BundleStoreError::AttachmentMismatch { path });
        }
        Ok(bytes)
    }
}

pub(super) fn serialize_json<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn serialize_jsonl<T: Serialize>(records: &[T]) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = Vec::new();
    for record in records {
        serde_json::to_writer(&mut bytes, record)?;
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn write_json_bytes<T: Serialize>(path: &Path, value: &T) -> Result<(), BundleStoreError> {
    let bytes = serialize_json(value).map_err(|source| BundleStoreError::SerializeJson {
        path: path.to_path_buf(),
        source,
    })?;
    fs::write(path, bytes).map_err(|source| BundleStoreError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn write_jsonl_bytes<T: Serialize>(path: &Path, value: &[T]) -> Result<(), BundleStoreError> {
    let bytes = serialize_jsonl(value).map_err(|source| BundleStoreError::SerializeJson {
        path: path.to_path_buf(),
        source,
    })?;
    fs::write(path, bytes).map_err(|source| BundleStoreError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, BundleStoreError> {
    let bytes = fs::read(path).map_err(|source| BundleStoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| BundleStoreError::ParseJson {
        path: path.to_path_buf(),
        source,
    })
}

fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, BundleStoreError> {
    let contents = fs::read_to_string(path).map_err(|source| BundleStoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|source| BundleStoreError::ParseJson {
                path: path.to_path_buf(),
                source,
            })
        })
        .collect()
}

pub(super) fn ensure_v2_suffix(path: &Path) -> Result<(), BundleStoreError> {
    if path.to_string_lossy().ends_with(V2_BUNDLE_SUFFIX) {
        Ok(())
    } else {
        Err(BundleStoreError::InvalidBundleSuffix {
            path: path.to_path_buf(),
            schema_version: SCHEMA_VERSION_V2,
        })
    }
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("write to String");
    }
    output
}

pub(super) fn checkpoint_for_bytes(
    bytes: &[u8],
    record_count: usize,
    last_record_id: Option<String>,
) -> StreamCheckpointV2 {
    StreamCheckpointV2 {
        committed_bytes: u64::try_from(bytes.len()).expect("usize fits in u64"),
        record_count: u64::try_from(record_count).expect("usize fits in u64"),
        last_record_id,
        committed_sha256: sha256_hex(bytes),
    }
}

fn validate_v2_bundle(
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
    verify_attachments: bool,
) -> Vec<BundleDiagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(duplicate_member_diagnostics(paths));
    let session_id = bundle.manifest.session_id.as_str();
    for (file, schema, actual_session) in [
        (
            BundleFileRole::Manifest,
            bundle.manifest.schema_version,
            bundle.manifest.session_id.as_str(),
        ),
        (
            BundleFileRole::SessionState,
            bundle.session_state.schema_version,
            bundle.session_state.session_id.as_str(),
        ),
        (
            BundleFileRole::Station,
            bundle.station.schema_version,
            bundle.station.session_id.as_str(),
        ),
        (
            BundleFileRole::Antennas,
            bundle.antennas.schema_version,
            bundle.antennas.session_id.as_str(),
        ),
        (
            BundleFileRole::Schedule,
            bundle.schedule.schema_version,
            bundle.schedule.session_id.as_str(),
        ),
        (
            BundleFileRole::Analysis,
            bundle.analysis.schema_version,
            bundle.analysis.session_id.as_str(),
        ),
    ] {
        if schema != SCHEMA_VERSION_V2 || actual_session != session_id {
            diagnostics.push(v2_diagnostic(
                codes::V2_CHECKPOINT_MISMATCH,
                file,
                None,
                format!("schema/session identity does not match v2 manifest for {file:?}"),
            ));
        }
    }

    let mut adapter_ids = HashSet::new();
    for record in &bundle.adapter_records {
        if !adapter_ids.insert(record.record_id.as_str()) {
            diagnostics.push(v2_diagnostic(
                codes::V2_ADAPTER_LINK,
                BundleFileRole::AdapterRecords,
                Some(record.record_id.as_str()),
                "adapter record ID is duplicated".into(),
            ));
        }
        validate_meta(
            &record.meta,
            session_id,
            BundleFileRole::AdapterRecords,
            &record.record_id,
            &mut diagnostics,
        );
        if let AdapterInput::Attachment { attachment } = &record.input {
            if attachment.relative_path().is_none() {
                diagnostics.push(v2_diagnostic(
                    codes::V2_ATTACHMENT,
                    BundleFileRole::AdapterRecords,
                    Some(&record.record_id),
                    "attachment reference has an invalid SHA-256 digest".into(),
                ));
            } else if verify_attachments
                && paths.attachments_dir.exists()
                && BundleStore::new(paths.manifest.parent().expect("bundle root"))
                    .read_attachment(attachment)
                    .is_err()
            {
                diagnostics.push(v2_diagnostic(
                    codes::V2_ATTACHMENT,
                    BundleFileRole::AdapterRecords,
                    Some(&record.record_id),
                    "attachment reference cannot be verified".into(),
                ));
            }
        }
    }

    let observation_ids = bundle
        .observations
        .iter()
        .map(|record| record.observation_id.as_str())
        .collect::<HashSet<_>>();
    let rig_ids = bundle
        .rig
        .iter()
        .map(|record| record.record_id.as_str())
        .collect::<HashSet<_>>();
    let propagation_ids = bundle
        .propagation
        .iter()
        .map(|record| record.record_id.as_str())
        .collect::<HashSet<_>>();
    for adapter in &bundle.adapter_records {
        for link in &adapter.normalized_records {
            let exists = match link.record_kind {
                antennabench_core::NormalizedRecordKind::Observation => {
                    observation_ids.contains(link.record_id.as_str())
                }
                antennabench_core::NormalizedRecordKind::Rig => {
                    rig_ids.contains(link.record_id.as_str())
                }
                antennabench_core::NormalizedRecordKind::Propagation => {
                    propagation_ids.contains(link.record_id.as_str())
                }
            };
            if !exists {
                diagnostics.push(v2_diagnostic(
                    codes::V2_ADAPTER_LINK,
                    BundleFileRole::AdapterRecords,
                    Some(&adapter.record_id),
                    format!(
                        "normalized record link {:?} does not resolve",
                        link.record_id
                    ),
                ));
            }
        }
    }
    for observation in &bundle.observations {
        validate_meta(
            &observation.meta,
            session_id,
            BundleFileRole::Observations,
            &observation.observation_id,
            &mut diagnostics,
        );
        validate_backlinks(
            &observation.adapter_record_ids,
            &adapter_ids,
            BundleFileRole::Observations,
            &observation.observation_id,
            &mut diagnostics,
        );
        validate_reciprocal_link(
            &bundle.adapter_records,
            &observation.adapter_record_ids,
            antennabench_core::NormalizedRecordKind::Observation,
            &observation.observation_id,
            BundleFileRole::Observations,
            &mut diagnostics,
        );
    }
    for event in &bundle.events {
        validate_meta(
            &event.meta,
            session_id,
            BundleFileRole::Events,
            &event.event_id,
            &mut diagnostics,
        );
    }
    for record in &bundle.rig {
        validate_meta(
            &record.meta,
            session_id,
            BundleFileRole::Rig,
            &record.record_id,
            &mut diagnostics,
        );
        validate_backlinks(
            &record.adapter_record_ids,
            &adapter_ids,
            BundleFileRole::Rig,
            &record.record_id,
            &mut diagnostics,
        );
        validate_reciprocal_link(
            &bundle.adapter_records,
            &record.adapter_record_ids,
            antennabench_core::NormalizedRecordKind::Rig,
            &record.record_id,
            BundleFileRole::Rig,
            &mut diagnostics,
        );
    }
    for record in &bundle.propagation {
        validate_meta(
            &record.meta,
            session_id,
            BundleFileRole::Propagation,
            &record.record_id,
            &mut diagnostics,
        );
        validate_backlinks(
            &record.adapter_record_ids,
            &adapter_ids,
            BundleFileRole::Propagation,
            &record.record_id,
            &mut diagnostics,
        );
        validate_reciprocal_link(
            &bundle.adapter_records,
            &record.adapter_record_ids,
            antennabench_core::NormalizedRecordKind::Propagation,
            &record.record_id,
            BundleFileRole::Propagation,
            &mut diagnostics,
        );
    }

    diagnostics.extend(validate_checkpoint(bundle, paths));
    diagnostics
}

fn duplicate_member_diagnostics(paths: &ResolvedBundlePathsV2) -> Vec<BundleDiagnostic> {
    let mut diagnostics = Vec::new();
    for (file, path) in [
        (BundleFileRole::Manifest, paths.manifest.as_path()),
        (BundleFileRole::SessionState, paths.session_state.as_path()),
        (BundleFileRole::Station, paths.station.as_path()),
        (BundleFileRole::Antennas, paths.antennas.as_path()),
        (BundleFileRole::Schedule, paths.schedule.as_path()),
        (BundleFileRole::Analysis, paths.analysis.as_path()),
    ] {
        if let Ok(contents) = fs::read_to_string(path) {
            if let Ok(duplicates) = scan_duplicate_members(&contents) {
                diagnostics.extend(duplicates.into_iter().map(|field_path| {
                    let mut diagnostic = v2_diagnostic(
                        codes::DUPLICATE_MEMBER,
                        file,
                        None,
                        "duplicate member makes schema-v2 modeled JSON ambiguous".into(),
                    );
                    diagnostic.location.field_path = Some(field_path);
                    diagnostic
                }));
            }
        }
    }
    for (file, path) in [
        (BundleFileRole::Events, paths.events.as_path()),
        (BundleFileRole::Observations, paths.observations.as_path()),
        (
            BundleFileRole::AdapterRecords,
            paths.adapter_records.as_path(),
        ),
        (BundleFileRole::Rig, paths.rig.as_path()),
        (BundleFileRole::Propagation, paths.propagation.as_path()),
    ] {
        if let Ok(contents) = fs::read_to_string(path) {
            for (line_index, line) in contents.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(duplicates) = scan_duplicate_members(line) {
                    diagnostics.extend(duplicates.into_iter().map(|field_path| {
                        let mut diagnostic = v2_diagnostic(
                            codes::DUPLICATE_MEMBER,
                            file,
                            None,
                            "duplicate member makes schema-v2 modeled JSON ambiguous".into(),
                        );
                        diagnostic.location.physical_line = Some(line_index + 1);
                        diagnostic.location.field_path = Some(field_path);
                        diagnostic
                    }));
                }
            }
        }
    }
    diagnostics
}

fn validate_meta(
    meta: &antennabench_core::RecordMetaV2,
    session_id: &str,
    file: BundleFileRole,
    record_id: &str,
    diagnostics: &mut Vec<BundleDiagnostic>,
) {
    if meta.schema_version != SCHEMA_VERSION_V2 || meta.session_id != session_id {
        diagnostics.push(v2_diagnostic(
            codes::V2_MUTATION,
            file,
            Some(record_id),
            "record schema/session identity does not match v2 manifest".into(),
        ));
    }
    if meta.mutation.mutation_id.is_empty()
        || meta.mutation.member_count == 0
        || meta.mutation.member_index >= meta.mutation.member_count
    {
        diagnostics.push(v2_diagnostic(
            codes::V2_MUTATION,
            file,
            Some(record_id),
            "mutation member requires a nonempty ID and index less than member count".into(),
        ));
    }
}

fn validate_backlinks(
    links: &[String],
    adapter_ids: &HashSet<&str>,
    file: BundleFileRole,
    record_id: &str,
    diagnostics: &mut Vec<BundleDiagnostic>,
) {
    if links.is_empty() || links.iter().any(|id| !adapter_ids.contains(id.as_str())) {
        diagnostics.push(v2_diagnostic(
            codes::V2_ADAPTER_LINK,
            file,
            Some(record_id),
            "adapter-produced normalized record must link to existing adapter evidence".into(),
        ));
    }
}

fn validate_reciprocal_link(
    adapters: &[antennabench_core::AdapterRecordV2],
    adapter_ids: &[String],
    record_kind: antennabench_core::NormalizedRecordKind,
    record_id: &str,
    file: BundleFileRole,
    diagnostics: &mut Vec<BundleDiagnostic>,
) {
    let reciprocal = adapters.iter().any(|adapter| {
        adapter_ids.contains(&adapter.record_id)
            && adapter
                .normalized_records
                .iter()
                .any(|link| link.record_kind == record_kind && link.record_id == record_id)
    });
    if !reciprocal {
        diagnostics.push(v2_diagnostic(
            codes::V2_ADAPTER_LINK,
            file,
            Some(record_id),
            "normalized record and adapter evidence must link to each other".into(),
        ));
    }
}

fn validate_checkpoint(
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
) -> Vec<BundleDiagnostic> {
    let mut diagnostics = Vec::new();
    let plan = [
        (
            "station",
            &paths.station,
            bundle.session_state.active_plan.station_sha256.as_str(),
        ),
        (
            "antennas",
            &paths.antennas,
            bundle.session_state.active_plan.antennas_sha256.as_str(),
        ),
        (
            "schedule",
            &paths.schedule,
            bundle.session_state.active_plan.schedule_sha256.as_str(),
        ),
    ];
    let mut plan_digests = Vec::new();
    for (name, path, expected) in plan {
        if let Ok(bytes) = fs::read(path) {
            let actual = sha256_hex(&bytes);
            plan_digests.push(actual.clone());
            if actual != expected {
                diagnostics.push(v2_diagnostic(
                    codes::V2_CHECKPOINT_MISMATCH,
                    BundleFileRole::SessionState,
                    None,
                    format!("active plan {name} digest does not match checkpoint"),
                ));
            }
        }
    }
    if plan_digests.len() == 3 {
        let root = sha256_hex(plan_digests.join("\n").as_bytes());
        if root != bundle.session_state.active_plan.root_sha256 {
            diagnostics.push(v2_diagnostic(
                codes::V2_CHECKPOINT_MISMATCH,
                BundleFileRole::SessionState,
                None,
                "active plan root digest does not match checkpoint".into(),
            ));
        }
    }

    let streams: [(&str, &Path, usize, Option<String>); 5] = [
        (
            "events",
            &paths.events,
            bundle.events.len(),
            bundle.events.last().map(|record| record.event_id.clone()),
        ),
        (
            "observations",
            &paths.observations,
            bundle.observations.len(),
            bundle
                .observations
                .last()
                .map(|record| record.observation_id.clone()),
        ),
        (
            "adapter_records",
            &paths.adapter_records,
            bundle.adapter_records.len(),
            bundle
                .adapter_records
                .last()
                .map(|record| record.record_id.clone()),
        ),
        (
            "rig",
            &paths.rig,
            bundle.rig.len(),
            bundle.rig.last().map(|record| record.record_id.clone()),
        ),
        (
            "propagation",
            &paths.propagation,
            bundle.propagation.len(),
            bundle
                .propagation
                .last()
                .map(|record| record.record_id.clone()),
        ),
    ];
    for (name, path, count, last_id) in streams {
        let Some(expected) = bundle.session_state.streams.get(name) else {
            diagnostics.push(v2_diagnostic(
                codes::V2_CHECKPOINT_MISMATCH,
                BundleFileRole::SessionState,
                None,
                format!("checkpoint is missing stream {name}"),
            ));
            continue;
        };
        if let Ok(bytes) = fs::read(path) {
            let actual = checkpoint_for_bytes(&bytes, count, last_id);
            if &actual != expected {
                diagnostics.push(v2_diagnostic(
                    codes::V2_CHECKPOINT_MISMATCH,
                    BundleFileRole::SessionState,
                    None,
                    format!("checkpoint head for stream {name} does not match durable bytes"),
                ));
            }
        }
    }
    diagnostics
}

fn v2_diagnostic(
    code: &str,
    file: BundleFileRole,
    record_id: Option<&str>,
    message: String,
) -> BundleDiagnostic {
    BundleDiagnostic {
        code: code.into(),
        category: BundleDiagnosticCategory::Structural,
        severity: BundleDiagnosticSeverity::Error,
        blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
        location: BundleDiagnosticLocation {
            file,
            record_kind: (file == BundleFileRole::AdapterRecords)
                .then_some(BundleRecordKind::AdapterRecord),
            record_id: record_id.map(str::to_string),
            record_index: None,
            physical_line: None,
            field_path: None,
        },
        message,
        related_locations: Vec::new(),
    }
}
