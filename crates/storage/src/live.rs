use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use antennabench_core::{
    reduce_operator_events_v3, validate_bundle_report, validate_lifecycle_transition_v2,
    validate_machine_identity, validate_operator_event_append_v2, AdapterRecordV2, AnalysisFile,
    AntennasFile, AttachmentReference, BundleManifestV2, BundleV2Contents, BundleV3Contents,
    BundleValidationProfile, EventTimeBasisV2, MutationMember, NormalizedRecordKind,
    ObservationRecordV2, OperatorEventPayloadV2, OperatorEventPayloadV3, OperatorEventV2,
    OperatorEventV3, PlanGenerationV2, PropagationRecordV2, Provenance, RecordMetaV2, RecordMetaV3,
    RecordSource, RigRecordV2, RigRecordV3, Schedule, SessionLifecycleV2, SessionStateV2, Station,
    StreamCheckpointV2, SCHEMA_VERSION_V2, SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    resource::{read_bounded, ResourceOperation},
    v2::{checkpoint_for_bytes, serialize_json, sha256_hex, ResolvedBundlePathsV2},
    BundleStore, BundleStoreError,
};

const LOCK_FILE: &str = ".antennabench.lock";
const CHECKPOINT_TEMP: &str = ".session-state.next.json";
const CHECKPOINT_PREVIOUS: &str = "session-state.previous.json";
const CHECKPOINT_MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LiveStreamV2 {
    AdapterRecords,
    Observations,
    Events,
    Rig,
    Propagation,
}

impl LiveStreamV2 {
    fn checkpoint_name(self) -> &'static str {
        match self {
            Self::AdapterRecords => "adapter_records",
            Self::Observations => "observations",
            Self::Events => "events",
            Self::Rig => "rig",
            Self::Propagation => "propagation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivePlanFile {
    Station,
    Antennas,
    Schedule,
    GenerationMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivePersistencePoint {
    BeforePlanWrite(LivePlanFile),
    AfterPlanWrite(LivePlanFile),
    BeforePlanSync(LivePlanFile),
    AfterPlanSync(LivePlanFile),
    BeforeStreamWrite(LiveStreamV2),
    MidStreamWrite(LiveStreamV2),
    AfterStreamWrite(LiveStreamV2),
    BeforeStreamSync(LiveStreamV2),
    AfterStreamSync(LiveStreamV2),
    BeforeCheckpointWrite,
    AfterCheckpointWrite,
    BeforeCheckpointSync,
    AfterCheckpointSync,
    BeforeCheckpointReplace,
    AfterCheckpointReplace,
    BeforeDirectorySync,
    AfterDirectorySync,
    BeforeCheckpointVerify,
    AfterCheckpointVerify,
    BeforeAcknowledge,
}

pub trait LivePersistenceHooks: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn new_id(&self, kind: &str) -> String;
    fn check(&self, _point: LivePersistencePoint) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct SystemLivePersistenceHooks;

impl LivePersistenceHooks for SystemLivePersistenceHooks {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn new_id(&self, kind: &str) -> String {
        format!("{kind}-{}", Uuid::new_v4())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiveMutationMemberV2 {
    Event(OperatorEventV2),
    AdapterRecord(AdapterRecordV2),
    Observation(ObservationRecordV2),
    Rig(RigRecordV2),
    Propagation(PropagationRecordV2),
}

impl LiveMutationMemberV2 {
    fn stream(&self) -> LiveStreamV2 {
        match self {
            Self::Event(_) => LiveStreamV2::Events,
            Self::AdapterRecord(_) => LiveStreamV2::AdapterRecords,
            Self::Observation(_) => LiveStreamV2::Observations,
            Self::Rig(_) => LiveStreamV2::Rig,
            Self::Propagation(_) => LiveStreamV2::Propagation,
        }
    }

    fn record_id(&self) -> &str {
        match self {
            Self::Event(record) => &record.event_id,
            Self::AdapterRecord(record) => &record.record_id,
            Self::Observation(record) => &record.observation_id,
            Self::Rig(record) => &record.record_id,
            Self::Propagation(record) => &record.record_id,
        }
    }

    fn meta(&self) -> &RecordMetaV2 {
        match self {
            Self::Event(record) => &record.meta,
            Self::AdapterRecord(record) => &record.meta,
            Self::Observation(record) => &record.meta,
            Self::Rig(record) => &record.meta,
            Self::Propagation(record) => &record.meta,
        }
    }

    fn meta_mut(&mut self) -> &mut RecordMetaV2 {
        match self {
            Self::Event(record) => &mut record.meta,
            Self::AdapterRecord(record) => &mut record.meta,
            Self::Observation(record) => &mut record.meta,
            Self::Rig(record) => &mut record.meta,
            Self::Propagation(record) => &mut record.meta,
        }
    }

    fn append_to(self, bundle: &mut BundleV2Contents) {
        match self {
            Self::Event(record) => bundle.events.push(record),
            Self::AdapterRecord(record) => bundle.adapter_records.push(record),
            Self::Observation(record) => bundle.observations.push(record),
            Self::Rig(record) => bundle.rig.push(record),
            Self::Propagation(record) => bundle.propagation.push(record),
        }
    }

    fn serialized_line(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = match self {
            Self::Event(record) => serde_json::to_vec(record)?,
            Self::AdapterRecord(record) => serde_json::to_vec(record)?,
            Self::Observation(record) => serde_json::to_vec(record)?,
            Self::Rig(record) => serde_json::to_vec(record)?,
            Self::Propagation(record) => serde_json::to_vec(record)?,
        };
        bytes.push(b'\n');
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveMutationV2 {
    pub expected_revision: u64,
    pub mutation_id: String,
    pub members: Vec<LiveMutationMemberV2>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanCommitV2 {
    pub expected_revision: u64,
    pub generation_id: String,
    pub station: Station,
    pub antennas: AntennasFile,
    pub schedule: Schedule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitReceiptV2 {
    pub revision: u64,
    pub mutation_id: String,
    pub idempotent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryArtifactV2 {
    pub source: String,
    pub committed_offset: u64,
    pub diagnosis: String,
    pub raw_attachment: AttachmentReference,
    pub metadata_attachment: AttachmentReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDispositionV2 {
    Clean,
    RolledForward,
    RolledBack,
    IdempotentTailRemoved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReportV2 {
    pub starting_revision: u64,
    pub recovered_revision: u64,
    pub final_revision: u64,
    pub disposition: RecoveryDispositionV2,
    pub artifacts: Vec<RecoveryArtifactV2>,
    pub interruption: Option<CommitReceiptV2>,
}

#[derive(Debug, Serialize)]
struct RecoveryArtifactMetadataV2<'a> {
    schema_version: u16,
    session_id: &'a str,
    source: &'a str,
    committed_offset: u64,
    detected_at: DateTime<Utc>,
    diagnosis: &'a str,
    raw_attachment: &'a AttachmentReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PlanGenerationMetadataV2 {
    schema_version: u16,
    session_id: String,
    base_revision: u64,
    generation: PlanGenerationV2,
}

#[derive(Debug, Error)]
pub enum LivePersistenceError {
    #[error(transparent)]
    Store(#[from] BundleStoreError),
    #[error("another writer owns the schema-v2 session lock")]
    WriterBusy,
    #[error("live mutation capability is unavailable: {message}")]
    Capability { message: String },
    #[error("expected checkpoint revision {expected}, but current revision is {actual}")]
    StaleRevision { expected: u64, actual: u64 },
    #[error("live session requires recovery before mutation: {message}")]
    RecoveryRequired { message: String },
    #[error("external modification froze live mutation: {message}")]
    ExternalModification { message: String },
    #[error("invalid live mutation: {message}")]
    InvalidMutation { message: String },
    #[error("mutation ID {mutation_id} is already committed with different content")]
    MutationConflict { mutation_id: String },
    #[error("the active plan is frozen in lifecycle {lifecycle:?}")]
    PlanFrozen { lifecycle: SessionLifecycleV2 },
    #[error("live persistence {operation} failed for {path}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("new checkpoint could not be verified: {message}")]
    CheckpointVerification { message: String },
}

pub struct LiveSessionV2 {
    store: BundleStore,
    _lock: File,
    hooks: Arc<dyn LivePersistenceHooks>,
    bundle: BundleV2Contents,
    paths: ResolvedBundlePathsV2,
    frozen: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveEventMutationV3 {
    pub expected_revision: u64,
    pub mutation_id: String,
    pub event: OperatorEventV3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveEvidenceMutationV3 {
    pub expected_revision: u64,
    pub mutation_id: String,
    pub adapter_records: Vec<AdapterRecordV2>,
    pub observations: Vec<ObservationRecordV2>,
}

/// One schema-v5 antenna-control checkpoint. Successful command-authorized
/// readiness uses two rig records plus the armed event; failed attempts omit
/// the event and therefore cannot change antenna occupancy.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveAntennaControlMutationV5 {
    pub expected_revision: u64,
    pub mutation_id: String,
    pub rig_records: Vec<RigRecordV3>,
    pub armed_event: Option<OperatorEventV3>,
}

pub struct LiveSessionV3 {
    store: BundleStore,
    _lock: File,
    hooks: Arc<dyn LivePersistenceHooks>,
    bundle: BundleV3Contents,
    paths: ResolvedBundlePathsV2,
    frozen: bool,
}

impl std::fmt::Debug for LiveSessionV3 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LiveSessionV3")
            .field("root", &self.store.root())
            .field("revision", &self.bundle.session_state.revision)
            .field("lifecycle", &self.bundle.session_state.lifecycle)
            .field("frozen", &self.frozen)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for LiveSessionV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LiveSessionV2")
            .field("root", &self.store.root())
            .field("revision", &self.bundle.session_state.revision)
            .field("lifecycle", &self.bundle.session_state.lifecycle)
            .field("frozen", &self.frozen)
            .finish_non_exhaustive()
    }
}

impl BundleStore {
    pub fn read_v2_checkpointed(&self) -> Result<BundleV2Contents, LivePersistenceError> {
        let lock_path = self.root().join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| live_io("open snapshot lock", &lock_path, source))?;
        match lock.try_lock_shared() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
            Err(fs::TryLockError::Error(source)) => {
                return Err(LivePersistenceError::Capability {
                    message: format!("shared OS file locking failed: {source}"),
                })
            }
        }
        load_checkpointed_bundle(self)
    }

    pub fn export_v2_checkpointed_to(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<BundleStore, LivePersistenceError> {
        let lock_path = self.root().join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| live_io("open checkpointed export lock", &lock_path, source))?;
        match lock.try_lock_shared() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
            Err(fs::TryLockError::Error(source)) => {
                return Err(LivePersistenceError::Capability {
                    message: format!("shared export locking failed: {source}"),
                })
            }
        }

        let bundle = load_checkpointed_bundle(self)?;
        let source_paths =
            self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        let destination_store = BundleStore::new(destination);
        let referenced = bundle
            .adapter_records
            .iter()
            .filter_map(|record| match &record.input {
                antennabench_core::AdapterInput::Attachment { attachment } => {
                    Some((attachment.sha256.clone(), attachment.clone()))
                }
                antennabench_core::AdapterInput::Inline { .. } => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut referenced_attachments = Vec::new();
        for reference in referenced.into_values() {
            referenced_attachments.push(crate::v2::BundleAttachment {
                bytes: self.read_attachment(&reference)?,
                reference,
            });
        }
        let mut destination_created = false;
        let result = (|| {
            if referenced_attachments.is_empty() {
                destination_store.write_v2(&bundle)?;
            } else {
                destination_store.write_v2_with_attachments(&bundle, &referenced_attachments)?;
            }
            destination_created = true;
            copy_checkpointed_attachments(
                self,
                &source_paths.attachments_dir,
                &destination_store
                    .v2_paths(&bundle.manifest.files)?
                    .attachments_dir,
            )?;
            destination_store.read_v2()?;
            Ok(())
        })();
        if let Err(error) = result {
            if destination_created {
                let _ = fs::remove_dir_all(destination_store.root());
            }
            return Err(error);
        }
        Ok(destination_store)
    }

    pub fn export_v3_checkpointed_to(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<BundleStore, LivePersistenceError> {
        let lock_path = self.root().join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| live_io("open checkpointed export lock", &lock_path, source))?;
        match lock.try_lock_shared() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
            Err(fs::TryLockError::Error(source)) => {
                return Err(LivePersistenceError::Capability {
                    message: format!("shared export locking failed: {source}"),
                });
            }
        }

        let bundle = self.read_v3()?;
        let source_paths =
            self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        verify_exact_checkpoint(self, &bundle.session_state, &source_paths)?;
        let destination_store = BundleStore::new(destination);
        let mut destination_created = false;
        let result = (|| {
            destination_store.write_v3_for_upgrade(&bundle)?;
            destination_created = true;
            copy_checkpointed_attachments(
                self,
                &source_paths.attachments_dir,
                &destination_store
                    .v2_paths(&bundle.manifest.files)?
                    .attachments_dir,
            )?;
            destination_store.read_v3()?;
            Ok(())
        })();
        if let Err(error) = result {
            if destination_created {
                let _ = fs::remove_dir_all(destination_store.root());
            }
            return Err(error);
        }
        Ok(destination_store)
    }

    /// Creates a new checkpointed schema-v2 bundle without exposing partial
    /// durable state at the selected destination.
    ///
    /// The complete bundle is written, synchronized, reopened, and checked
    /// through the live-writer capability boundary in a sibling directory.
    /// Only then is that directory published at the requested path.
    pub fn create_v2_checkpointed(
        &self,
        bundle: &BundleV2Contents,
    ) -> Result<(), LivePersistenceError> {
        if fs::symlink_metadata(self.root()).is_ok() {
            return Err(BundleStoreError::DestinationExists {
                path: self.root().to_path_buf(),
            }
            .into());
        }

        let parent = self
            .root()
            .parent()
            .ok_or_else(|| LivePersistenceError::Capability {
                message: "a new live bundle requires a parent directory".into(),
            })?;
        let parent_metadata = fs::metadata(parent)
            .map_err(|source| live_io("inspect creation parent", parent, source))?;
        if !parent_metadata.is_dir() {
            return Err(LivePersistenceError::Capability {
                message: format!("creation parent {} is not a directory", parent.display()),
            });
        }

        let staging = parent.join(format!(
            ".antennabench-creating-{}.session.antennabundle",
            Uuid::new_v4().simple()
        ));
        let staging_store = BundleStore::new(&staging);
        let result = (|| {
            staging_store.write_v2(bundle)?;
            let paths = staging_store.v2_paths(&bundle.manifest.files)?;
            for path in paths.root_files() {
                sync_regular_file(path)
                    .map_err(|source| live_io("synchronize new bundle file", path, source))?;
            }
            sync_directory(&staging)
                .map_err(|source| live_io("synchronize new bundle directory", &staging, source))?;

            // Opening the writer exercises the exact lock, replacement, and
            // synchronization capability required by later mutations.
            drop(staging_store.open_v2_writer()?);
            let reopened = staging_store.read_v2_checkpointed()?;
            if reopened != *bundle {
                return Err(LivePersistenceError::CheckpointVerification {
                    message: "newly created checkpointed bundle differs after reopen".into(),
                });
            }

            let lock_path = staging.join(LOCK_FILE);
            remove_file_if_present(&lock_path)
                .map_err(|source| live_io("remove creation lock", &lock_path, source))?;
            sync_directory(&staging)
                .map_err(|source| live_io("synchronize creation lock cleanup", &staging, source))?;

            if fs::symlink_metadata(self.root()).is_ok() {
                return Err(BundleStoreError::DestinationExists {
                    path: self.root().to_path_buf(),
                }
                .into());
            }
            publish_new_bundle(&staging, self.root())
                .map_err(|source| live_io("publish new bundle", self.root(), source))?;
            sync_directory(parent)
                .map_err(|source| live_io("synchronize creation parent", parent, source))?;
            Ok(())
        })();

        if result.is_err() && staging.exists() {
            fs::remove_dir_all(&staging)
                .map_err(|source| live_io("clean up failed bundle creation", &staging, source))?;
            sync_directory(parent)
                .map_err(|source| live_io("synchronize failed creation cleanup", parent, source))?;
        }
        result
    }

    /// Creates and durably publishes a new checkpointed schema-v3/v4-family bundle.
    pub fn create_v3_checkpointed(
        &self,
        bundle: &BundleV3Contents,
    ) -> Result<(), LivePersistenceError> {
        if fs::symlink_metadata(self.root()).is_ok() {
            return Err(BundleStoreError::DestinationExists {
                path: self.root().to_path_buf(),
            }
            .into());
        }
        let parent = self
            .root()
            .parent()
            .ok_or_else(|| LivePersistenceError::Capability {
                message: "a new live bundle requires a parent directory".into(),
            })?;
        if !fs::metadata(parent)
            .map_err(|source| live_io("inspect creation parent", parent, source))?
            .is_dir()
        {
            return Err(LivePersistenceError::Capability {
                message: format!("creation parent {} is not a directory", parent.display()),
            });
        }
        let staging = parent.join(format!(
            ".antennabench-creating-{}.session.antennabundle",
            Uuid::new_v4().simple()
        ));
        let staging_store = BundleStore::new(&staging);
        let result = (|| {
            staging_store.write_v3(bundle)?;
            let paths = staging_store.v2_paths(&bundle.manifest.files)?;
            for path in paths.root_files() {
                sync_regular_file(path)
                    .map_err(|source| live_io("synchronize new bundle file", path, source))?;
            }
            sync_directory(&staging)
                .map_err(|source| live_io("synchronize new bundle directory", &staging, source))?;
            drop(staging_store.open_v3_writer()?);
            if staging_store.read_v3_checkpointed()? != *bundle {
                return Err(LivePersistenceError::CheckpointVerification {
                    message: "new schema-v3/v4-family bundle differs after checkpointed reopen"
                        .into(),
                });
            }
            let lock_path = staging.join(LOCK_FILE);
            remove_file_if_present(&lock_path)
                .map_err(|source| live_io("remove creation lock", &lock_path, source))?;
            sync_directory(&staging)
                .map_err(|source| live_io("synchronize creation lock cleanup", &staging, source))?;
            if fs::symlink_metadata(self.root()).is_ok() {
                return Err(BundleStoreError::DestinationExists {
                    path: self.root().to_path_buf(),
                }
                .into());
            }
            publish_new_bundle(&staging, self.root())
                .map_err(|source| live_io("publish new bundle", self.root(), source))?;
            sync_directory(parent)
                .map_err(|source| live_io("synchronize creation parent", parent, source))?;
            Ok(())
        })();
        if result.is_err() && staging.exists() {
            fs::remove_dir_all(&staging)
                .map_err(|source| live_io("clean up failed bundle creation", &staging, source))?;
            sync_directory(parent)
                .map_err(|source| live_io("synchronize failed creation cleanup", parent, source))?;
        }
        result
    }

    pub fn read_v3_checkpointed(&self) -> Result<BundleV3Contents, LivePersistenceError> {
        let lock_path = self.root().join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| live_io("open snapshot lock", &lock_path, source))?;
        match lock.try_lock_shared() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
            Err(fs::TryLockError::Error(source)) => {
                return Err(LivePersistenceError::Capability {
                    message: format!("shared OS file locking failed: {source}"),
                });
            }
        }
        let bundle = self.read_v3()?;
        let paths = self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        verify_exact_checkpoint(self, &bundle.session_state, &paths)?;
        Ok(bundle)
    }

    pub fn open_v3_writer(&self) -> Result<LiveSessionV3, LivePersistenceError> {
        self.open_v3_writer_with_hooks(Arc::new(SystemLivePersistenceHooks))
    }

    pub fn open_v3_writer_with_hooks(
        &self,
        hooks: Arc<dyn LivePersistenceHooks>,
    ) -> Result<LiveSessionV3, LivePersistenceError> {
        if !self.root().is_dir() {
            return Err(LivePersistenceError::Capability {
                message: "bundle root is not a regular local directory".into(),
            });
        }
        let lock_path = self.root().join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| live_io("open writer lock", &lock_path, source))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
            Err(fs::TryLockError::Error(source)) => {
                return Err(LivePersistenceError::Capability {
                    message: format!("OS file locking failed: {source}"),
                });
            }
        }
        probe_live_persistence(self.root()).map_err(|source| LivePersistenceError::Capability {
            message: format!("live persistence durability probe failed: {source}"),
        })?;
        let bundle = self.read_v3()?;
        let paths = self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        verify_exact_checkpoint(self, &bundle.session_state, &paths)?;
        Ok(LiveSessionV3 {
            store: self.clone(),
            _lock: lock,
            hooks,
            bundle,
            paths,
            frozen: false,
        })
    }

    pub fn recover_v3(&self) -> Result<RecoveryReportV2, LivePersistenceError> {
        self.recover_v3_with_hooks(Arc::new(SystemLivePersistenceHooks))
    }

    pub fn recover_v3_with_hooks(
        &self,
        hooks: Arc<dyn LivePersistenceHooks>,
    ) -> Result<RecoveryReportV2, LivePersistenceError> {
        if !self.root().is_dir() {
            return Err(LivePersistenceError::Capability {
                message: "bundle root is not a regular local directory".into(),
            });
        }
        let lock_path = self.root().join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| live_io("open recovery lock", &lock_path, source))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
            Err(fs::TryLockError::Error(source)) => {
                return Err(LivePersistenceError::Capability {
                    message: format!("OS file locking failed during recovery: {source}"),
                })
            }
        }

        probe_live_persistence(self.root()).map_err(|source| LivePersistenceError::Capability {
            message: format!("live persistence durability probe failed: {source}"),
        })?;

        let manifest_path = self.root().join("manifest.json");
        let manifest: BundleManifestV2 = read_json_file(self, &manifest_path, "manifest")?;
        if !matches!(
            manifest.schema_version,
            SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5
        ) {
            return Err(LivePersistenceError::Store(
                BundleStoreError::UnsupportedSchemaVersion {
                    actual: manifest.schema_version,
                },
            ));
        }
        let bootstrap = self.v2_paths(&manifest.files)?;
        let checkpoint: SessionStateV2 =
            read_json_file(self, &bootstrap.session_state, "checkpoint")?;
        let starting_revision = checkpoint.revision;
        let paths = self.v2_paths_for_state(&manifest.files, &checkpoint)?;
        verify_committed_prefixes(self, &checkpoint, &paths)?;
        let tails = read_stream_tails(self, &checkpoint, &paths)?;
        let has_tails = tails.iter().any(|tail| !tail.bytes.is_empty());
        if has_tails {
            truncate_tails(&checkpoint, &paths)?;
        }

        let bundle = self.read_v3()?;
        let mut artifacts = if has_tails {
            preserve_tails_for(
                self,
                manifest.schema_version,
                &bundle.manifest.session_id,
                &paths,
                &tails,
                "schema-v3 recovery conservatively rolled back uncommitted stream data",
                hooks.now(),
            )?
        } else {
            Vec::new()
        };
        recover_checkpoint_temp_for(
            self,
            manifest.schema_version,
            &bundle.manifest.session_id,
            &paths,
            &mut artifacts,
            hooks.now(),
        )?;
        verify_exact_checkpoint(self, &bundle.session_state, &paths)?;

        let recovered_revision = bundle.session_state.revision;
        let disposition = if has_tails {
            RecoveryDispositionV2::RolledBack
        } else {
            RecoveryDispositionV2::Clean
        };
        let mut session = LiveSessionV3 {
            store: self.clone(),
            _lock: lock,
            hooks,
            bundle,
            paths,
            frozen: false,
        };
        let interruption = if session.bundle.session_state.lifecycle == SessionLifecycleV2::Running
        {
            let revision = session.bundle.session_state.revision;
            let event_id = session.allocate_id("event");
            let mutation_id = session.allocate_id("mutation");
            let occurred_at = session.hooks.now();
            Some(session.append_event(LiveEventMutationV3 {
                expected_revision: revision,
                mutation_id: mutation_id.clone(),
                event: OperatorEventV3 {
                    meta: RecordMetaV3 {
                        schema_version: manifest.schema_version,
                        session_id: session.bundle.manifest.session_id.clone(),
                        recorded_at: occurred_at,
                        provenance: Provenance::from_legacy(
                            RecordSource::Derived,
                            env!("CARGO_PKG_VERSION"),
                        ),
                        mutation: MutationMember {
                            mutation_id,
                            member_index: 0,
                            member_count: 1,
                        },
                    },
                    event_id,
                    occurred_at,
                    time_basis: EventTimeBasisV2::RecoverySystem,
                    uncertainty_seconds: None,
                    slot_id: None,
                    payload: OperatorEventPayloadV3::InterruptionDetected {
                        reason: Some("recovery opened a session left running".into()),
                    },
                },
            })?)
        } else {
            None
        };
        let final_revision = session.bundle.session_state.revision;
        Ok(RecoveryReportV2 {
            starting_revision,
            recovered_revision,
            final_revision,
            disposition,
            artifacts,
            interruption,
        })
    }

    pub fn open_v2_writer(&self) -> Result<LiveSessionV2, LivePersistenceError> {
        self.open_v2_writer_with_hooks(Arc::new(SystemLivePersistenceHooks))
    }

    pub fn open_v2_writer_with_hooks(
        &self,
        hooks: Arc<dyn LivePersistenceHooks>,
    ) -> Result<LiveSessionV2, LivePersistenceError> {
        if !self.root().is_dir() {
            return Err(LivePersistenceError::Capability {
                message: "bundle root is not a regular local directory".into(),
            });
        }
        let lock_path = self.root().join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| live_io("open writer lock", &lock_path, source))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
            Err(fs::TryLockError::Error(source)) => {
                return Err(LivePersistenceError::Capability {
                    message: format!("OS file locking failed: {source}"),
                })
            }
        }

        probe_live_persistence(self.root()).map_err(|source| LivePersistenceError::Capability {
            message: format!("live persistence durability probe failed: {source}"),
        })?;
        let bundle = self.read_v2()?;
        let paths = self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        verify_exact_checkpoint(self, &bundle.session_state, &paths)?;
        Ok(LiveSessionV2 {
            store: self.clone(),
            _lock: lock,
            hooks,
            bundle,
            paths,
            frozen: false,
        })
    }

