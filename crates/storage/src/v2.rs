use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use antennabench_core::{
    codes, validate_bundle_report, validate_machine_identity, AdapterInput, AttachmentReference,
    BundleDiagnostic, BundleDiagnosticCategory, BundleDiagnosticLocation, BundleDiagnosticSeverity,
    BundleFileRole, BundleFilesV2, BundleManifestV2, BundleRecordKind, BundleV2Contents,
    BundleValidationProfile, BundleValidationReport, CorrectableOperatorEventPayloadV2,
    SessionLifecycleV2, StreamCheckpointV2, ALL_TYPED_OPERATIONS, ANALYSIS_AND_WRITE_OPERATIONS,
    SCHEMA_VERSION_V2, V2_BUNDLE_SUFFIX, WRITE_OPERATIONS,
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

use super::{
    create_directory, ensure_bundle_root, ensure_directory,
    inspection::scan_duplicate_members,
    resource::{
        inventory_attachment_tree, preflight_attachment_write, read_bounded,
        serialize_jsonl_bounded, serialize_root_bounded, ModeledBudget, ResourceOperation,
        ResourceStage,
    },
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
        let store = BundleStore::new(".");
        let mut budget = ModeledBudget::default();
        let station_path = PathBuf::from("station.json");
        let antennas_path = PathBuf::from("antennas.json");
        let schedule_path = PathBuf::from("schedule.json");
        let station = serialize_root_bounded(&store, &station_path, &bundle.station, &mut budget)?;
        let antennas =
            serialize_root_bounded(&store, &antennas_path, &bundle.antennas, &mut budget)?;
        let schedule =
            serialize_root_bounded(&store, &schedule_path, &bundle.schedule, &mut budget)?;
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

        let events = serialize_jsonl_bounded(
            &store,
            Path::new("events.jsonl"),
            &bundle.events,
            &mut budget,
        )?;
        let observations = serialize_jsonl_bounded(
            &store,
            Path::new("observations.jsonl"),
            &bundle.observations,
            &mut budget,
        )?;
        let adapter_records = serialize_jsonl_bounded(
            &store,
            Path::new("adapter-records.jsonl"),
            &bundle.adapter_records,
            &mut budget,
        )?;
        let rig =
            serialize_jsonl_bounded(&store, Path::new("rig.jsonl"), &bundle.rig, &mut budget)?;
        let propagation = serialize_jsonl_bounded(
            &store,
            Path::new("propagation.jsonl"),
            &bundle.propagation,
            &mut budget,
        )?;
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
        report.extend(validate_v2_bundle(self, &bundle, &paths, true));
        let current = bundle.into_current();
        report.extend(validate_bundle_report(&current.bundle).into_diagnostics());
        let current = report
            .allows(BundleValidationProfile::CompatibilityRead)
            .then_some(current);
        Ok(BundleInspection { current, report })
    }

    pub fn read_v2(&self) -> Result<BundleV2Contents, BundleStoreError> {
        let (bundle, paths) = self.load_v2_bundle()?;
        let mut report =
            BundleValidationReport::new(validate_v2_bundle(self, &bundle, &paths, true));
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
        let mut budget = ModeledBudget::default();
        let manifest: BundleManifestV2 =
            self.read_json_bounded(&self.bundle_path("manifest.json")?, &mut budget)?;
        if manifest.schema_version != SCHEMA_VERSION_V2 {
            return Err(BundleStoreError::UnsupportedSchemaVersion {
                actual: manifest.schema_version,
            });
        }
        let paths = self.v2_paths(&manifest.files)?;
        paths.ensure_readable()?;
        self.inventory_root(ResourceOperation::Read)?;
        inventory_attachment_tree(self, &paths.attachments_dir, ResourceOperation::Read)?;

        let bundle = BundleV2Contents {
            manifest,
            session_state: self.read_json_bounded(&paths.session_state, &mut budget)?,
            station: self.read_json_bounded(&paths.station, &mut budget)?,
            antennas: self.read_json_bounded(&paths.antennas, &mut budget)?,
            schedule: self.read_json_bounded(&paths.schedule, &mut budget)?,
            events: self.read_jsonl_bounded(&paths.events, &mut budget)?,
            observations: self.read_jsonl_bounded(&paths.observations, &mut budget)?,
            adapter_records: self.read_jsonl_bounded(&paths.adapter_records, &mut budget)?,
            rig: self.read_jsonl_bounded(&paths.rig, &mut budget)?,
            propagation: self.read_jsonl_bounded(&paths.propagation, &mut budget)?,
            analysis: self.read_json_bounded(&paths.analysis, &mut budget)?,
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
        self.write_v2_files(bundle, BundleValidationProfile::StrictCreation)
    }

    pub fn write_v2_with_attachments(
        &self,
        bundle: &BundleV2Contents,
        attachments: &[BundleAttachment],
    ) -> Result<(), BundleStoreError> {
        self.write_v2_files(bundle, BundleValidationProfile::StrictCreation)?;
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

    pub(super) fn write_v2_for_upgrade(
        &self,
        bundle: &BundleV2Contents,
    ) -> Result<(), BundleStoreError> {
        self.write_v2_files(bundle, BundleValidationProfile::Upgrade)
    }

    fn write_v2_files(
        &self,
        bundle: &BundleV2Contents,
        profile: BundleValidationProfile,
    ) -> Result<(), BundleStoreError> {
        let mut report = validate_bundle_report(&bundle.clone().into_current().bundle);
        report.extend(validate_v2_event_model(bundle));
        if !report.allows(profile) {
            return Err(antennabench_core::BundleValidationError::from_report(report).into());
        }
        let mut adapter_ids = HashSet::new();
        for record in &bundle.adapter_records {
            if validate_machine_identity(&record.record_id).is_err() {
                return Err(BundleStoreError::InvalidV2Bundle {
                    message: format!(
                        "adapter record identity {:?} must be nonempty ASCII and at most 128 bytes",
                        record.record_id
                    ),
                });
            }
            if !adapter_ids.insert(record.record_id.as_str()) {
                return Err(BundleStoreError::InvalidV2Bundle {
                    message: format!(
                        "adapter record identity {:?} is duplicated",
                        record.record_id
                    ),
                });
            }
        }
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
        let mut budget = ModeledBudget::default();
        let manifest =
            serialize_root_bounded(self, &paths.manifest, &bundle.manifest, &mut budget)?;
        let session_state = serialize_root_bounded(
            self,
            &paths.session_state,
            &bundle.session_state,
            &mut budget,
        )?;
        let station = serialize_root_bounded(self, &paths.station, &bundle.station, &mut budget)?;
        let antennas =
            serialize_root_bounded(self, &paths.antennas, &bundle.antennas, &mut budget)?;
        let schedule =
            serialize_root_bounded(self, &paths.schedule, &bundle.schedule, &mut budget)?;
        let events = serialize_jsonl_bounded(self, &paths.events, &bundle.events, &mut budget)?;
        let observations =
            serialize_jsonl_bounded(self, &paths.observations, &bundle.observations, &mut budget)?;
        let adapter_records = serialize_jsonl_bounded(
            self,
            &paths.adapter_records,
            &bundle.adapter_records,
            &mut budget,
        )?;
        let rig = serialize_jsonl_bounded(self, &paths.rig, &bundle.rig, &mut budget)?;
        let propagation =
            serialize_jsonl_bounded(self, &paths.propagation, &bundle.propagation, &mut budget)?;
        let analysis =
            serialize_root_bounded(self, &paths.analysis, &bundle.analysis, &mut budget)?;
        create_directory(self.root())?;
        let result = (|| {
            for (path, bytes) in [
                (&paths.manifest, manifest.as_slice()),
                (&paths.session_state, session_state.as_slice()),
                (&paths.station, station.as_slice()),
                (&paths.antennas, antennas.as_slice()),
                (&paths.schedule, schedule.as_slice()),
                (&paths.events, events.as_slice()),
                (&paths.observations, observations.as_slice()),
                (&paths.adapter_records, adapter_records.as_slice()),
                (&paths.rig, rig.as_slice()),
                (&paths.propagation, propagation.as_slice()),
                (&paths.analysis, analysis.as_slice()),
            ] {
                fs::write(path, bytes).map_err(|source| BundleStoreError::Write {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
            create_directory(&paths.attachments_dir)?;

            let diagnostics = validate_v2_bundle(self, bundle, &paths, false);
            let blocking = diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.blocks(profile))
                .collect::<Vec<_>>();
            if !blocking.is_empty() {
                return Err(BundleStoreError::InvalidV2Bundle {
                    message: blocking
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
        let mut budget = ModeledBudget::default();
        let manifest: BundleManifestV2 =
            self.read_json_bounded(&self.bundle_path("manifest.json")?, &mut budget)?;
        let paths = self.v2_paths(&manifest.files)?;
        let digest_dir = paths.attachments_dir.join("sha256");
        let path = digest_dir.join(digest);
        let path_exists = fs::symlink_metadata(&path).is_ok();
        let additional_entries = u64::from(!path_exists) + u64::from(!digest_dir.exists());
        preflight_attachment_write(
            self,
            &paths.attachments_dir,
            &path,
            reference.byte_size,
            additional_entries,
        )?;
        create_directory(&digest_dir)?;
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let result = bytes.chunks(64 * 1024).try_for_each(|chunk| {
                    self.check_cancelled(ResourceOperation::Write, ResourceStage::Stream, &path)?;
                    file.write_all(chunk)
                        .map_err(|source| BundleStoreError::Write {
                            path: path.clone(),
                            source,
                        })
                });
                if let Err(error) = result {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = read_bounded(
                    self,
                    &path,
                    self.profile().attachment_file_bytes,
                    "resource.attachments.file_bytes",
                    ResourceOperation::Write,
                )?;
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
        let mut budget = ModeledBudget::default();
        let manifest: BundleManifestV2 =
            self.read_json_bounded(&self.bundle_path("manifest.json")?, &mut budget)?;
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
        let bytes = read_bounded(
            self,
            &path,
            self.profile().attachment_file_bytes,
            "resource.attachments.file_bytes",
            ResourceOperation::Read,
        )?;
        if u64::try_from(bytes.len()).ok() != Some(reference.byte_size)
            || sha256_hex(&bytes) != reference.sha256
        {
            return Err(BundleStoreError::AttachmentMismatch { path });
        }
        Ok(bytes)
    }

    fn read_json_bounded<T: DeserializeOwned>(
        &self,
        path: &Path,
        budget: &mut ModeledBudget,
    ) -> Result<T, BundleStoreError> {
        let contents = self.read_root_json(path, budget, ResourceOperation::Read)?;
        serde_json::from_str(&contents).map_err(|source| BundleStoreError::ParseJson {
            path: path.to_path_buf(),
            source,
        })
    }

    fn read_jsonl_bounded<T: DeserializeOwned>(
        &self,
        path: &Path,
        budget: &mut ModeledBudget,
    ) -> Result<Vec<T>, BundleStoreError> {
        let mut records = Vec::new();
        self.for_each_jsonl(path, budget, ResourceOperation::Read, |_, _, line| {
            records.push(serde_json::from_str(line).map_err(|source| {
                BundleStoreError::ParseJson {
                    path: path.to_path_buf(),
                    source,
                }
            })?);
            Ok(())
        })?;
        Ok(records)
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
    encode_lower_hex(Sha256::digest(bytes))
}

pub(super) fn encode_lower_hex(bytes: impl IntoIterator<Item = u8>) -> String {
    let bytes = bytes.into_iter();
    let mut output = String::with_capacity(bytes.size_hint().0 * 2);
    for byte in bytes {
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
    store: &BundleStore,
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
    verify_attachments: bool,
) -> Vec<BundleDiagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(duplicate_member_diagnostics(store, paths));
    diagnostics.extend(validate_v2_event_model(bundle));
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
        if let Err(error) = validate_machine_identity(&record.record_id) {
            diagnostics.push(BundleDiagnostic {
                code: if record.record_id.is_empty() {
                    codes::EMPTY_IDENTITY
                } else {
                    codes::INVALID_IDENTITY
                }
                .into(),
                category: BundleDiagnosticCategory::Semantic,
                severity: BundleDiagnosticSeverity::Warning,
                blocked_operations: vec![
                    BundleValidationProfile::StrictCreation,
                    BundleValidationProfile::Upgrade,
                ],
                location: BundleDiagnosticLocation {
                    file: BundleFileRole::AdapterRecords,
                    record_kind: Some(BundleRecordKind::AdapterRecord),
                    record_id: Some(record.record_id.clone()),
                    record_index: None,
                    physical_line: None,
                    field_path: Some("/record_id".into()),
                },
                message: format!("adapter record identity is invalid: {error}"),
                related_locations: Vec::new(),
            });
        }
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
                && store.read_attachment(attachment).is_err()
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

    diagnostics.extend(validate_checkpoint(store, bundle, paths));
    diagnostics
}

fn duplicate_member_diagnostics(
    store: &BundleStore,
    paths: &ResolvedBundlePathsV2,
) -> Vec<BundleDiagnostic> {
    let mut diagnostics = Vec::new();
    for (file, path) in [
        (BundleFileRole::Manifest, paths.manifest.as_path()),
        (BundleFileRole::SessionState, paths.session_state.as_path()),
        (BundleFileRole::Station, paths.station.as_path()),
        (BundleFileRole::Antennas, paths.antennas.as_path()),
        (BundleFileRole::Schedule, paths.schedule.as_path()),
        (BundleFileRole::Analysis, paths.analysis.as_path()),
    ] {
        if let Ok(bytes) = read_bounded(
            store,
            path,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Inspect,
        ) {
            let Ok(contents) = std::str::from_utf8(&bytes) else {
                continue;
            };
            if let Ok(duplicates) = scan_duplicate_members(contents) {
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
        if let Ok(bytes) = read_bounded(
            store,
            path,
            store.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            ResourceOperation::Inspect,
        ) {
            let Ok(contents) = std::str::from_utf8(&bytes) else {
                continue;
            };
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
    store: &BundleStore,
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
        if let Ok(bytes) = read_bounded(
            store,
            path,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Inspect,
        ) {
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
        if let Ok(bytes) = read_bounded(
            store,
            path,
            store.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            ResourceOperation::Inspect,
        ) {
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

fn validate_v2_event_model(bundle: &BundleV2Contents) -> Vec<BundleDiagnostic> {
    let reduction =
        antennabench_core::reduce_operator_events_v2(SessionLifecycleV2::Ready, &bundle.events);
    let mut diagnostics = reduction
        .diagnostics
        .iter()
        .map(|event_diagnostic| BundleDiagnostic {
            code: codes::V2_EVENT_SEMANTICS.into(),
            category: BundleDiagnosticCategory::Semantic,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: WRITE_OPERATIONS.to_vec(),
            location: BundleDiagnosticLocation {
                file: BundleFileRole::Events,
                record_kind: Some(BundleRecordKind::OperatorEvent),
                record_id: Some(event_diagnostic.event_id.clone()),
                record_index: bundle
                    .events
                    .iter()
                    .position(|event| event.event_id == event_diagnostic.event_id),
                physical_line: None,
                field_path: None,
            },
            message: event_diagnostic.message.clone(),
            related_locations: event_diagnostic
                .related_event_id
                .iter()
                .map(|event_id| BundleDiagnosticLocation {
                    file: BundleFileRole::Events,
                    record_kind: Some(BundleRecordKind::OperatorEvent),
                    record_id: Some(event_id.clone()),
                    record_index: bundle
                        .events
                        .iter()
                        .position(|event| event.event_id == *event_id),
                    physical_line: None,
                    field_path: None,
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    if reduction.lifecycle != bundle.session_state.lifecycle {
        diagnostics.push(BundleDiagnostic {
            code: codes::V2_LIFECYCLE_MISMATCH.into(),
            category: BundleDiagnosticCategory::Structural,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
            location: BundleDiagnosticLocation::file(BundleFileRole::SessionState),
            message: format!(
                "checkpoint lifecycle {:?} does not match append-ordered event lifecycle {:?}",
                bundle.session_state.lifecycle, reduction.lifecycle
            ),
            related_locations: Vec::new(),
        });
    }

    let antenna_labels = bundle
        .antennas
        .antennas
        .iter()
        .map(|antenna| antenna.label.as_str())
        .collect::<HashSet<_>>();
    let mut slot_facts = HashMap::<&str, Vec<&str>>::new();
    for event in &reduction.effective_events {
        if let CorrectableOperatorEventPayloadV2::AntennaStateConfirmed { antenna_label, .. } =
            &event.payload
        {
            if !antenna_labels.contains(antenna_label.as_str()) {
                diagnostics.push(BundleDiagnostic {
                    code: codes::UNKNOWN_ANTENNA_LABEL.into(),
                    category: BundleDiagnosticCategory::Structural,
                    severity: BundleDiagnosticSeverity::Error,
                    blocked_operations: ANALYSIS_AND_WRITE_OPERATIONS.to_vec(),
                    location: BundleDiagnosticLocation {
                        file: BundleFileRole::Events,
                        record_kind: Some(BundleRecordKind::OperatorEvent),
                        record_id: Some(event.source_event_id.clone()),
                        record_index: bundle
                            .events
                            .iter()
                            .position(|candidate| candidate.event_id == event.source_event_id),
                        physical_line: None,
                        field_path: Some("/payload/antenna_label".into()),
                    },
                    message: format!(
                        "actual antenna label {antenna_label:?} is not defined by antennas.json"
                    ),
                    related_locations: Vec::new(),
                });
            }
        }
        if !matches!(
            event.payload,
            CorrectableOperatorEventPayloadV2::NoteAdded { .. }
        ) {
            if let Some(slot_id) = event.slot_id.as_deref() {
                slot_facts
                    .entry(slot_id)
                    .or_default()
                    .push(event.source_event_id.as_str());
            }
        }
    }
    for (slot_id, event_ids) in slot_facts {
        if event_ids.len() < 2 {
            continue;
        }
        diagnostics.push(BundleDiagnostic {
            code: codes::V2_EVENT_CONFLICT.into(),
            category: BundleDiagnosticCategory::Eligibility,
            severity: BundleDiagnosticSeverity::Warning,
            blocked_operations: vec![BundleValidationProfile::Analysis],
            location: BundleDiagnosticLocation {
                file: BundleFileRole::Events,
                record_kind: None,
                record_id: None,
                record_index: None,
                physical_line: None,
                field_path: Some(format!("/slot/{slot_id}")),
            },
            message: format!(
                "slot {slot_id:?} has competing active operator facts and is conservatively excluded"
            ),
            related_locations: event_ids
                .into_iter()
                .map(|event_id| BundleDiagnosticLocation {
                    file: BundleFileRole::Events,
                    record_kind: Some(BundleRecordKind::OperatorEvent),
                    record_id: Some(event_id.to_string()),
                    record_index: bundle
                        .events
                        .iter()
                        .position(|event| event.event_id == event_id),
                    physical_line: None,
                    field_path: None,
                })
                .collect(),
        });
    }

    diagnostics
}
