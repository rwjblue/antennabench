use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use antennabench_core::{
    reduce_operator_events_v3, validate_bundle_report, validate_machine_identity, AdapterRecordV2,
    AntennasFile, AttachmentReference, BundleManifestV2, BundleV2Contents, BundleV3Contents,
    BundleValidationProfile, EventTimeBasisV2, MutationMember, ObservationRecordV2,
    OperatorEventPayloadV2, OperatorEventPayloadV3, OperatorEventV2, OperatorEventV3,
    PlanGenerationV2, PropagationRecordV2, Provenance, RecordMetaV2, RecordMetaV3, RecordSource,
    RigRecordV2, RigRecordV3, Schedule, SessionLifecycleV2, SessionStateV2, Station,
    SCHEMA_VERSION_V2, SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5,
};
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    v2::{serialize_json, sha256_hex, ResolvedBundlePathsV2},
    BundleStore, BundleStoreError,
};

mod attachments;
mod checkpoint;
mod durability;
mod mutation;
mod recovery;

use attachments::{copy_checkpointed_attachments, durable_attachment};
use checkpoint::{
    all_streams, checkpoint_from_paths, commit_checkpoint, load_checkpointed_bundle,
    load_recovery_bundle, read_json_file, read_state, stream_path, verify_committed_prefixes,
    verify_exact_checkpoint, write_plan_file,
};
use durability::{
    probe_live_persistence, publish_new_bundle, remove_file_if_present, sync_directory,
    sync_regular_file,
};
use mutation::{
    append_line, committed_mutation, committed_v5_antenna_control, preflight_live_budget,
    prepare_mutation, prepare_v3_evidence, rollback_v3_streams, same_business_members,
    same_v3_event_business_value, same_v3_evidence_business_value, serialize_v3_lines,
    v3_committed_evidence, v3_other_stream_has_mutation, validate_generation_id, validate_mutation,
    validate_v3_evidence,
};
use recovery::{
    parse_tail_mutation, preserve_tails, preserve_tails_for, read_stream_tails,
    recover_checkpoint_temp, recover_checkpoint_temp_for, recover_pending_plan_generation,
    truncate_tails, PlanGenerationMetadataV2,
};

const LOCK_FILE: &str = ".antennabench.lock";

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

impl Drop for LiveSessionV2 {
    fn drop(&mut self) {
        let _ = File::unlock(&self._lock);
    }
}

impl Drop for LiveSessionV3 {
    fn drop(&mut self) {
        let _ = File::unlock(&self._lock);
    }
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