    pub fn recover_v2(&self) -> Result<RecoveryReportV2, LivePersistenceError> {
        self.recover_v2_with_hooks(Arc::new(SystemLivePersistenceHooks))
    }

    pub fn recover_v2_with_hooks(
        &self,
        hooks: Arc<dyn LivePersistenceHooks>,
    ) -> Result<RecoveryReportV2, LivePersistenceError> {
        if !self.root().is_dir() {
            return Err(LivePersistenceError::Capability {
                message: "bundle root is not a regular local directory".into(),
            });
        }
        let lock_path = self.root().join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| live_io("open recovery lock", &lock_path, source))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
            Err(fs::TryLockError::Error(source)) => {
                return Err(LivePersistenceError::Capability {
                    message: format!("OS file locking failed during recovery: {source}"),
                })
            }
        }

        probe_live_persistence(self.root()).map_err(|source| LivePersistenceError::Capability {
            message: format!("live persistence durability probe failed: {source}"),
        })?;

        let (mut bundle, restore_selected_checkpoint) = load_recovery_bundle(self)?;
        let starting_revision = bundle.session_state.revision;
        let mut paths = self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        if restore_selected_checkpoint {
            commit_checkpoint(
                self.root(),
                &paths.session_state,
                &bundle.session_state,
                hooks.as_ref(),
            )?;
        }
        let mut artifacts = Vec::new();
        let plan_disposition = recover_pending_plan_generation(
            self,
            &mut bundle,
            &mut paths,
            &mut artifacts,
            hooks.as_ref(),
        )?;
        let tails = read_stream_tails(self, &bundle.session_state, &paths)?;
        let disposition;

        if tails.iter().all(|tail| tail.bytes.is_empty()) {
            disposition = plan_disposition.unwrap_or(RecoveryDispositionV2::Clean);
        } else {
            match parse_tail_mutation(&bundle, &tails) {
                Ok(mutation) => {
                    if let Some(existing) = committed_mutation(&bundle, &mutation.mutation_id) {
                        if !same_business_members(&existing, &mutation.members) {
                            let diagnosis = format!(
                                "tail reuses committed mutation ID {} with conflicting content",
                                mutation.mutation_id
                            );
                            artifacts.extend(preserve_tails(
                                self,
                                &bundle,
                                &paths,
                                &tails,
                                &diagnosis,
                                hooks.now(),
                            )?);
                            truncate_tails(&bundle.session_state, &paths)?;
                            disposition = RecoveryDispositionV2::RolledBack;
                        } else {
                            truncate_tails(&bundle.session_state, &paths)?;
                            disposition = RecoveryDispositionV2::IdempotentTailRemoved;
                        }
                    } else {
                        let next_lifecycle = validate_mutation(&bundle, &mutation)?;
                        for member in mutation.members.clone() {
                            member.append_to(&mut bundle);
                        }
                        let mut next = checkpoint_from_paths(self, &bundle, &paths)?;
                        next.revision = next.revision.checked_add(1).ok_or_else(|| {
                            LivePersistenceError::CheckpointVerification {
                                message: "checkpoint revision overflowed during recovery".into(),
                            }
                        })?;
                        next.lifecycle = next_lifecycle;
                        next.last_committed_mutation_id = Some(mutation.mutation_id);
                        commit_checkpoint(
                            self.root(),
                            &paths.session_state,
                            &next,
                            hooks.as_ref(),
                        )?;
                        bundle.session_state = next;
                        disposition = RecoveryDispositionV2::RolledForward;
                    }
                }
                Err(diagnosis) => {
                    artifacts.extend(preserve_tails(
                        self,
                        &bundle,
                        &paths,
                        &tails,
                        &diagnosis,
                        hooks.now(),
                    )?);
                    truncate_tails(&bundle.session_state, &paths)?;
                    disposition = RecoveryDispositionV2::RolledBack;
                }
            }
        }

        recover_checkpoint_temp(self, &bundle, &paths, &mut artifacts, hooks.now())?;
        let recovered_revision = bundle.session_state.revision;
        paths = self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        verify_exact_checkpoint(self, &bundle.session_state, &paths)?;

        let mut session = LiveSessionV2 {
            store: self.clone(),
            _lock: lock,
            hooks,
            bundle,
            paths,
            frozen: false,
        };
        let interruption = if session.bundle.session_state.lifecycle == SessionLifecycleV2::Running
        {
            let revision = session.bundle.session_state.revision;
            let event_id = session.allocate_id("event");
            let mutation_id = session.allocate_id("mutation");
            let occurred_at = session.hooks.now();
            Some(session.append(LiveMutationV2 {
                expected_revision: revision,
                mutation_id: mutation_id.clone(),
                members: vec![LiveMutationMemberV2::Event(OperatorEventV2 {
                    meta: RecordMetaV2 {
                        schema_version: SCHEMA_VERSION_V2,
                        session_id: session.bundle.manifest.session_id.clone(),
                        recorded_at: occurred_at,
                        provenance: Provenance::from_legacy(
                            RecordSource::Derived,
                            env!("CARGO_PKG_VERSION"),
                        ),
                        mutation: MutationMember {
                            mutation_id,
                            member_index: 0,
                            member_count: 1,
                        },
                    },
                    event_id,
                    occurred_at,
                    time_basis: EventTimeBasisV2::RecoverySystem,
                    uncertainty_seconds: None,
                    slot_id: None,
                    payload: OperatorEventPayloadV2::InterruptionDetected {
                        reason: Some("recovery opened a session left running".into()),
                    },
                })],
            })?)
        } else {
            None
        };
        let final_revision = session.bundle.session_state.revision;
        Ok(RecoveryReportV2 {
            starting_revision,
            recovered_revision,
            final_revision,
            disposition,
            artifacts,
            interruption,
        })
    }
}

impl LiveSessionV2 {
    pub fn checkpoint(&self) -> &SessionStateV2 {
        &self.bundle.session_state
    }

    pub fn snapshot(&self) -> &BundleV2Contents {
        &self.bundle
    }

