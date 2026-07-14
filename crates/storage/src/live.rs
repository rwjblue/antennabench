use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use antennabench_core::{
    validate_bundle_report, validate_lifecycle_transition_v2, validate_operator_event_append_v2,
    AdapterRecordV2, AntennasFile, BundleV2Contents, BundleValidationProfile, ObservationRecordV2,
    OperatorEventPayloadV2, OperatorEventV2, PlanGenerationV2, PropagationRecordV2, RecordMetaV2,
    RigRecordV2, Schedule, SessionLifecycleV2, SessionStateV2, Station, SCHEMA_VERSION_V2,
};
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    v2::{checkpoint_for_bytes, serialize_json, sha256_hex, ResolvedBundlePathsV2},
    BundleStore, BundleStoreError,
};

const LOCK_FILE: &str = ".antennabench.lock";
const CHECKPOINT_TEMP: &str = ".session-state.next.json";
const CHECKPOINT_PREVIOUS: &str = "session-state.previous.json";

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

        sync_directory(self.root()).map_err(|source| LivePersistenceError::Capability {
            message: format!("bundle directory synchronization failed: {source}"),
        })?;
        let bundle = self.read_v2()?;
        let paths = self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        verify_exact_checkpoint(&bundle.session_state, &paths)?;
        Ok(LiveSessionV2 {
            store: self.clone(),
            _lock: lock,
            hooks,
            bundle,
            paths,
            frozen: false,
        })
    }
}

impl LiveSessionV2 {
    pub fn checkpoint(&self) -> &SessionStateV2 {
        &self.bundle.session_state
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
            let mut next = checkpoint_from_paths(&next_bundle, &self.paths)?;
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

    pub fn commit_plan(
        &mut self,
        plan: PlanCommitV2,
    ) -> Result<CommitReceiptV2, LivePersistenceError> {
        self.ensure_mutable()?;
        self.refresh_if_advanced()?;
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
        commit_checkpoint(
            self.store.root(),
            &self.paths.session_state,
            &next,
            self.hooks.as_ref(),
        )?;
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
        if let Err(error) = verify_exact_checkpoint(&actual, &self.paths) {
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
        let bytes = fs::read(stream_path(paths, stream)).map_err(|source| {
            live_io("read appended stream", stream_path(paths, stream), source)
        })?;
        state.streams.insert(
            stream.checkpoint_name().into(),
            checkpoint_for_bytes(&bytes, count, last_id),
        );
    }
    Ok(state)
}

fn verify_exact_checkpoint(
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
        let bytes =
            fs::read(path).map_err(|source| live_io("read checkpointed stream", path, source))?;
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
        let bytes = fs::read(path).map_err(|source| live_io("read active plan", path, source))?;
        if sha256_hex(&bytes) != *expected {
            return Err(LivePersistenceError::ExternalModification {
                message: format!("active plan {name} digest changed"),
            });
        }
    }
    Ok(())
}

fn read_state(path: &Path) -> Result<SessionStateV2, LivePersistenceError> {
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
    File::open(&previous_temp)?.sync_all()?;
    fs::rename(&previous_temp, previous)?;
    fs::rename(temp, current)
}

#[cfg(windows)]
fn replace_checkpoint(temp: &Path, current: &Path, previous: &Path) -> io::Result<()> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt};

    unsafe extern "system" {
        fn ReplaceFileW(
            replaced_file_name: *const u16,
            replacement_file_name: *const u16,
            backup_file_name: *const u16,
            replace_flags: u32,
            exclude: *mut c_void,
            reserved: *mut c_void,
        ) -> i32;
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    if previous.exists() {
        fs::remove_file(previous)?;
    }
    let current = wide(current);
    let temp = wide(temp);
    let previous = wide(previous);
    let result = unsafe {
        ReplaceFileW(
            current.as_ptr(),
            temp.as_ptr(),
            previous.as_ptr(),
            0x0000_0001,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?
        .sync_all()
}