    pub fn allocate_id(&self, kind: &str) -> String {
        self.hooks.new_id(kind)
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn append(
        &mut self,
        mut mutation: LiveMutationV2,
    ) -> Result<CommitReceiptV2, LivePersistenceError> {
        self.ensure_mutable()?;
        self.refresh_if_advanced()?;

        if let Some(existing) = committed_mutation(&self.bundle, &mutation.mutation_id) {
            if same_business_members(&existing, &mutation.members) {
                return Ok(CommitReceiptV2 {
                    revision: self.bundle.session_state.revision,
                    mutation_id: mutation.mutation_id,
                    idempotent: true,
                });
            }
            return Err(LivePersistenceError::MutationConflict {
                mutation_id: mutation.mutation_id,
            });
        }
        if mutation.expected_revision != self.bundle.session_state.revision {
            return Err(LivePersistenceError::StaleRevision {
                expected: mutation.expected_revision,
                actual: self.bundle.session_state.revision,
            });
        }

        let recorded_at = self.hooks.now();
        prepare_mutation(&mut mutation, &self.bundle.manifest.session_id, recorded_at)?;
        let next_lifecycle = validate_mutation(&self.bundle, &mutation)?;
        self.verify_unchanged()?;

        let mut serialized = mutation
            .members
            .iter()
            .map(|member| Ok((member.stream(), member.serialized_line()?)))
            .collect::<Result<Vec<_>, serde_json::Error>>()
            .map_err(|source| LivePersistenceError::InvalidMutation {
                message: format!("record serialization failed: {source}"),
            })?;
        serialized.sort_by_key(|(stream, _)| *stream);
        preflight_live_budget(&self.store, &self.bundle.session_state, &serialized)?;

        let baseline = self.bundle.session_state.clone();
        let operation = (|| {
            for (stream, bytes) in &serialized {
                append_line(
                    stream_path(&self.paths, *stream),
                    *stream,
                    bytes,
                    self.hooks.as_ref(),
                )?;
            }

            let mut next_bundle = self.bundle.clone();
            for member in mutation.members.clone() {
                member.append_to(&mut next_bundle);
            }
            let mut next = checkpoint_from_paths(&self.store, &next_bundle, &self.paths)?;
            next.revision = baseline.revision.checked_add(1).ok_or_else(|| {
                LivePersistenceError::InvalidMutation {
                    message: "checkpoint revision overflowed".into(),
                }
            })?;
            next.lifecycle = next_lifecycle;
            next.last_committed_mutation_id = Some(mutation.mutation_id.clone());
            commit_checkpoint(
                self.store.root(),
                &self.paths.session_state,
                &next,
                self.hooks.as_ref(),
            )?;
            next_bundle.session_state = next;
            self.bundle = next_bundle;
            Ok(())
        })();

        if let Err(error) = operation {
            if !self.reload_committed_revision(baseline.revision)? {
                self.rollback_streams(&baseline)?;
            }
            return Err(error);
        }

        self.hooks
            .check(LivePersistencePoint::BeforeAcknowledge)
            .map_err(|source| live_io("acknowledge checkpoint", self.store.root(), source))?;
        Ok(CommitReceiptV2 {
            revision: self.bundle.session_state.revision,
            mutation_id: mutation.mutation_id,
            idempotent: false,
        })
    }

    pub fn append_with_attachment(
        &mut self,
        bytes: &[u8],
        media_type: &str,
        encoding: Option<String>,
        container: Option<String>,
        source_locator: Option<String>,
        build_mutation: impl FnOnce(AttachmentReference) -> LiveMutationV2,
    ) -> Result<(AttachmentReference, CommitReceiptV2), LivePersistenceError> {
        self.ensure_mutable()?;
        let digest = sha256_hex(bytes);
        let attachment_path = self.paths.attachments_dir.join("sha256").join(&digest);
        let existed = fs::symlink_metadata(&attachment_path).is_ok();
        let mut attachment =
            durable_attachment(&self.store, &self.paths, bytes, media_type, source_locator)?;
        attachment.encoding = encoding;
        attachment.container = container;
        let mutation = build_mutation(attachment.clone());
        let mutation_id = mutation.mutation_id.clone();
        match self.append(mutation) {
            Ok(receipt) => Ok((attachment, receipt)),
            Err(error) => {
                let committed = self
                    .bundle
                    .session_state
                    .last_committed_mutation_id
                    .as_deref()
                    == Some(mutation_id.as_str());
                if !existed && !committed {
                    remove_file_if_present(&attachment_path).map_err(|source| {
                        live_io("remove uncommitted attachment", &attachment_path, source)
                    })?;
                    if let Some(parent) = attachment_path.parent() {
                        sync_directory(parent).map_err(|source| {
                            live_io("synchronize attachment rollback", parent, source)
                        })?;
                    }
                    sync_directory(&self.paths.attachments_dir).map_err(|source| {
                        live_io(
                            "synchronize attachments rollback",
                            &self.paths.attachments_dir,
                            source,
                        )
                    })?;
                }
                Err(error)
            }
        }
    }

    pub fn commit_plan(
        &mut self,
        plan: PlanCommitV2,
    ) -> Result<CommitReceiptV2, LivePersistenceError> {
        self.ensure_mutable()?;
        self.refresh_if_advanced()?;
        if self.bundle.session_state.active_plan.generation_id == plan.generation_id {
            if self.bundle.station == plan.station
                && self.bundle.antennas == plan.antennas
                && self.bundle.schedule == plan.schedule
            {
                return Ok(CommitReceiptV2 {
                    revision: self.bundle.session_state.revision,
                    mutation_id: format!("plan:{}", plan.generation_id),
                    idempotent: true,
                });
            }
            return Err(LivePersistenceError::MutationConflict {
                mutation_id: format!("plan:{}", plan.generation_id),
            });
        }
        if plan.expected_revision != self.bundle.session_state.revision {
            return Err(LivePersistenceError::StaleRevision {
                expected: plan.expected_revision,
                actual: self.bundle.session_state.revision,
            });
        }
        if !matches!(
            self.bundle.session_state.lifecycle,
            SessionLifecycleV2::Draft | SessionLifecycleV2::Ready
        ) {
            return Err(LivePersistenceError::PlanFrozen {
                lifecycle: self.bundle.session_state.lifecycle,
            });
        }
        validate_generation_id(&plan.generation_id)?;
        self.verify_unchanged()?;

        let station = serialize_json(&plan.station).map_err(invalid_serialization)?;
        let antennas = serialize_json(&plan.antennas).map_err(invalid_serialization)?;
        let schedule = serialize_json(&plan.schedule).map_err(invalid_serialization)?;
        let station_digest = sha256_hex(&station);
        let antennas_digest = sha256_hex(&antennas);
        let schedule_digest = sha256_hex(&schedule);
        let generation = PlanGenerationV2 {
            generation_id: plan.generation_id.clone(),
            station_sha256: station_digest.clone(),
            antennas_sha256: antennas_digest.clone(),
            schedule_sha256: schedule_digest.clone(),
            root_sha256: sha256_hex(
                [station_digest, antennas_digest, schedule_digest]
                    .join("\n")
                    .as_bytes(),
            ),
        };

        let mut candidate = self.bundle.clone();
        candidate.station = plan.station;
        candidate.antennas = plan.antennas;
        candidate.schedule = plan.schedule;
        candidate.session_state.active_plan = generation.clone();
        candidate.session_state.lifecycle = SessionLifecycleV2::Ready;
        let report = validate_bundle_report(&candidate.clone().into_current().bundle);
        if !report.allows(BundleValidationProfile::StrictCreation) {
            return Err(LivePersistenceError::InvalidMutation {
                message: report
                    .blocking_diagnostics(BundleValidationProfile::StrictCreation)
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            });
        }

        let generation_dir = self
            .store
            .root()
            .join("plan-generations")
            .join(&plan.generation_id);
        fs::create_dir_all(&generation_dir)
            .map_err(|source| live_io("create plan generation", &generation_dir, source))?;
        for (kind, path, bytes) in [
            (
                LivePlanFile::Station,
                generation_dir.join("station.json"),
                station,
            ),
            (
                LivePlanFile::Antennas,
                generation_dir.join("antennas.json"),
                antennas,
            ),
            (
                LivePlanFile::Schedule,
                generation_dir.join("schedule.json"),
                schedule,
            ),
        ] {
            write_plan_file(&path, kind, &bytes, self.hooks.as_ref())?;
        }
        let generation_metadata = serialize_json(&PlanGenerationMetadataV2 {
            schema_version: SCHEMA_VERSION_V2,
            session_id: self.bundle.manifest.session_id.clone(),
            base_revision: self.bundle.session_state.revision,
            generation: generation.clone(),
        })
        .map_err(invalid_serialization)?;
        write_plan_file(
            &generation_dir.join("generation.json"),
            LivePlanFile::GenerationMetadata,
            &generation_metadata,
            self.hooks.as_ref(),
        )?;
        sync_directory(&generation_dir)
            .map_err(|source| live_io("synchronize plan generation", &generation_dir, source))?;

        let mut next = self.bundle.session_state.clone();
        next.revision =
            next.revision
                .checked_add(1)
                .ok_or_else(|| LivePersistenceError::InvalidMutation {
                    message: "checkpoint revision overflowed".into(),
                })?;
        next.lifecycle = SessionLifecycleV2::Ready;
        next.active_plan = generation;
        next.last_committed_mutation_id = Some(format!("plan:{}", plan.generation_id));
        let baseline_revision = self.bundle.session_state.revision;
        if let Err(error) = commit_checkpoint(
            self.store.root(),
            &self.paths.session_state,
            &next,
            self.hooks.as_ref(),
        ) {
            if read_state(&self.paths.session_state)
                .is_ok_and(|state| state.revision > baseline_revision)
            {
                self.bundle = self.store.read_v2()?;
                self.paths = self
                    .store
                    .v2_paths_for_state(&self.bundle.manifest.files, &self.bundle.session_state)?;
            }
            return Err(error);
        }
        candidate.session_state = next;
        self.bundle = candidate;
        self.paths = self
            .store
            .v2_paths_for_state(&self.bundle.manifest.files, &self.bundle.session_state)?;
        self.hooks
            .check(LivePersistencePoint::BeforeAcknowledge)
            .map_err(|source| live_io("acknowledge checkpoint", self.store.root(), source))?;
        Ok(CommitReceiptV2 {
            revision: self.bundle.session_state.revision,
            mutation_id: format!("plan:{}", plan.generation_id),
            idempotent: false,
        })
    }

    fn ensure_mutable(&self) -> Result<(), LivePersistenceError> {
        if self.frozen {
            Err(LivePersistenceError::ExternalModification {
                message: "this handle is read-only after a persistence failure".into(),
            })
        } else {
            Ok(())
        }
    }

    fn refresh_if_advanced(&mut self) -> Result<(), LivePersistenceError> {
        let state = read_state(&self.paths.session_state)?;
        if state.revision == self.bundle.session_state.revision {
            return Ok(());
        }
        if state.revision < self.bundle.session_state.revision {
            self.frozen = true;
            return Err(LivePersistenceError::ExternalModification {
                message: "checkpoint revision moved backwards".into(),
            });
        }
        self.bundle = self.store.read_v2()?;
        self.paths = self
            .store
            .v2_paths_for_state(&self.bundle.manifest.files, &self.bundle.session_state)?;
        Ok(())
    }

    fn verify_unchanged(&mut self) -> Result<(), LivePersistenceError> {
        let actual = read_state(&self.paths.session_state)?;
        if actual != self.bundle.session_state {
            self.frozen = true;
            return Err(LivePersistenceError::ExternalModification {
                message: "session-state.json changed since the writer snapshot".into(),
            });
        }
        if let Err(error) = verify_exact_checkpoint(&self.store, &actual, &self.paths) {
            self.frozen = true;
            return Err(error);
        }
        Ok(())
    }

    fn reload_committed_revision(
        &mut self,
        baseline_revision: u64,
    ) -> Result<bool, LivePersistenceError> {
        let state = read_state(&self.paths.session_state)?;
        if state.revision <= baseline_revision {
            return Ok(false);
        }
        self.bundle = self.store.read_v2()?;
        self.paths = self
            .store
            .v2_paths_for_state(&self.bundle.manifest.files, &self.bundle.session_state)?;
        Ok(true)
    }

    fn rollback_streams(&mut self, baseline: &SessionStateV2) -> Result<(), LivePersistenceError> {
        for stream in all_streams() {
            let Some(checkpoint) = baseline.streams.get(stream.checkpoint_name()) else {
                self.frozen = true;
                return Err(LivePersistenceError::CheckpointVerification {
                    message: format!(
                        "baseline checkpoint is missing {}",
                        stream.checkpoint_name()
                    ),
                });
            };
            let path = stream_path(&self.paths, stream);
            let file = OpenOptions::new()
                .write(true)
                .open(path)
                .map_err(|source| live_io("open stream rollback", path, source))?;
            file.set_len(checkpoint.committed_bytes)
                .map_err(|source| live_io("truncate uncommitted stream tail", path, source))?;
            file.sync_all()
                .map_err(|source| live_io("synchronize stream rollback", path, source))?;
        }
        Ok(())
    }
}

impl LiveSessionV3 {
    pub fn checkpoint(&self) -> &SessionStateV2 {
        &self.bundle.session_state
    }

    pub fn snapshot(&self) -> &BundleV3Contents {
        &self.bundle
    }

    pub fn allocate_id(&self, kind: &str) -> String {
        self.hooks.new_id(kind)
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn append_evidence(
        &mut self,
        mut mutation: LiveEvidenceMutationV3,
    ) -> Result<CommitReceiptV2, LivePersistenceError> {
        if self.frozen {
            return Err(LivePersistenceError::ExternalModification {
                message: "this handle is read-only after a persistence failure".into(),
            });
        }
        let actual_state = read_state(&self.paths.session_state)?;
        if actual_state != self.bundle.session_state {
            self.frozen = true;
            return Err(LivePersistenceError::ExternalModification {
                message: "session-state.json changed since the writer snapshot".into(),
            });
        }
        verify_exact_checkpoint(&self.store, &actual_state, &self.paths).inspect_err(|_| {
            self.frozen = true;
        })?;

        if let Some(existing) = v3_committed_evidence(&self.bundle, &mutation.mutation_id) {
            if same_v3_evidence_business_value(&existing, &mutation) {
                return Ok(CommitReceiptV2 {
                    revision: self.bundle.session_state.revision,
                    mutation_id: mutation.mutation_id,
                    idempotent: true,
                });
            }
            return Err(LivePersistenceError::MutationConflict {
                mutation_id: mutation.mutation_id,
            });
        }
        if self
            .bundle
            .events
            .iter()
            .any(|event| event.meta.mutation.mutation_id == mutation.mutation_id)
            || self
                .bundle
                .rig
                .iter()
                .any(|record| record.meta.mutation.mutation_id == mutation.mutation_id)
            || self
                .bundle
                .propagation
                .iter()
                .any(|record| record.meta.mutation.mutation_id == mutation.mutation_id)
        {
            return Err(LivePersistenceError::MutationConflict {
                mutation_id: mutation.mutation_id,
            });
        }
        if mutation.expected_revision != self.bundle.session_state.revision {
            return Err(LivePersistenceError::StaleRevision {
                expected: mutation.expected_revision,
                actual: self.bundle.session_state.revision,
            });
        }
        prepare_v3_evidence(
            &mut mutation,
            &self.bundle.manifest.session_id,
            self.bundle.manifest.schema_version,
            self.hooks.now(),
        )?;
        validate_v3_evidence(&self.bundle, &mutation)?;
        for record in &mutation.adapter_records {
            if let antennabench_core::AdapterInput::Attachment { attachment } = &record.input {
                self.store.read_attachment(attachment)?;
            }
        }

        let mut candidate = self.bundle.clone();
        candidate
            .adapter_records
            .extend(mutation.adapter_records.iter().cloned());
        candidate
            .observations
            .extend(mutation.observations.iter().cloned());
        BundleStore::refresh_v3_checkpoint(&mut candidate)?;
        candidate.session_state.revision = self
            .bundle
            .session_state
            .revision
            .checked_add(1)
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "checkpoint revision overflowed".into(),
            })?;
        candidate.session_state.last_committed_mutation_id = Some(mutation.mutation_id.clone());
        crate::v3::validate_v3_model(&candidate)?;

        let adapter_bytes = serialize_v3_lines(&mutation.adapter_records, "adapter record")?;
        let observation_bytes = serialize_v3_lines(&mutation.observations, "observation")?;
        let serialized = [
            (LiveStreamV2::AdapterRecords, adapter_bytes),
            (LiveStreamV2::Observations, observation_bytes),
        ]
        .into_iter()
        .filter(|(_, bytes)| !bytes.is_empty())
        .collect::<Vec<_>>();
        preflight_live_budget(&self.store, &self.bundle.session_state, &serialized)?;

        let baseline = self.bundle.session_state.clone();
        let operation = (|| {
            for (stream, bytes) in &serialized {
                append_line(
                    stream_path(&self.paths, *stream),
                    *stream,
                    bytes,
                    self.hooks.as_ref(),
                )?;
            }
            commit_checkpoint(
                self.store.root(),
                &self.paths.session_state,
                &candidate.session_state,
                self.hooks.as_ref(),
            )
        })();
        if let Err(error) = operation {
            let committed = read_state(&self.paths.session_state)
                .is_ok_and(|state| state.revision > baseline.revision);
            if committed {
                self.bundle = self.store.read_v3()?;
            } else {
                rollback_v3_streams(
                    &self.paths,
                    &baseline,
                    &[LiveStreamV2::AdapterRecords, LiveStreamV2::Observations],
                )?;
            }
            return Err(error);
        }
        self.bundle = candidate;
        self.hooks
            .check(LivePersistencePoint::BeforeAcknowledge)
            .map_err(|source| live_io("acknowledge checkpoint", self.store.root(), source))?;
        Ok(CommitReceiptV2 {
            revision: self.bundle.session_state.revision,
            mutation_id: mutation.mutation_id,
            idempotent: false,
        })
    }

    pub fn append_antenna_control(
        &mut self,
        mut mutation: LiveAntennaControlMutationV5,
    ) -> Result<CommitReceiptV2, LivePersistenceError> {
        if self.frozen {
            return Err(LivePersistenceError::ExternalModification {
                message: "this handle is read-only after a persistence failure".into(),
            });
        }
        let actual_state = read_state(&self.paths.session_state)?;
        if actual_state != self.bundle.session_state {
            self.frozen = true;
            return Err(LivePersistenceError::ExternalModification {
                message: "session-state.json changed since the writer snapshot".into(),
            });
        }
        verify_exact_checkpoint(&self.store, &actual_state, &self.paths).inspect_err(|_| {
            self.frozen = true;
        })?;
        if self.bundle.session_state.lifecycle != SessionLifecycleV2::Running {
            return Err(LivePersistenceError::InvalidMutation {
                message: "antenna-control evidence may append only to a running session".into(),
            });
        }
        if mutation.rig_records.is_empty() || mutation.rig_records.len() > 2 {
            return Err(LivePersistenceError::InvalidMutation {
                message: "antenna-control mutation requires one or two rig records".into(),
            });
        }
        if validate_machine_identity(&mutation.mutation_id).is_err() {
            return Err(LivePersistenceError::InvalidMutation {
                message: "mutation identity must be bounded nonempty ASCII".into(),
            });
        }
        if let Some(receipt) = committed_v5_antenna_control(&self.bundle, &mutation) {
            return receipt;
        }
        if mutation.expected_revision != self.bundle.session_state.revision {
            return Err(LivePersistenceError::StaleRevision {
                expected: mutation.expected_revision,
                actual: self.bundle.session_state.revision,
            });
        }
        if v3_other_stream_has_mutation(&self.bundle, &mutation.mutation_id)
            || self
                .bundle
                .rig
                .iter()
                .any(|record| record.meta.mutation.mutation_id == mutation.mutation_id)
        {
            return Err(LivePersistenceError::MutationConflict {
                mutation_id: mutation.mutation_id,
            });
        }

        let member_count =
            u32::try_from(mutation.rig_records.len() + usize::from(mutation.armed_event.is_some()))
                .map_err(|_| LivePersistenceError::InvalidMutation {
                    message: "antenna-control mutation has too many members".into(),
                })?;
        let recorded_at = self.hooks.now();
        for (index, record) in mutation.rig_records.iter_mut().enumerate() {
            if validate_machine_identity(&record.record_id).is_err()
                || record.antenna_control.is_none()
            {
                return Err(LivePersistenceError::InvalidMutation {
                    message: "antenna-control rig records require bounded identities and typed invocation evidence".into(),
                });
            }
            record.meta.schema_version = self.bundle.manifest.schema_version;
            record.meta.session_id = self.bundle.manifest.session_id.clone();
            record.meta.recorded_at = recorded_at;
            record.meta.mutation = MutationMember {
                mutation_id: mutation.mutation_id.clone(),
                member_index: u32::try_from(index).expect("at most two rig records"),
                member_count,
            };
        }
        if let Some(event) = &mut mutation.armed_event {
            if validate_machine_identity(&event.event_id).is_err() {
                return Err(LivePersistenceError::InvalidMutation {
                    message: "armed event identity must be bounded nonempty ASCII".into(),
                });
            }
            event.meta.schema_version = self.bundle.manifest.schema_version;
            event.meta.session_id = self.bundle.manifest.session_id.clone();
            event.meta.recorded_at = recorded_at;
            event.meta.mutation = MutationMember {
                mutation_id: mutation.mutation_id.clone(),
                member_index: u32::try_from(mutation.rig_records.len())
                    .expect("at most two rig records"),
                member_count,
            };
        }

        let mut candidate = self.bundle.clone();
        candidate.rig.extend(mutation.rig_records.iter().cloned());
        if let Some(event) = &mutation.armed_event {
            candidate.events.push(event.clone());
            let reduction = reduce_operator_events_v3(SessionLifecycleV2::Ready, &candidate.events);
            if let Some(diagnostic) = reduction.diagnostics.first() {
                return Err(LivePersistenceError::InvalidMutation {
                    message: diagnostic.message.clone(),
                });
            }
            candidate.session_state.lifecycle = reduction.lifecycle;
        }
        BundleStore::refresh_v3_checkpoint(&mut candidate)?;
        candidate.session_state.revision = self
            .bundle
            .session_state
            .revision
            .checked_add(1)
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "checkpoint revision overflowed".into(),
            })?;
        candidate.session_state.last_committed_mutation_id = Some(mutation.mutation_id.clone());
        crate::v3::validate_v3_model(&candidate)?;

        let rig_bytes = serialize_v3_lines(&mutation.rig_records, "rig record")?;
        let event_bytes = mutation
            .armed_event
            .as_ref()
            .map(|event| serialize_v3_lines(std::slice::from_ref(event), "armed event"))
            .transpose()?
            .unwrap_or_default();
        let serialized = [
            (LiveStreamV2::Rig, rig_bytes),
            (LiveStreamV2::Events, event_bytes),
        ]
        .into_iter()
        .filter(|(_, bytes)| !bytes.is_empty())
        .collect::<Vec<_>>();
        preflight_live_budget(&self.store, &self.bundle.session_state, &serialized)?;

        let baseline = self.bundle.session_state.clone();
        let operation = (|| {
            for (stream, bytes) in &serialized {
                append_line(
                    stream_path(&self.paths, *stream),
                    *stream,
                    bytes,
                    self.hooks.as_ref(),
                )?;
            }
            commit_checkpoint(
                self.store.root(),
                &self.paths.session_state,
                &candidate.session_state,
                self.hooks.as_ref(),
            )
        })();
        if let Err(error) = operation {
            let committed = read_state(&self.paths.session_state)
                .is_ok_and(|state| state.revision > baseline.revision);
            if committed {
                self.bundle = self.store.read_v3()?;
            } else {
                rollback_v3_streams(
                    &self.paths,
                    &baseline,
                    &[LiveStreamV2::Rig, LiveStreamV2::Events],
                )?;
            }
            return Err(error);
        }
        self.bundle = candidate;
        self.hooks
            .check(LivePersistencePoint::BeforeAcknowledge)
            .map_err(|source| live_io("acknowledge checkpoint", self.store.root(), source))?;
        Ok(CommitReceiptV2 {
            revision: self.bundle.session_state.revision,
            mutation_id: mutation.mutation_id,
            idempotent: false,
        })
    }

    pub fn append_evidence_with_attachment(
        &mut self,
        bytes: &[u8],
        media_type: &str,
        encoding: Option<String>,
        container: Option<String>,
        source_locator: Option<String>,
        build_mutation: impl FnOnce(AttachmentReference) -> LiveEvidenceMutationV3,
    ) -> Result<(AttachmentReference, CommitReceiptV2), LivePersistenceError> {
        if self.frozen {
            return Err(LivePersistenceError::ExternalModification {
                message: "this handle is read-only after a persistence failure".into(),
            });
        }
        let digest = sha256_hex(bytes);
        let attachment_path = self.paths.attachments_dir.join("sha256").join(&digest);
        let existed = fs::symlink_metadata(&attachment_path).is_ok();
        let mut attachment =
            durable_attachment(&self.store, &self.paths, bytes, media_type, source_locator)?;
        attachment.encoding = encoding;
        attachment.container = container;
        let mutation = build_mutation(attachment.clone());
        let mutation_id = mutation.mutation_id.clone();
        match self.append_evidence(mutation) {
            Ok(receipt) => Ok((attachment, receipt)),
            Err(error) => {
                let committed = self
                    .bundle
                    .session_state
                    .last_committed_mutation_id
                    .as_deref()
                    == Some(mutation_id.as_str());
                if !existed && !committed {
                    remove_file_if_present(&attachment_path).map_err(|source| {
                        live_io("remove uncommitted attachment", &attachment_path, source)
                    })?;
                    if let Some(parent) = attachment_path.parent() {
                        sync_directory(parent).map_err(|source| {
                            live_io("synchronize attachment rollback", parent, source)
                        })?;
                    }
                    sync_directory(&self.paths.attachments_dir).map_err(|source| {
                        live_io(
                            "synchronize attachments rollback",
                            &self.paths.attachments_dir,
                            source,
                        )
                    })?;
                }
                Err(error)
            }
        }
    }

    pub fn append_event(
        &mut self,
        mut mutation: LiveEventMutationV3,
    ) -> Result<CommitReceiptV2, LivePersistenceError> {
        if self.frozen {
            return Err(LivePersistenceError::ExternalModification {
                message: "this handle is read-only after a persistence failure".into(),
            });
        }
        let actual_state = read_state(&self.paths.session_state)?;
        if actual_state != self.bundle.session_state {
            self.frozen = true;
            return Err(LivePersistenceError::ExternalModification {
                message: "session-state.json changed since the writer snapshot".into(),
            });
        }
        verify_exact_checkpoint(&self.store, &actual_state, &self.paths).inspect_err(|_| {
            self.frozen = true;
        })?;

        if let Some(existing) = self
            .bundle
            .events
            .iter()
            .find(|event| event.meta.mutation.mutation_id == mutation.mutation_id)
        {
            if same_v3_event_business_value(existing, &mutation.event) {
                return Ok(CommitReceiptV2 {
                    revision: self.bundle.session_state.revision,
                    mutation_id: mutation.mutation_id,
                    idempotent: true,
                });
            }
            return Err(LivePersistenceError::MutationConflict {
                mutation_id: mutation.mutation_id,
            });
        }
        if v3_other_stream_has_mutation(&self.bundle, &mutation.mutation_id) {
            return Err(LivePersistenceError::MutationConflict {
                mutation_id: mutation.mutation_id,
            });
        }
        if mutation.expected_revision != self.bundle.session_state.revision {
            return Err(LivePersistenceError::StaleRevision {
                expected: mutation.expected_revision,
                actual: self.bundle.session_state.revision,
            });
        }
        if validate_machine_identity(&mutation.mutation_id).is_err() {
            return Err(LivePersistenceError::InvalidMutation {
                message: "mutation identity must be bounded nonempty ASCII".into(),
            });
        }
        if validate_machine_identity(&mutation.event.event_id).is_err() {
            return Err(LivePersistenceError::InvalidMutation {
                message: "event identity must be bounded nonempty ASCII".into(),
            });
        }
        mutation.event.meta.schema_version = self.bundle.manifest.schema_version;
        mutation.event.meta.session_id = self.bundle.manifest.session_id.clone();
        mutation.event.meta.recorded_at = self.hooks.now();
        mutation.event.meta.mutation = MutationMember {
            mutation_id: mutation.mutation_id.clone(),
            member_index: 0,
            member_count: 1,
        };

        let mut candidate = self.bundle.clone();
        candidate.events.push(mutation.event.clone());
        let reduction = reduce_operator_events_v3(SessionLifecycleV2::Ready, &candidate.events);
        if let Some(diagnostic) = reduction.diagnostics.first() {
            return Err(LivePersistenceError::InvalidMutation {
                message: diagnostic.message.clone(),
            });
        }
        candidate.session_state.lifecycle = reduction.lifecycle;
        BundleStore::refresh_v3_checkpoint(&mut candidate)?;
        candidate.session_state.revision = self
            .bundle
            .session_state
            .revision
            .checked_add(1)
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "checkpoint revision overflowed".into(),
            })?;
        candidate.session_state.last_committed_mutation_id = Some(mutation.mutation_id.clone());
        crate::v3::validate_v3_model(&candidate)?;

        let mut line = serde_json::to_vec(&mutation.event).map_err(|source| {
            LivePersistenceError::InvalidMutation {
                message: format!("event serialization failed: {source}"),
            }
        })?;
        line.push(b'\n');
        preflight_live_budget(
            &self.store,
            &self.bundle.session_state,
            &[(LiveStreamV2::Events, line.clone())],
        )?;

        let baseline = self.bundle.session_state.clone();
        let operation = (|| {
            append_line(
                &self.paths.events,
                LiveStreamV2::Events,
                &line,
                self.hooks.as_ref(),
            )?;
            commit_checkpoint(
                self.store.root(),
                &self.paths.session_state,
                &candidate.session_state,
                self.hooks.as_ref(),
            )
        })();
        if let Err(error) = operation {
            let committed = read_state(&self.paths.session_state)
                .is_ok_and(|state| state.revision > baseline.revision);
            if committed {
                self.bundle = self.store.read_v3()?;
            } else {
                let file = OpenOptions::new()
                    .write(true)
                    .open(&self.paths.events)
                    .map_err(|source| {
                        live_io("open schema-v3 event rollback", &self.paths.events, source)
                    })?;
                file.set_len(
                    baseline
                        .streams
                        .get(LiveStreamV2::Events.checkpoint_name())
                        .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                            message: "baseline checkpoint is missing events".into(),
                        })?
                        .committed_bytes,
                )
                .map_err(|source| {
                    live_io(
                        "truncate schema-v3 event rollback",
                        &self.paths.events,
                        source,
                    )
                })?;
                file.sync_all().map_err(|source| {
                    live_io(
                        "synchronize schema-v3 event rollback",
                        &self.paths.events,
                        source,
                    )
                })?;
            }
            return Err(error);
        }
        self.bundle = candidate;
        self.hooks
            .check(LivePersistencePoint::BeforeAcknowledge)
            .map_err(|source| live_io("acknowledge checkpoint", self.store.root(), source))?;
        Ok(CommitReceiptV2 {
            revision: self.bundle.session_state.revision,
            mutation_id: mutation.mutation_id,
            idempotent: false,
        })
    }
}

fn prepare_v3_evidence(
    mutation: &mut LiveEvidenceMutationV3,
    session_id: &str,
    schema_version: u16,
    recorded_at: DateTime<Utc>,
) -> Result<(), LivePersistenceError> {
    if validate_machine_identity(&mutation.mutation_id).is_err()
        || mutation.adapter_records.is_empty()
    {
        return Err(LivePersistenceError::InvalidMutation {
            message: "evidence mutation requires a bounded identity and adapter records".into(),
        });
    }
    let member_count = u32::try_from(mutation.adapter_records.len() + mutation.observations.len())
        .map_err(|_| LivePersistenceError::InvalidMutation {
            message: "evidence mutation has too many members".into(),
        })?;
    for (index, record) in mutation.adapter_records.iter_mut().enumerate() {
        prepare_v3_evidence_meta(
            &mut record.meta,
            &record.record_id,
            session_id,
            schema_version,
            &mutation.mutation_id,
            u32::try_from(index).expect("member count fits u32"),
            member_count,
            recorded_at,
        )?;
    }
    for (offset, record) in mutation.observations.iter_mut().enumerate() {
        prepare_v3_evidence_meta(
            &mut record.meta,
            &record.observation_id,
            session_id,
            schema_version,
            &mutation.mutation_id,
            u32::try_from(mutation.adapter_records.len() + offset).expect("member count fits u32"),
            member_count,
            recorded_at,
        )?;
    }
    Ok(())
}

fn committed_v5_antenna_control(
    bundle: &BundleV3Contents,
    mutation: &LiveAntennaControlMutationV5,
) -> Option<Result<CommitReceiptV2, LivePersistenceError>> {
    let existing_rig = bundle
        .rig
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation.mutation_id)
        .collect::<Vec<_>>();
    let existing_event = bundle
        .events
        .iter()
        .find(|event| event.meta.mutation.mutation_id == mutation.mutation_id);
    if existing_rig.is_empty() && existing_event.is_none() {
        return None;
    }
    let rig_matches = existing_rig.len() == mutation.rig_records.len()
        && existing_rig
            .iter()
            .zip(&mutation.rig_records)
            .all(|(existing, proposed)| {
                existing.record_id == proposed.record_id
                    && existing.adapter_record_ids == proposed.adapter_record_ids
                    && existing.status == proposed.status
                    && existing.frequency_hz == proposed.frequency_hz
                    && existing.mode == proposed.mode
                    && existing.power_watts == proposed.power_watts
                    && existing.antenna_control == proposed.antenna_control
                    && existing.raw == proposed.raw
            });
    let event_matches = match (existing_event, mutation.armed_event.as_ref()) {
        (None, None) => true,
        (Some(existing), Some(proposed)) => {
            existing.event_id == proposed.event_id
                && existing.occurred_at == proposed.occurred_at
                && existing.time_basis == proposed.time_basis
                && existing.uncertainty_seconds == proposed.uncertainty_seconds
                && existing.slot_id == proposed.slot_id
                && existing.payload == proposed.payload
        }
        _ => false,
    };
    Some(if rig_matches && event_matches {
        Ok(CommitReceiptV2 {
            revision: bundle.session_state.revision,
            mutation_id: mutation.mutation_id.clone(),
            idempotent: true,
        })
    } else {
        Err(LivePersistenceError::MutationConflict {
            mutation_id: mutation.mutation_id.clone(),
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_v3_evidence_meta(
    meta: &mut RecordMetaV2,
    record_id: &str,
    session_id: &str,
    schema_version: u16,
    mutation_id: &str,
    member_index: u32,
    member_count: u32,
    recorded_at: DateTime<Utc>,
) -> Result<(), LivePersistenceError> {
    if validate_machine_identity(record_id).is_err() {
        return Err(LivePersistenceError::InvalidMutation {
            message: "evidence record identities must be bounded nonempty ASCII".into(),
        });
    }
    meta.schema_version = schema_version;
    meta.session_id = session_id.into();
    meta.recorded_at = recorded_at;
    meta.mutation = MutationMember {
        mutation_id: mutation_id.into(),
        member_index,
        member_count,
    };
    Ok(())
}

fn validate_v3_evidence(
    bundle: &BundleV3Contents,
    mutation: &LiveEvidenceMutationV3,
) -> Result<(), LivePersistenceError> {
    if matches!(
        bundle.session_state.lifecycle,
        SessionLifecycleV2::Draft | SessionLifecycleV2::Ready
    ) {
        return Err(LivePersistenceError::InvalidMutation {
            message: "adapter evidence may append only after the session has started".into(),
        });
    }
    let mut ids = bundle
        .events
        .iter()
        .map(|record| record.event_id.as_str())
        .chain(
            bundle
                .adapter_records
                .iter()
                .map(|record| record.record_id.as_str()),
        )
        .chain(
            bundle
                .observations
                .iter()
                .map(|record| record.observation_id.as_str()),
        )
        .chain(bundle.rig.iter().map(|record| record.record_id.as_str()))
        .chain(
            bundle
                .propagation
                .iter()
                .map(|record| record.record_id.as_str()),
        )
        .collect::<BTreeSet<_>>();
    for record in &mutation.adapter_records {
        if !ids.insert(&record.record_id) {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!("record identity {:?} is already present", record.record_id),
            });
        }
        for link in &record.normalized_records {
            if link.record_kind != NormalizedRecordKind::Observation
                || !bundle
                    .observations
                    .iter()
                    .chain(mutation.observations.iter())
                    .any(|observation| observation.observation_id == link.record_id)
            {
                return Err(LivePersistenceError::InvalidMutation {
                    message: format!(
                        "adapter record {:?} has a missing normalized observation link",
                        record.record_id
                    ),
                });
            }
        }
    }
    for record in &mutation.observations {
        if !ids.insert(&record.observation_id) {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "record identity {:?} is already present",
                    record.observation_id
                ),
            });
        }
        if record.adapter_record_ids.is_empty()
            || !record.adapter_record_ids.iter().all(|adapter_id| {
                bundle
                    .adapter_records
                    .iter()
                    .chain(mutation.adapter_records.iter())
                    .any(|adapter| adapter.record_id == *adapter_id)
            })
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "observation {:?} has missing adapter backlinks",
                    record.observation_id
                ),
            });
        }
    }
    Ok(())
}

fn v3_committed_evidence(
    bundle: &BundleV3Contents,
    mutation_id: &str,
) -> Option<LiveEvidenceMutationV3> {
    let adapter_records = bundle
        .adapter_records
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation_id)
        .cloned()
        .collect::<Vec<_>>();
    let observations = bundle
        .observations
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation_id)
        .cloned()
        .collect::<Vec<_>>();
    (!adapter_records.is_empty() || !observations.is_empty()).then(|| LiveEvidenceMutationV3 {
        expected_revision: bundle.session_state.revision,
        mutation_id: mutation_id.into(),
        adapter_records,
        observations,
    })
}

fn same_v3_evidence_business_value(
    existing: &LiveEvidenceMutationV3,
    proposed: &LiveEvidenceMutationV3,
) -> bool {
    if existing.adapter_records.len() != proposed.adapter_records.len()
        || existing.observations.len() != proposed.observations.len()
    {
        return false;
    }
    existing
        .adapter_records
        .iter()
        .zip(&proposed.adapter_records)
        .all(|(existing, proposed)| {
            let mut proposed = proposed.clone();
            proposed.meta = existing.meta.clone();
            existing == &proposed
        })
        && existing
            .observations
            .iter()
            .zip(&proposed.observations)
            .all(|(existing, proposed)| {
                let mut proposed = proposed.clone();
                proposed.meta = existing.meta.clone();
                existing == &proposed
            })
}

fn serialize_v3_lines<T: Serialize>(
    records: &[T],
    label: &str,
) -> Result<Vec<u8>, LivePersistenceError> {
    let mut bytes = Vec::new();
    for record in records {
        serde_json::to_writer(&mut bytes, record).map_err(|source| {
            LivePersistenceError::InvalidMutation {
                message: format!("{label} serialization failed: {source}"),
            }
        })?;
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn rollback_v3_streams(
    paths: &ResolvedBundlePathsV2,
    baseline: &SessionStateV2,
    streams: &[LiveStreamV2],
) -> Result<(), LivePersistenceError> {
    for stream in streams {
        let checkpoint = baseline
            .streams
            .get(stream.checkpoint_name())
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: format!(
                    "baseline checkpoint is missing {}",
                    stream.checkpoint_name()
                ),
            })?;
        let path = stream_path(paths, *stream);
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|source| live_io("open schema-v3 evidence rollback", path, source))?;
        file.set_len(checkpoint.committed_bytes)
            .map_err(|source| live_io("truncate schema-v3 evidence rollback", path, source))?;
        file.sync_all()
            .map_err(|source| live_io("synchronize schema-v3 evidence rollback", path, source))?;
    }
    Ok(())
}

fn same_v3_event_business_value(existing: &OperatorEventV3, proposed: &OperatorEventV3) -> bool {
    let mut proposed = proposed.clone();
    proposed.meta = existing.meta.clone();
    existing == &proposed
}

fn v3_other_stream_has_mutation(bundle: &BundleV3Contents, mutation_id: &str) -> bool {
    bundle
        .adapter_records
        .iter()
        .any(|record| record.meta.mutation.mutation_id == mutation_id)
        || bundle
            .observations
            .iter()
            .any(|record| record.meta.mutation.mutation_id == mutation_id)
        || bundle
            .rig
            .iter()
            .any(|record| record.meta.mutation.mutation_id == mutation_id)
        || bundle
            .propagation
            .iter()
            .any(|record| record.meta.mutation.mutation_id == mutation_id)
}

#[derive(Debug)]
struct StreamTail {
    stream: LiveStreamV2,
    committed_offset: u64,
    bytes: Vec<u8>,
}

struct RecoveryBytes<'a> {
    source: &'a str,
    committed_offset: u64,
    bytes: &'a [u8],
}

fn recover_pending_plan_generation(
    store: &BundleStore,
    bundle: &mut BundleV2Contents,
    paths: &mut ResolvedBundlePathsV2,
    artifacts: &mut Vec<RecoveryArtifactV2>,
    hooks: &dyn LivePersistenceHooks,
) -> Result<Option<RecoveryDispositionV2>, LivePersistenceError> {
    let generations_dir = store.root().join("plan-generations");
    if !generations_dir.exists() {
        return Ok(None);
    }
    let mut pending = Vec::<(PathBuf, PlanGenerationMetadataV2)>::new();
    let mut unresolved = Vec::<PathBuf>::new();
    let entries = fs::read_dir(&generations_dir)
        .map_err(|source| live_io("inspect plan generations", &generations_dir, source))?;
    for entry in entries {
        let entry = entry
            .map_err(|source| live_io("inspect plan generation entry", &generations_dir, source))?;
        let path = entry.path();
        let metadata = entry
            .file_type()
            .map_err(|source| live_io("inspect plan generation type", &path, source))?;
        if !metadata.is_dir() || metadata.is_symlink() {
            unresolved.push(path);
            continue;
        }
        if entry.file_name().to_string_lossy() == bundle.session_state.active_plan.generation_id {
            continue;
        }
        let marker = path.join("generation.json");
        match read_json_file::<PlanGenerationMetadataV2>(store, &marker, "plan generation metadata")
        {
            Ok(metadata)
                if metadata.schema_version == SCHEMA_VERSION_V2
                    && metadata.session_id == bundle.manifest.session_id
                    && metadata.base_revision == bundle.session_state.revision =>
            {
                pending.push((path, metadata));
            }
            Ok(metadata) if metadata.base_revision < bundle.session_state.revision => {
                // A formerly active generation remains immutable history.
            }
            _ => unresolved.push(path),
        }
    }

    if pending.len() == 1 && unresolved.is_empty() {
        let (generation_dir, metadata) = pending.pop().expect("one pending generation");
        match load_pending_plan(store, &generation_dir, &metadata) {
            Ok((station, antennas, schedule))
                if matches!(
                    bundle.session_state.lifecycle,
                    SessionLifecycleV2::Draft | SessionLifecycleV2::Ready
                ) =>
            {
                let mut candidate = bundle.clone();
                candidate.station = station;
                candidate.antennas = antennas;
                candidate.schedule = schedule;
                candidate.session_state.active_plan = metadata.generation.clone();
                candidate.session_state.lifecycle = SessionLifecycleV2::Ready;
                let report = validate_bundle_report(&candidate.clone().into_current().bundle);
                if report.allows(BundleValidationProfile::StrictCreation) {
                    let mut next = candidate.session_state.clone();
                    next.revision = next.revision.checked_add(1).ok_or_else(|| {
                        LivePersistenceError::CheckpointVerification {
                            message: "checkpoint revision overflowed during plan recovery".into(),
                        }
                    })?;
                    next.last_committed_mutation_id =
                        Some(format!("plan:{}", metadata.generation.generation_id));
                    commit_checkpoint(store.root(), &paths.session_state, &next, hooks)?;
                    candidate.session_state = next;
                    *bundle = candidate;
                    *paths =
                        store.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
                    return Ok(Some(RecoveryDispositionV2::RolledForward));
                }
            }
            _ => {}
        }
        unresolved.push(generation_dir);
    } else {
        unresolved.extend(pending.into_iter().map(|(path, _)| path));
    }

    if unresolved.is_empty() {
        return Ok(None);
    }
    unresolved.sort();
    let diagnosis = if unresolved.len() == 1 {
        "plan generation is incomplete, malformed, or invalid"
    } else {
        "multiple pending plan generations conflict"
    };
    for generation in unresolved {
        preserve_plan_generation(
            store,
            bundle,
            paths,
            &generation,
            diagnosis,
            hooks.now(),
            artifacts,
        )?;
        if generation.is_dir() {
            fs::remove_dir_all(&generation).map_err(|source| {
                live_io("remove recovered plan generation", &generation, source)
            })?;
        } else {
            fs::remove_file(&generation)
                .map_err(|source| live_io("remove recovered plan artifact", &generation, source))?;
        }
    }
    sync_directory(&generations_dir).map_err(|source| {
        live_io(
            "synchronize recovered plan generation cleanup",
            &generations_dir,
            source,
        )
    })?;
    Ok(Some(RecoveryDispositionV2::RolledBack))
}

fn load_pending_plan(
    store: &BundleStore,
    generation_dir: &Path,
    metadata: &PlanGenerationMetadataV2,
) -> Result<(Station, AntennasFile, Schedule), LivePersistenceError> {
    let station_path = generation_dir.join("station.json");
    let antennas_path = generation_dir.join("antennas.json");
    let schedule_path = generation_dir.join("schedule.json");
    let station_bytes = read_bounded(
        store,
        &station_path,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Write,
    )?;
    let antennas_bytes = read_bounded(
        store,
        &antennas_path,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Write,
    )?;
    let schedule_bytes = read_bounded(
        store,
        &schedule_path,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Write,
    )?;
    let digests = [
        sha256_hex(&station_bytes),
        sha256_hex(&antennas_bytes),
        sha256_hex(&schedule_bytes),
    ];
    if digests[0] != metadata.generation.station_sha256
        || digests[1] != metadata.generation.antennas_sha256
        || digests[2] != metadata.generation.schedule_sha256
        || sha256_hex(digests.join("\n").as_bytes()) != metadata.generation.root_sha256
    {
        return Err(LivePersistenceError::CheckpointVerification {
            message: "pending plan generation digest does not match its durable metadata".into(),
        });
    }
    let station = serde_json::from_slice(&station_bytes).map_err(|source| {
        LivePersistenceError::CheckpointVerification {
            message: format!("pending station plan is invalid: {source}"),
        }
    })?;
    let antennas = serde_json::from_slice(&antennas_bytes).map_err(|source| {
        LivePersistenceError::CheckpointVerification {
            message: format!("pending antenna plan is invalid: {source}"),
        }
    })?;
    let schedule = serde_json::from_slice(&schedule_bytes).map_err(|source| {
        LivePersistenceError::CheckpointVerification {
            message: format!("pending schedule plan is invalid: {source}"),
        }
    })?;
    Ok((station, antennas, schedule))
}

fn preserve_plan_generation(
    store: &BundleStore,
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
    generation: &Path,
    diagnosis: &str,
    detected_at: DateTime<Utc>,
    artifacts: &mut Vec<RecoveryArtifactV2>,
) -> Result<(), LivePersistenceError> {
    if generation.is_file() {
        let bytes = read_bounded(
            store,
            generation,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Write,
        )?;
        let source = format!(
            "plan-generation/{}",
            generation.file_name().unwrap_or_default().to_string_lossy()
        );
        artifacts.push(preserve_recovery_bytes(
            store,
            SCHEMA_VERSION_V2,
            &bundle.manifest.session_id,
            paths,
            RecoveryBytes {
                source: &source,
                committed_offset: 0,
                bytes: &bytes,
            },
            diagnosis,
            detected_at,
        )?);
        return Ok(());
    }
    let mut entries = fs::read_dir(generation)
        .map_err(|source| live_io("inspect unresolved plan generation", generation, source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| live_io("inspect unresolved plan generation", generation, source))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|source| live_io("inspect unresolved plan file", &path, source))?
            .is_file()
        {
            return Err(LivePersistenceError::RecoveryRequired {
                message: format!("unsupported entry remains in {}", generation.display()),
            });
        }
        let bytes = read_bounded(
            store,
            &path,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Write,
        )?;
        let source = format!(
            "plan-generation/{}/{}",
            generation.file_name().unwrap_or_default().to_string_lossy(),
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        artifacts.push(preserve_recovery_bytes(
            store,
            SCHEMA_VERSION_V2,
            &bundle.manifest.session_id,
            paths,
            RecoveryBytes {
                source: &source,
                committed_offset: 0,
                bytes: &bytes,
            },
            diagnosis,
            detected_at,
        )?);
    }
    Ok(())
}

fn load_checkpointed_bundle(store: &BundleStore) -> Result<BundleV2Contents, LivePersistenceError> {
    let manifest_path = store.root().join("manifest.json");
    let manifest: BundleManifestV2 = read_json_file(store, &manifest_path, "manifest")?;
    if manifest.schema_version != SCHEMA_VERSION_V2 {
        return Err(LivePersistenceError::Store(
            BundleStoreError::UnsupportedSchemaVersion {
                actual: manifest.schema_version,
            },
        ));
    }
    let bootstrap = store.v2_paths(&manifest.files)?;
    let session_state: SessionStateV2 =
        read_json_file(store, &bootstrap.session_state, "checkpoint")?;
    load_checkpointed_bundle_from_state(store, manifest, session_state)
}

fn load_recovery_bundle(
    store: &BundleStore,
) -> Result<(BundleV2Contents, bool), LivePersistenceError> {
    let manifest_path = store.root().join("manifest.json");
    let manifest: BundleManifestV2 = read_json_file(store, &manifest_path, "manifest")?;
    if manifest.schema_version != SCHEMA_VERSION_V2 {
        return Err(LivePersistenceError::Store(
            BundleStoreError::UnsupportedSchemaVersion {
                actual: manifest.schema_version,
            },
        ));
    }
    let bootstrap = store.v2_paths(&manifest.files)?;
    let candidates = [
        (bootstrap.session_state.clone(), false),
        (store.root().join(CHECKPOINT_PREVIOUS), true),
    ];
    let mut valid = Vec::new();
    let mut last_error = None;
    for (path, previous) in candidates {
        if !path.exists() {
            continue;
        }
        let state = match read_json_file(store, &path, "recovery checkpoint") {
            Ok(state) => state,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        match load_checkpointed_bundle_from_state(store, manifest.clone(), state) {
            Ok(bundle) => valid.push((bundle, previous)),
            Err(error) => last_error = Some(error),
        }
    }
    valid.sort_by_key(|(bundle, _)| bundle.session_state.revision);
    valid.pop().ok_or_else(|| {
        last_error.unwrap_or_else(|| LivePersistenceError::CheckpointVerification {
            message: "neither current nor previous checkpoint is valid".into(),
        })
    })
}

fn load_checkpointed_bundle_from_state(
    store: &BundleStore,
    manifest: BundleManifestV2,
    session_state: SessionStateV2,
) -> Result<BundleV2Contents, LivePersistenceError> {
    let paths = store.v2_paths_for_state(&manifest.files, &session_state)?;
    verify_committed_prefixes(store, &session_state, &paths)?;
    let bundle = BundleV2Contents {
        manifest,
        session_state: session_state.clone(),
        station: read_json_file(store, &paths.station, "active station plan")?,
        antennas: read_json_file(store, &paths.antennas, "active antenna plan")?,
        schedule: read_json_file(store, &paths.schedule, "active schedule plan")?,
        events: read_jsonl_prefix(
            store,
            &paths.events,
            stream_checkpoint(&session_state, LiveStreamV2::Events)?.committed_bytes,
        )?,
        observations: read_jsonl_prefix(
            store,
            &paths.observations,
            stream_checkpoint(&session_state, LiveStreamV2::Observations)?.committed_bytes,
        )?,
        adapter_records: read_jsonl_prefix(
            store,
            &paths.adapter_records,
            stream_checkpoint(&session_state, LiveStreamV2::AdapterRecords)?.committed_bytes,
        )?,
        rig: read_jsonl_prefix(
            store,
            &paths.rig,
            stream_checkpoint(&session_state, LiveStreamV2::Rig)?.committed_bytes,
        )?,
        propagation: read_jsonl_prefix(
            store,
            &paths.propagation,
            stream_checkpoint(&session_state, LiveStreamV2::Propagation)?.committed_bytes,
        )?,
        analysis: read_json_file::<AnalysisFile>(store, &paths.analysis, "analysis metadata")?,
    };
    verify_loaded_counts(&bundle)?;
    Ok(bundle)
}

fn verify_loaded_counts(bundle: &BundleV2Contents) -> Result<(), LivePersistenceError> {
    for (stream, count, last_id) in [
        (
            LiveStreamV2::Events,
            bundle.events.len(),
            bundle.events.last().map(|record| record.event_id.as_str()),
        ),
        (
            LiveStreamV2::AdapterRecords,
            bundle.adapter_records.len(),
            bundle
                .adapter_records
                .last()
                .map(|record| record.record_id.as_str()),
        ),
        (
            LiveStreamV2::Observations,
            bundle.observations.len(),
            bundle
                .observations
                .last()
                .map(|record| record.observation_id.as_str()),
        ),
        (
            LiveStreamV2::Rig,
            bundle.rig.len(),
            bundle.rig.last().map(|record| record.record_id.as_str()),
        ),
        (
            LiveStreamV2::Propagation,
            bundle.propagation.len(),
            bundle
                .propagation
                .last()
                .map(|record| record.record_id.as_str()),
        ),
    ] {
        let checkpoint = stream_checkpoint(&bundle.session_state, stream)?;
        if checkpoint.record_count != u64::try_from(count).expect("usize fits u64")
            || checkpoint.last_record_id.as_deref() != last_id
        {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "{} committed record count or last identity is inconsistent",
                    stream.checkpoint_name()
                ),
            });
        }
    }
    Ok(())
}

fn verify_committed_prefixes(
    store: &BundleStore,
    checkpoint: &SessionStateV2,
    paths: &ResolvedBundlePathsV2,
) -> Result<(), LivePersistenceError> {
    for stream in all_streams() {
        let expected = stream_checkpoint(checkpoint, stream)?;
        let path = stream_path(paths, stream);
        let bytes = read_bounded(
            store,
            path,
            store.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            ResourceOperation::Write,
        )?;
        let committed = usize::try_from(expected.committed_bytes).map_err(|_| {
            LivePersistenceError::CheckpointVerification {
                message: format!(
                    "{} committed offset does not fit this platform",
                    stream.checkpoint_name()
                ),
            }
        })?;
        if bytes.len() < committed {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "{} is shorter than its committed prefix",
                    stream.checkpoint_name()
                ),
            });
        }
        if sha256_hex(&bytes[..committed]) != expected.committed_sha256 {
            return Err(LivePersistenceError::ExternalModification {
                message: format!(
                    "{} has corruption inside its committed prefix",
                    stream.checkpoint_name()
                ),
            });
        }
    }
    for (name, path, expected) in [
        (
            "station",
            &paths.station,
            &checkpoint.active_plan.station_sha256,
        ),
        (
            "antennas",
            &paths.antennas,
            &checkpoint.active_plan.antennas_sha256,
        ),
        (
            "schedule",
            &paths.schedule,
            &checkpoint.active_plan.schedule_sha256,
        ),
    ] {
        let bytes = read_bounded(
            store,
            path,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Write,
        )?;
        if sha256_hex(&bytes) != *expected {
            return Err(LivePersistenceError::ExternalModification {
                message: format!("active plan {name} has committed corruption"),
            });
        }
    }
    Ok(())
}

fn read_stream_tails(
    store: &BundleStore,
    checkpoint: &SessionStateV2,
    paths: &ResolvedBundlePathsV2,
) -> Result<Vec<StreamTail>, LivePersistenceError> {
    all_streams()
        .into_iter()
        .map(|stream| {
            let committed_offset = stream_checkpoint(checkpoint, stream)?.committed_bytes;
            let path = stream_path(paths, stream);
            let bytes = read_bounded(
                store,
                path,
                store.profile().jsonl_stream_bytes,
                "resource.jsonl.stream_bytes",
                ResourceOperation::Write,
            )?;
            let offset = usize::try_from(committed_offset).map_err(|_| {
                LivePersistenceError::CheckpointVerification {
                    message: "committed offset does not fit this platform".into(),
                }
            })?;
            Ok(StreamTail {
                stream,
                committed_offset,
                bytes: bytes[offset..].to_vec(),
            })
        })
        .collect()
}

fn parse_tail_mutation(
    bundle: &BundleV2Contents,
    tails: &[StreamTail],
) -> Result<LiveMutationV2, String> {
    let mut members = Vec::new();
    for tail in tails.iter().filter(|tail| !tail.bytes.is_empty()) {
        if !tail.bytes.ends_with(b"\n") {
            return Err(format!(
                "{} tail is torn and not newline terminated",
                tail.stream.checkpoint_name()
            ));
        }
        for line in tail.bytes.split_inclusive(|byte| *byte == b'\n') {
            let line = &line[..line.len() - 1];
            if line.is_empty() {
                return Err(format!(
                    "{} tail contains an empty member",
                    tail.stream.checkpoint_name()
                ));
            }
            members.push(parse_tail_member(tail.stream, line).map_err(|error| {
                format!(
                    "{} tail contains malformed JSON: {error}",
                    tail.stream.checkpoint_name()
                )
            })?);
        }
    }
    if members.is_empty() {
        return Err("no complete tail mutation members were found".into());
    }
    members.sort_by_key(|member| member.meta().mutation.member_index);
    let mutation_id = members[0].meta().mutation.mutation_id.clone();
    let member_count = members[0].meta().mutation.member_count;
    if mutation_id.is_empty()
        || member_count != u32::try_from(members.len()).unwrap_or(u32::MAX)
        || members.iter().enumerate().any(|(index, member)| {
            member.meta().mutation.mutation_id != mutation_id
                || member.meta().mutation.member_count != member_count
                || member.meta().mutation.member_index
                    != u32::try_from(index).expect("tail member count fits u32")
        })
    {
        return Err("tail does not contain one complete declared mutation".into());
    }
    Ok(LiveMutationV2 {
        expected_revision: bundle.session_state.revision,
        mutation_id,
        members,
    })
}

fn parse_tail_member(
    stream: LiveStreamV2,
    line: &[u8],
) -> Result<LiveMutationMemberV2, serde_json::Error> {
    match stream {
        LiveStreamV2::Events => serde_json::from_slice(line).map(LiveMutationMemberV2::Event),
        LiveStreamV2::AdapterRecords => {
            serde_json::from_slice(line).map(LiveMutationMemberV2::AdapterRecord)
        }
        LiveStreamV2::Observations => {
            serde_json::from_slice(line).map(LiveMutationMemberV2::Observation)
        }
        LiveStreamV2::Rig => serde_json::from_slice(line).map(LiveMutationMemberV2::Rig),
        LiveStreamV2::Propagation => {
            serde_json::from_slice(line).map(LiveMutationMemberV2::Propagation)
        }
    }
}

fn preserve_tails(
    store: &BundleStore,
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
    tails: &[StreamTail],
    diagnosis: &str,
    detected_at: DateTime<Utc>,
) -> Result<Vec<RecoveryArtifactV2>, LivePersistenceError> {
    preserve_tails_for(
        store,
        SCHEMA_VERSION_V2,
        &bundle.manifest.session_id,
        paths,
        tails,
        diagnosis,
        detected_at,
    )
}

fn preserve_tails_for(
    store: &BundleStore,
    schema_version: u16,
    session_id: &str,
    paths: &ResolvedBundlePathsV2,
    tails: &[StreamTail],
    diagnosis: &str,
    detected_at: DateTime<Utc>,
) -> Result<Vec<RecoveryArtifactV2>, LivePersistenceError> {
    let mut artifacts = Vec::new();
    for tail in tails.iter().filter(|tail| !tail.bytes.is_empty()) {
        let source = tail.stream.checkpoint_name().to_string();
        artifacts.push(preserve_recovery_bytes(
            store,
            schema_version,
            session_id,
            paths,
            RecoveryBytes {
                source: &source,
                committed_offset: tail.committed_offset,
                bytes: &tail.bytes,
            },
            diagnosis,
            detected_at,
        )?);
    }
    Ok(artifacts)
}

fn preserve_recovery_bytes(
    store: &BundleStore,
    schema_version: u16,
    session_id: &str,
    paths: &ResolvedBundlePathsV2,
    evidence: RecoveryBytes<'_>,
    diagnosis: &str,
    detected_at: DateTime<Utc>,
) -> Result<RecoveryArtifactV2, LivePersistenceError> {
    let RecoveryBytes {
        source,
        committed_offset,
        bytes,
    } = evidence;
    let raw_attachment = durable_attachment(
        store,
        paths,
        bytes,
        "application/octet-stream",
        Some(format!("recovery:{source}:{committed_offset}")),
    )?;
    let metadata = RecoveryArtifactMetadataV2 {
        schema_version,
        session_id,
        source,
        committed_offset,
        detected_at,
        diagnosis,
        raw_attachment: &raw_attachment,
    };
    let metadata_bytes = serialize_json(&metadata).map_err(invalid_serialization)?;
    let metadata_attachment = durable_attachment(
        store,
        paths,
        &metadata_bytes,
        "application/json",
        Some(format!("recovery-metadata:{source}:{committed_offset}")),
    )?;
    Ok(RecoveryArtifactV2 {
        source: source.into(),
        committed_offset,
        diagnosis: diagnosis.into(),
        raw_attachment,
        metadata_attachment,
    })
}

fn durable_attachment(
    store: &BundleStore,
    paths: &ResolvedBundlePathsV2,
    bytes: &[u8],
    media_type: &str,
    source_locator: Option<String>,
) -> Result<AttachmentReference, LivePersistenceError> {
    let reference = store.write_attachment(bytes, media_type, None, None, source_locator)?;
    let relative =
        reference
            .relative_path()
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: "recovery attachment digest is invalid".into(),
            })?;
    let path = paths.attachments_dir.join(relative);
    sync_regular_file(&path)
        .map_err(|source| live_io("synchronize recovery attachment", &path, source))?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)
            .map_err(|source| live_io("synchronize recovery digest directory", parent, source))?;
    }
    sync_directory(&paths.attachments_dir).map_err(|source| {
        live_io(
            "synchronize recovery attachments directory",
            &paths.attachments_dir,
            source,
        )
    })?;
    Ok(reference)
}

fn copy_checkpointed_attachments(
    store: &BundleStore,
    source_attachments: &Path,
    destination_attachments: &Path,
) -> Result<(), LivePersistenceError> {
    let source_digest_dir = source_attachments.join("sha256");
    if !source_digest_dir.exists() {
        return Ok(());
    }
    let destination_digest_dir = destination_attachments.join("sha256");
    fs::create_dir_all(&destination_digest_dir).map_err(|source| {
        live_io(
            "create export attachment directory",
            &destination_digest_dir,
            source,
        )
    })?;
    let mut entries = fs::read_dir(&source_digest_dir)
        .map_err(|source| {
            live_io(
                "inspect checkpointed attachments",
                &source_digest_dir,
                source,
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| {
            live_io(
                "inspect checkpointed attachments",
                &source_digest_dir,
                source,
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let source_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| live_io("inspect checkpointed attachment", &source_path, source))?;
        if !file_type.is_file() || file_type.is_symlink() {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "checkpointed attachment is not a regular file: {}",
                    source_path.display()
                ),
            });
        }
        let name = entry.file_name();
        let expected = name.to_string_lossy();
        if digest_file(&source_path, store.profile().attachment_file_bytes)? != expected {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "attachment digest name does not match {}",
                    source_path.display()
                ),
            });
        }
        let destination_path = destination_digest_dir.join(&name);
        if !destination_path.exists() {
            fs::copy(&source_path, &destination_path).map_err(|source| {
                live_io("copy checkpointed attachment", &destination_path, source)
            })?;
        }
        if digest_file(&destination_path, store.profile().attachment_file_bytes)? != expected {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "exported attachment digest does not match {}",
                    destination_path.display()
                ),
            });
        }
        sync_regular_file(&destination_path).map_err(|source| {
            live_io("synchronize exported attachment", &destination_path, source)
        })?;
    }
    sync_directory(&destination_digest_dir).map_err(|source| {
        live_io(
            "synchronize exported digest directory",
            &destination_digest_dir,
            source,
        )
    })?;
    sync_directory(destination_attachments).map_err(|source| {
        live_io(
            "synchronize exported attachments directory",
            destination_attachments,
            source,
        )
    })
}

fn digest_file(path: &Path, limit: u64) -> Result<String, LivePersistenceError> {
    let size = fs::metadata(path)
        .map_err(|source| live_io("inspect attachment digest input", path, source))?
        .len();
    if size > limit {
        return Err(LivePersistenceError::CheckpointVerification {
            message: format!(
                "attachment {} exceeds the {} byte live-copy limit",
                path.display(),
                limit
            ),
        });
    }
    let mut file =
        File::open(path).map_err(|source| live_io("open attachment digest input", path, source))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|source| live_io("read attachment digest input", path, source))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(crate::v2::encode_lower_hex(digest.finalize()))
}

fn truncate_tails(
    checkpoint: &SessionStateV2,
    paths: &ResolvedBundlePathsV2,
) -> Result<(), LivePersistenceError> {
    for stream in all_streams() {
        let committed = stream_checkpoint(checkpoint, stream)?.committed_bytes;
        let path = stream_path(paths, stream);
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|source| live_io("open recovery truncation", path, source))?;
        file.set_len(committed)
            .map_err(|source| live_io("truncate recovered stream", path, source))?;
        file.sync_all()
            .map_err(|source| live_io("synchronize recovered stream", path, source))?;
    }
    sync_directory(paths.session_state.parent().expect("checkpoint has parent")).map_err(|source| {
        live_io(
            "synchronize recovered bundle directory",
            paths.session_state.parent().expect("checkpoint has parent"),
            source,
        )
    })
}

fn recover_checkpoint_temp(
    store: &BundleStore,
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
    artifacts: &mut Vec<RecoveryArtifactV2>,
    detected_at: DateTime<Utc>,
) -> Result<(), LivePersistenceError> {
    recover_checkpoint_temp_for(
        store,
        SCHEMA_VERSION_V2,
        &bundle.manifest.session_id,
        paths,
        artifacts,
        detected_at,
    )
}

fn recover_checkpoint_temp_for(
    store: &BundleStore,
    schema_version: u16,
    session_id: &str,
    paths: &ResolvedBundlePathsV2,
    artifacts: &mut Vec<RecoveryArtifactV2>,
    detected_at: DateTime<Utc>,
) -> Result<(), LivePersistenceError> {
    let temp = store.root().join(CHECKPOINT_TEMP);
    if !temp.exists() {
        return Ok(());
    }
    let bytes = read_bounded(
        store,
        &temp,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Write,
    )?;
    if !bytes.is_empty() && serde_json::from_slice::<SessionStateV2>(&bytes).is_err() {
        let diagnosis = "checkpoint temporary file is malformed";
        let raw_attachment = durable_attachment(
            store,
            paths,
            &bytes,
            "application/json",
            Some("recovery:checkpoint-temp".into()),
        )?;
        let metadata = RecoveryArtifactMetadataV2 {
            schema_version,
            session_id,
            source: "checkpoint_temp",
            committed_offset: 0,
            detected_at,
            diagnosis,
            raw_attachment: &raw_attachment,
        };
        let metadata_bytes = serialize_json(&metadata).map_err(invalid_serialization)?;
        let metadata_attachment = durable_attachment(
            store,
            paths,
            &metadata_bytes,
            "application/json",
            Some("recovery-metadata:checkpoint-temp".into()),
        )?;
        artifacts.push(RecoveryArtifactV2 {
            source: "checkpoint_temp".into(),
            committed_offset: 0,
            diagnosis: diagnosis.into(),
            raw_attachment,
            metadata_attachment,
        });
    }
    fs::remove_file(&temp)
        .map_err(|source| live_io("remove recovered checkpoint temp", &temp, source))?;
    sync_directory(store.root())
        .map_err(|source| live_io("synchronize checkpoint temp cleanup", store.root(), source))
}

fn stream_checkpoint(
    state: &SessionStateV2,
    stream: LiveStreamV2,
) -> Result<&StreamCheckpointV2, LivePersistenceError> {
    state.streams.get(stream.checkpoint_name()).ok_or_else(|| {
        LivePersistenceError::CheckpointVerification {
            message: format!("checkpoint is missing {}", stream.checkpoint_name()),
        }
    })
}

fn read_json_file<T: serde::de::DeserializeOwned>(
    store: &BundleStore,
    path: &Path,
    description: &str,
) -> Result<T, LivePersistenceError> {
    let bytes = read_bounded(
        store,
        path,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Read,
    )?;
    serde_json::from_slice(&bytes).map_err(|source| LivePersistenceError::CheckpointVerification {
        message: format!("{description} at {} is invalid: {source}", path.display()),
    })
}

fn read_jsonl_prefix<T: serde::de::DeserializeOwned>(
    store: &BundleStore,
    path: &Path,
    committed_bytes: u64,
) -> Result<Vec<T>, LivePersistenceError> {
    let bytes = read_bounded(
        store,
        path,
        store.profile().jsonl_stream_bytes,
        "resource.jsonl.stream_bytes",
        ResourceOperation::Read,
    )?;
    let end = usize::try_from(committed_bytes).map_err(|_| {
        LivePersistenceError::CheckpointVerification {
            message: format!(
                "committed prefix for {} does not fit this platform",
                path.display()
            ),
        }
    })?;
    let prefix = bytes
        .get(..end)
        .ok_or_else(|| LivePersistenceError::CheckpointVerification {
            message: format!("{} is shorter than its committed prefix", path.display()),
        })?;
    if !prefix.is_empty() && !prefix.ends_with(b"\n") {
        return Err(LivePersistenceError::CheckpointVerification {
            message: format!(
                "{} committed prefix is not newline terminated",
                path.display()
            ),
        });
    }
    prefix
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_slice(line).map_err(|source| {
                LivePersistenceError::CheckpointVerification {
                    message: format!(
                        "{} contains malformed committed JSONL: {source}",
                        path.display()
                    ),
                }
            })
        })
        .collect()
}

fn prepare_mutation(
    mutation: &mut LiveMutationV2,
    session_id: &str,
    recorded_at: DateTime<Utc>,
) -> Result<(), LivePersistenceError> {
    if mutation.mutation_id.is_empty() || mutation.members.is_empty() {
        return Err(LivePersistenceError::InvalidMutation {
            message: "mutation ID and members must not be empty".into(),
        });
    }
    let member_count = u32::try_from(mutation.members.len()).map_err(|_| {
        LivePersistenceError::InvalidMutation {
            message: "mutation has too many members".into(),
        }
    })?;
    mutation
        .members
        .sort_by_key(|member| member.meta().mutation.member_index);
    for (index, member) in mutation.members.iter_mut().enumerate() {
        if member.record_id().is_empty() {
            return Err(LivePersistenceError::InvalidMutation {
                message: "record identities must not be empty".into(),
            });
        }
        let expected_index = u32::try_from(index).expect("member count fits u32");
        if member.meta().mutation.member_index != expected_index {
            return Err(LivePersistenceError::InvalidMutation {
                message: "member indexes must be contiguous from zero".into(),
            });
        }
        let meta = member.meta_mut();
        meta.schema_version = SCHEMA_VERSION_V2;
        meta.session_id = session_id.to_string();
        meta.recorded_at = recorded_at;
        meta.mutation.mutation_id = mutation.mutation_id.clone();
        meta.mutation.member_count = member_count;
    }
    Ok(())
}

fn validate_mutation(
    bundle: &BundleV2Contents,
    mutation: &LiveMutationV2,
) -> Result<SessionLifecycleV2, LivePersistenceError> {
    let declared_count = u32::try_from(mutation.members.len()).map_err(|_| {
        LivePersistenceError::InvalidMutation {
            message: "mutation has too many members".into(),
        }
    })?;
    for (index, member) in mutation.members.iter().enumerate() {
        let meta = member.meta();
        if meta.schema_version != SCHEMA_VERSION_V2
            || meta.session_id != bundle.manifest.session_id
            || meta.mutation.mutation_id != mutation.mutation_id
            || meta.mutation.member_count != declared_count
            || meta.mutation.member_index != u32::try_from(index).expect("member count fits u32")
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: "mutation member envelope does not match the bundle and declared batch"
                    .into(),
            });
        }
    }
    let mut ids = bundle
        .events
        .iter()
        .map(|record| record.event_id.as_str())
        .chain(
            bundle
                .adapter_records
                .iter()
                .map(|record| record.record_id.as_str()),
        )
        .chain(
            bundle
                .observations
                .iter()
                .map(|record| record.observation_id.as_str()),
        )
        .chain(bundle.rig.iter().map(|record| record.record_id.as_str()))
        .chain(
            bundle
                .propagation
                .iter()
                .map(|record| record.record_id.as_str()),
        )
        .collect::<BTreeSet<_>>();
    let mut next_lifecycle = bundle.session_state.lifecycle;
    let mut event_count = 0;
    for member in &mutation.members {
        if !ids.insert(member.record_id()) {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "record identity {:?} is already present",
                    member.record_id()
                ),
            });
        }
        if let LiveMutationMemberV2::Event(event) = member {
            event_count += 1;
            let initial =
                if bundle.events.is_empty() {
                    bundle.session_state.lifecycle
                } else if bundle.events.iter().any(|event| {
                    matches!(event.payload, OperatorEventPayloadV2::SessionStarted { .. })
                }) {
                    SessionLifecycleV2::Ready
                } else {
                    SessionLifecycleV2::Draft
                };
            validate_operator_event_append_v2(
                initial,
                bundle.session_state.revision,
                mutation.expected_revision,
                &bundle.events,
                event,
            )
            .map_err(|error| LivePersistenceError::InvalidMutation {
                message: error.to_string(),
            })?;
            if is_lifecycle_payload(&event.payload) {
                next_lifecycle = validate_lifecycle_transition_v2(
                    next_lifecycle,
                    bundle.session_state.revision,
                    mutation.expected_revision,
                    &event.payload,
                )
                .map_err(|error| LivePersistenceError::InvalidMutation {
                    message: error.to_string(),
                })?;
            }
        }
    }
    if event_count > 1 {
        return Err(LivePersistenceError::InvalidMutation {
            message: "one operator action may append only one event".into(),
        });
    }
    if mutation
        .members
        .iter()
        .any(|member| !matches!(member, LiveMutationMemberV2::Event(_)))
        && bundle.session_state.lifecycle != SessionLifecycleV2::Running
    {
        return Err(LivePersistenceError::InvalidMutation {
            message: "adapter and normalized evidence may append only while running".into(),
        });
    }

    let antenna_labels = bundle
        .antennas
        .antennas
        .iter()
        .map(|antenna| antenna.label.as_str())
        .collect::<BTreeSet<_>>();
    for member in &mutation.members {
        if let LiveMutationMemberV2::Event(OperatorEventV2 {
            payload: OperatorEventPayloadV2::AntennaStateConfirmed { antenna_label, .. },
            ..
        }) = member
        {
            if !antenna_labels.contains(antenna_label.as_str()) {
                return Err(LivePersistenceError::InvalidMutation {
                    message: format!(
                        "actual antenna label {antenna_label:?} is not in the active plan"
                    ),
                });
            }
        }
    }

    let mut candidate = bundle.clone();
    for member in mutation.members.clone() {
        member.append_to(&mut candidate);
    }
    candidate.session_state.lifecycle = next_lifecycle;
    validate_adapter_links(&candidate)?;
    let report = validate_bundle_report(&candidate.into_current().bundle);
    if !report.allows(BundleValidationProfile::StrictCreation) {
        return Err(LivePersistenceError::InvalidMutation {
            message: report
                .blocking_diagnostics(BundleValidationProfile::StrictCreation)
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        });
    }
    Ok(next_lifecycle)
}

fn validate_adapter_links(bundle: &BundleV2Contents) -> Result<(), LivePersistenceError> {
    for observation in &bundle.observations {
        if observation.adapter_record_ids.is_empty()
            || !observation.adapter_record_ids.iter().all(|adapter_id| {
                bundle.adapter_records.iter().any(|adapter| {
                    adapter.record_id == *adapter_id
                        && adapter.normalized_records.iter().any(|link| {
                            link.record_kind == antennabench_core::NormalizedRecordKind::Observation
                                && link.record_id == observation.observation_id
                        })
                })
            })
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "observation {:?} lacks reciprocal adapter evidence",
                    observation.observation_id
                ),
            });
        }
    }
    for record in &bundle.rig {
        if record.adapter_record_ids.is_empty()
            || !record.adapter_record_ids.iter().all(|adapter_id| {
                bundle.adapter_records.iter().any(|adapter| {
                    adapter.record_id == *adapter_id
                        && adapter.normalized_records.iter().any(|link| {
                            link.record_kind == antennabench_core::NormalizedRecordKind::Rig
                                && link.record_id == record.record_id
                        })
                })
            })
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "rig record {:?} lacks reciprocal adapter evidence",
                    record.record_id
                ),
            });
        }
    }
    for record in &bundle.propagation {
        if record.adapter_record_ids.is_empty()
            || !record.adapter_record_ids.iter().all(|adapter_id| {
                bundle.adapter_records.iter().any(|adapter| {
                    adapter.record_id == *adapter_id
                        && adapter.normalized_records.iter().any(|link| {
                            link.record_kind == antennabench_core::NormalizedRecordKind::Propagation
                                && link.record_id == record.record_id
                        })
                })
            })
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "propagation record {:?} lacks reciprocal adapter evidence",
                    record.record_id
                ),
            });
        }
    }
    Ok(())
}

fn is_lifecycle_payload(payload: &OperatorEventPayloadV2) -> bool {
    matches!(
        payload,
        OperatorEventPayloadV2::SessionStarted { .. }
            | OperatorEventPayloadV2::SessionInterrupted { .. }
            | OperatorEventPayloadV2::InterruptionDetected { .. }
            | OperatorEventPayloadV2::SessionResumed { .. }
            | OperatorEventPayloadV2::SessionEnded { .. }
            | OperatorEventPayloadV2::SessionAbandoned { .. }
    )
}

fn committed_mutation(
    bundle: &BundleV2Contents,
    mutation_id: &str,
) -> Option<Vec<LiveMutationMemberV2>> {
    let mut members = bundle
        .events
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation_id)
        .cloned()
        .map(LiveMutationMemberV2::Event)
        .chain(
            bundle
                .adapter_records
                .iter()
                .filter(|record| record.meta.mutation.mutation_id == mutation_id)
                .cloned()
                .map(LiveMutationMemberV2::AdapterRecord),
        )
        .chain(
            bundle
                .observations
                .iter()
                .filter(|record| record.meta.mutation.mutation_id == mutation_id)
                .cloned()
                .map(LiveMutationMemberV2::Observation),
        )
        .chain(
            bundle
                .rig
                .iter()
                .filter(|record| record.meta.mutation.mutation_id == mutation_id)
                .cloned()
                .map(LiveMutationMemberV2::Rig),
        )
        .chain(
            bundle
                .propagation
                .iter()
                .filter(|record| record.meta.mutation.mutation_id == mutation_id)
                .cloned()
                .map(LiveMutationMemberV2::Propagation),
        )
        .collect::<Vec<_>>();
    if members.is_empty() {
        None
    } else {
        members.sort_by_key(|member| member.meta().mutation.member_index);
        Some(members)
    }
}

fn same_business_members(
    existing: &[LiveMutationMemberV2],
    proposed: &[LiveMutationMemberV2],
) -> bool {
    if existing.len() != proposed.len() {
        return false;
    }
    let mut proposed = proposed.to_vec();
    proposed.sort_by_key(|member| member.meta().mutation.member_index);
    existing
        .iter()
        .zip(proposed)
        .all(|(existing, mut proposed)| {
            proposed.meta_mut().schema_version = existing.meta().schema_version;
            proposed.meta_mut().session_id = existing.meta().session_id.clone();
            proposed.meta_mut().recorded_at = existing.meta().recorded_at;
            proposed.meta_mut().mutation = existing.meta().mutation.clone();
            existing == &proposed
        })
}

fn append_line(
    path: &Path,
    stream: LiveStreamV2,
    bytes: &[u8],
    hooks: &dyn LivePersistenceHooks,
) -> Result<(), LivePersistenceError> {
    hooks
        .check(LivePersistencePoint::BeforeStreamWrite(stream))
        .map_err(|source| live_io("stream write failpoint", path, source))?;
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|source| live_io("open stream append", path, source))?;
    let split = bytes.len() / 2;
    file.write_all(&bytes[..split])
        .map_err(|source| live_io("append stream prefix", path, source))?;
    hooks
        .check(LivePersistencePoint::MidStreamWrite(stream))
        .map_err(|source| live_io("stream mid-write failpoint", path, source))?;
    file.write_all(&bytes[split..])
        .map_err(|source| live_io("append stream suffix", path, source))?;
    hooks
        .check(LivePersistencePoint::AfterStreamWrite(stream))
        .map_err(|source| live_io("stream post-write failpoint", path, source))?;
    hooks
        .check(LivePersistencePoint::BeforeStreamSync(stream))
        .map_err(|source| live_io("stream pre-sync failpoint", path, source))?;
    file.sync_all()
        .map_err(|source| live_io("synchronize stream", path, source))?;
    hooks
        .check(LivePersistencePoint::AfterStreamSync(stream))
        .map_err(|source| live_io("stream post-sync failpoint", path, source))?;
    Ok(())
}

fn preflight_live_budget(
    store: &BundleStore,
    checkpoint: &SessionStateV2,
    serialized: &[(LiveStreamV2, Vec<u8>)],
) -> Result<(), LivePersistenceError> {
    let profile = store.profile();
    let mut added_bytes = BTreeMap::<LiveStreamV2, u64>::new();
    let mut added_records = BTreeMap::<LiveStreamV2, u64>::new();
    for (stream, bytes) in serialized {
        let line_bytes = u64::try_from(bytes.len()).expect("usize fits u64");
        if line_bytes > profile.jsonl_line_bytes {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "{} member exceeds the {} byte JSONL line limit",
                    stream.checkpoint_name(),
                    profile.jsonl_line_bytes
                ),
            });
        }
        *added_bytes.entry(*stream).or_default() += line_bytes;
        *added_records.entry(*stream).or_default() += 1;
    }
    let mut total_bytes = 0_u64;
    let mut total_records = 0_u64;
    for stream in all_streams() {
        let current = stream_checkpoint(checkpoint, stream)?;
        let next_bytes = current
            .committed_bytes
            .checked_add(added_bytes.get(&stream).copied().unwrap_or_default())
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "stream byte accounting overflowed".into(),
            })?;
        let next_records = current
            .record_count
            .checked_add(added_records.get(&stream).copied().unwrap_or_default())
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "stream record accounting overflowed".into(),
            })?;
        if next_bytes > profile.jsonl_stream_bytes || next_records > profile.jsonl_stream_records {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "{} mutation would exceed the live stream resource profile",
                    stream.checkpoint_name()
                ),
            });
        }
        total_bytes = total_bytes.checked_add(next_bytes).ok_or_else(|| {
            LivePersistenceError::InvalidMutation {
                message: "modeled byte accounting overflowed".into(),
            }
        })?;
        total_records = total_records.checked_add(next_records).ok_or_else(|| {
            LivePersistenceError::InvalidMutation {
                message: "modeled record accounting overflowed".into(),
            }
        })?;
    }
    if total_bytes > profile.modeled_total_bytes || total_records > profile.modeled_total_records {
        return Err(LivePersistenceError::InvalidMutation {
            message: "mutation would exceed the aggregate live resource profile".into(),
        });
    }
    Ok(())
}

fn write_plan_file(
    path: &Path,
    kind: LivePlanFile,
    bytes: &[u8],
    hooks: &dyn LivePersistenceHooks,
) -> Result<(), LivePersistenceError> {
    hooks
        .check(LivePersistencePoint::BeforePlanWrite(kind))
        .map_err(|source| live_io("plan pre-write failpoint", path, source))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| live_io("create plan file", path, source))?;
    file.write_all(bytes)
        .map_err(|source| live_io("write plan file", path, source))?;
    hooks
        .check(LivePersistencePoint::AfterPlanWrite(kind))
        .map_err(|source| live_io("plan post-write failpoint", path, source))?;
    hooks
        .check(LivePersistencePoint::BeforePlanSync(kind))
        .map_err(|source| live_io("plan pre-sync failpoint", path, source))?;
    file.sync_all()
        .map_err(|source| live_io("synchronize plan file", path, source))?;
    hooks
        .check(LivePersistencePoint::AfterPlanSync(kind))
        .map_err(|source| live_io("plan post-sync failpoint", path, source))?;
    Ok(())
}

fn commit_checkpoint(
    root: &Path,
    current: &Path,
    checkpoint: &SessionStateV2,
    hooks: &dyn LivePersistenceHooks,
) -> Result<(), LivePersistenceError> {
    let temp = root.join(CHECKPOINT_TEMP);
    let previous = root.join(CHECKPOINT_PREVIOUS);
    if temp.exists() {
        fs::remove_file(&temp)
            .map_err(|source| live_io("remove stale checkpoint temp", &temp, source))?;
    }
    let bytes = serialize_json(checkpoint).map_err(invalid_serialization)?;
    hooks
        .check(LivePersistencePoint::BeforeCheckpointWrite)
        .map_err(|source| live_io("checkpoint pre-write failpoint", &temp, source))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|source| live_io("create checkpoint temp", &temp, source))?;
    file.write_all(&bytes)
        .map_err(|source| live_io("write checkpoint temp", &temp, source))?;
    hooks
        .check(LivePersistencePoint::AfterCheckpointWrite)
        .map_err(|source| live_io("checkpoint post-write failpoint", &temp, source))?;
    hooks
        .check(LivePersistencePoint::BeforeCheckpointSync)
        .map_err(|source| live_io("checkpoint pre-sync failpoint", &temp, source))?;
    file.sync_all()
        .map_err(|source| live_io("synchronize checkpoint temp", &temp, source))?;
    hooks
        .check(LivePersistencePoint::AfterCheckpointSync)
        .map_err(|source| live_io("checkpoint post-sync failpoint", &temp, source))?;
    drop(file);
    hooks
        .check(LivePersistencePoint::BeforeCheckpointReplace)
        .map_err(|source| live_io("checkpoint pre-replace failpoint", &temp, source))?;
    replace_checkpoint(&temp, current, &previous)
        .map_err(|source| live_io("atomically replace checkpoint", current, source))?;
    hooks
        .check(LivePersistencePoint::AfterCheckpointReplace)
        .map_err(|source| live_io("checkpoint post-replace failpoint", current, source))?;
    hooks
        .check(LivePersistencePoint::BeforeDirectorySync)
        .map_err(|source| live_io("directory pre-sync failpoint", root, source))?;
    sync_directory(root).map_err(|source| live_io("synchronize bundle directory", root, source))?;
    hooks
        .check(LivePersistencePoint::AfterDirectorySync)
        .map_err(|source| live_io("directory post-sync failpoint", root, source))?;
    hooks
        .check(LivePersistencePoint::BeforeCheckpointVerify)
        .map_err(|source| live_io("checkpoint pre-verify failpoint", current, source))?;
    let reopened = read_state(current)?;
    if &reopened != checkpoint {
        return Err(LivePersistenceError::CheckpointVerification {
            message: "reopened checkpoint differs from the promoted value".into(),
        });
    }
    hooks
        .check(LivePersistencePoint::AfterCheckpointVerify)
        .map_err(|source| live_io("checkpoint post-verify failpoint", current, source))?;
    Ok(())
}

fn checkpoint_from_paths(
    store: &BundleStore,
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
) -> Result<SessionStateV2, LivePersistenceError> {
    let mut state = bundle.session_state.clone();
    state.streams = BTreeMap::new();
    for (stream, count, last_id) in [
        (
            LiveStreamV2::Events,
            bundle.events.len(),
            bundle.events.last().map(|record| record.event_id.clone()),
        ),
        (
            LiveStreamV2::AdapterRecords,
            bundle.adapter_records.len(),
            bundle
                .adapter_records
                .last()
                .map(|record| record.record_id.clone()),
        ),
        (
            LiveStreamV2::Observations,
            bundle.observations.len(),
            bundle
                .observations
                .last()
                .map(|record| record.observation_id.clone()),
        ),
        (
            LiveStreamV2::Rig,
            bundle.rig.len(),
            bundle.rig.last().map(|record| record.record_id.clone()),
        ),
        (
            LiveStreamV2::Propagation,
            bundle.propagation.len(),
            bundle
                .propagation
                .last()
                .map(|record| record.record_id.clone()),
        ),
    ] {
        let bytes = read_bounded(
            store,
            stream_path(paths, stream),
            store.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            ResourceOperation::Write,
        )?;
        state.streams.insert(
            stream.checkpoint_name().into(),
            checkpoint_for_bytes(&bytes, count, last_id),
        );
    }
    Ok(state)
}

fn verify_exact_checkpoint(
    store: &BundleStore,
    checkpoint: &SessionStateV2,
    paths: &ResolvedBundlePathsV2,
) -> Result<(), LivePersistenceError> {
    for stream in all_streams() {
        let expected = checkpoint
            .streams
            .get(stream.checkpoint_name())
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: format!("checkpoint is missing {}", stream.checkpoint_name()),
            })?;
        let path = stream_path(paths, stream);
        let bytes = read_bounded(
            store,
            path,
            store.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            ResourceOperation::Write,
        )?;
        if u64::try_from(bytes.len()).ok() != Some(expected.committed_bytes) {
            return Err(LivePersistenceError::RecoveryRequired {
                message: format!(
                    "{} has bytes outside its committed head",
                    stream.checkpoint_name()
                ),
            });
        }
        if sha256_hex(&bytes) != expected.committed_sha256 {
            return Err(LivePersistenceError::ExternalModification {
                message: format!(
                    "{} committed prefix digest changed",
                    stream.checkpoint_name()
                ),
            });
        }
    }
    for (name, path, expected) in [
        (
            "station",
            &paths.station,
            &checkpoint.active_plan.station_sha256,
        ),
        (
            "antennas",
            &paths.antennas,
            &checkpoint.active_plan.antennas_sha256,
        ),
        (
            "schedule",
            &paths.schedule,
            &checkpoint.active_plan.schedule_sha256,
        ),
    ] {
        let bytes = read_bounded(
            store,
            path,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Write,
        )?;
        if sha256_hex(&bytes) != *expected {
            return Err(LivePersistenceError::ExternalModification {
                message: format!("active plan {name} digest changed"),
            });
        }
    }
    Ok(())
}

fn read_state(path: &Path) -> Result<SessionStateV2, LivePersistenceError> {
    let size = fs::metadata(path)
        .map_err(|source| live_io("inspect checkpoint", path, source))?
        .len();
    if size > CHECKPOINT_MAX_BYTES {
        return Err(LivePersistenceError::CheckpointVerification {
            message: format!(
                "checkpoint {} exceeds the {} byte limit",
                path.display(),
                CHECKPOINT_MAX_BYTES
            ),
        });
    }
    let bytes = fs::read(path).map_err(|source| live_io("read checkpoint", path, source))?;
    serde_json::from_slice(&bytes).map_err(|source| LivePersistenceError::CheckpointVerification {
        message: format!("{} is not a valid checkpoint: {source}", path.display()),
    })
}

fn stream_path(paths: &ResolvedBundlePathsV2, stream: LiveStreamV2) -> &Path {
    match stream {
        LiveStreamV2::AdapterRecords => &paths.adapter_records,
        LiveStreamV2::Observations => &paths.observations,
        LiveStreamV2::Events => &paths.events,
        LiveStreamV2::Rig => &paths.rig,
        LiveStreamV2::Propagation => &paths.propagation,
    }
}

fn all_streams() -> [LiveStreamV2; 5] {
    [
        LiveStreamV2::AdapterRecords,
        LiveStreamV2::Observations,
        LiveStreamV2::Events,
        LiveStreamV2::Rig,
        LiveStreamV2::Propagation,
    ]
}

fn validate_generation_id(value: &str) -> Result<(), LivePersistenceError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Err(LivePersistenceError::InvalidMutation {
            message: "plan generation ID must be a nonempty ASCII path component".into(),
        })
    } else {
        Ok(())
    }
}

fn invalid_serialization(error: serde_json::Error) -> LivePersistenceError {
    LivePersistenceError::InvalidMutation {
        message: format!("JSON serialization failed: {error}"),
    }
}

fn live_io(operation: &'static str, path: &Path, source: io::Error) -> LivePersistenceError {
    LivePersistenceError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(unix)]
fn replace_checkpoint(temp: &Path, current: &Path, previous: &Path) -> io::Result<()> {
    let previous_temp = previous.with_extension("json.next");
    if previous_temp.exists() {
        fs::remove_file(&previous_temp)?;
    }
    fs::copy(current, &previous_temp)?;
    sync_regular_file(&previous_temp)?;
    fs::rename(&previous_temp, previous)?;
    fs::rename(temp, current)
}

#[cfg(unix)]
fn publish_new_bundle(staging: &Path, destination: &Path) -> io::Result<()> {
    // The caller checks for an existing destination immediately before this
    // same-volume rename. Existing paths are never intentionally replaced.
    fs::rename(staging, destination)
}

#[cfg(windows)]
fn replace_checkpoint(temp: &Path, current: &Path, previous: &Path) -> io::Result<()> {
    let previous_temp = previous.with_extension("json.next");
    remove_file_if_present(&previous_temp)?;
    fs::copy(current, &previous_temp)?;
    sync_regular_file(&previous_temp)?;
    move_file_write_through(&previous_temp, previous)?;
    move_file_write_through(temp, current)
}

#[cfg(windows)]
fn publish_new_bundle(staging: &Path, destination: &Path) -> io::Result<()> {
    move_file_write_through_without_replacement(staging, destination)
}

#[cfg(unix)]
fn probe_live_persistence(path: &Path) -> io::Result<()> {
    sync_directory(path)
}

#[cfg(windows)]
fn probe_live_persistence(path: &Path) -> io::Result<()> {
    let probe_id = Uuid::new_v4().simple();
    let current = path.join(format!(".antennabench-durability-{probe_id}.current"));
    let replacement = path.join(format!(".antennabench-durability-{probe_id}.next"));
    let previous = path.join(format!(".antennabench-durability-{probe_id}.previous"));
    let previous_temp = previous.with_extension("json.next");
    let result = (|| {
        write_synced_probe(&current, b"current")?;
        write_synced_probe(&replacement, b"replacement")?;
        replace_checkpoint(&replacement, &current, &previous)?;
        if fs::read(&current)? != b"replacement" {
            return Err(io::Error::other(
                "write-through replacement did not preserve the replacement bytes",
            ));
        }
        if fs::read(&previous)? != b"current" {
            return Err(io::Error::other(
                "write-through replacement did not preserve the prior bytes",
            ));
        }
        Ok(())
    })();
    let cleanup_current = remove_file_if_present(&current);
    let cleanup_replacement = remove_file_if_present(&replacement);
    let cleanup_previous = remove_file_if_present(&previous);
    let cleanup_previous_temp = remove_file_if_present(&previous_temp);
    result
        .and(cleanup_current)
        .and(cleanup_replacement)
        .and(cleanup_previous)
        .and(cleanup_previous_temp)
}

#[cfg(windows)]
fn write_synced_probe(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(windows)]
fn move_file_write_through(source: &Path, destination: &Path) -> io::Result<()> {
    move_file_write_through_with_flags(source, destination, true)
}

#[cfg(windows)]
fn move_file_write_through_without_replacement(
    source: &Path,
    destination: &Path,
) -> io::Result<()> {
    move_file_write_through_with_flags(source, destination, false)
}

#[cfg(windows)]
fn move_file_write_through_with_flags(
    source: &Path,
    destination: &Path,
    replace_existing: bool,
) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace_existing {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn remove_file_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn sync_regular_file(path: &Path) -> io::Result<()> {
    OpenOptions::new().write(true).open(path)?.sync_all()
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_directory(_path: &Path) -> io::Result<()> {
    // Windows has no supported directory-fsync equivalent. Regular files are
    // synchronized before they become reachable, while checkpoint promotion
    // and the capability probe use MoveFileExW with MOVEFILE_WRITE_THROUGH as
    // the metadata durability barrier.
    Ok(())
}
