use std::{
    fs::{File, OpenOptions},
    io,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use antennabench_core::{
    v2::{
        AdapterInput, EventTimeBasisV2, MutationMember, NormalizedRecordKind, NormalizedRecordLink,
        OperatorEventPayloadV2, OperatorEventV2, Provenance, RecordMetaV2, SessionLifecycleV2,
        V2_BUNDLE_SUFFIX,
    },
    RecordSource, SCHEMA_VERSION_V2,
};
use antennabench_storage::{
    BundleStore, LiveMutationMemberV2, LiveMutationV2, LivePersistenceError, LivePersistenceHooks,
    LivePersistencePoint, LivePlanFile, LiveSessionV2, LiveStreamV2, PlanCommitV2,
    RecoveryDispositionV2, SystemLivePersistenceHooks,
};
use chrono::{DateTime, TimeZone, Utc};

#[derive(Debug)]
struct DeterministicHooks {
    now: DateTime<Utc>,
    points: Mutex<Vec<LivePersistencePoint>>,
    fail_once: Mutex<Option<LivePersistencePoint>>,
    next_id: Mutex<u64>,
}

impl DeterministicHooks {
    fn new() -> Self {
        Self {
            now: Utc.with_ymd_and_hms(2026, 7, 14, 20, 0, 0).unwrap(),
            points: Mutex::new(Vec::new()),
            fail_once: Mutex::new(None),
            next_id: Mutex::new(1),
        }
    }

    fn fail_once_at(&self, point: LivePersistencePoint) {
        *self.fail_once.lock().unwrap() = Some(point);
    }

    fn points(&self) -> Vec<LivePersistencePoint> {
        self.points.lock().unwrap().clone()
    }
}

impl LivePersistenceHooks for DeterministicHooks {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }

    fn new_id(&self, kind: &str) -> String {
        let mut next = self.next_id.lock().unwrap();
        let id = format!("{kind}-{next:04}");
        *next += 1;
        id
    }

    fn sync_all(&self, _file: &File) -> io::Result<()> {
        Ok(())
    }

    fn check(&self, point: LivePersistencePoint) -> io::Result<()> {
        self.points.lock().unwrap().push(point);
        let mut fail = self.fail_once.lock().unwrap();
        if fail.as_ref() == Some(&point) {
            *fail = None;
            Err(io::Error::other("injected persistence failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn system_hooks_smoke_exercises_platform_sync() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sync-smoke");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(b"durability smoke").unwrap();

    SystemLivePersistenceHooks.sync_all(&file).unwrap();
}

#[test]
fn checkpointed_creation_publishes_only_a_verified_complete_bundle() {
    let temp = tempfile::tempdir().unwrap();
    let source = ready_v2_store(temp.path());
    let bundle = source.read_v2().unwrap();
    let destination = temp.path().join(format!("created{V2_BUNDLE_SUFFIX}"));
    let created = BundleStore::new(&destination);

    created.create_v2_checkpointed(&bundle).unwrap();

    assert_eq!(created.read_v2_checkpointed().unwrap(), bundle);
    assert!(temp.path().read_dir().unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".antennabench-creating-")
    }));
}

#[test]
fn checkpointed_creation_preserves_existing_destinations_and_cleans_failures() {
    let temp = tempfile::tempdir().unwrap();
    let source = ready_v2_store(temp.path());
    let bundle = source.read_v2().unwrap();
    let destination = temp.path().join(format!("existing{V2_BUNDLE_SUFFIX}"));
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("owner.txt"), b"keep me").unwrap();

    let error = BundleStore::new(&destination)
        .create_v2_checkpointed(&bundle)
        .unwrap_err();
    assert!(matches!(
        error,
        LivePersistenceError::Store(
            antennabench_storage::BundleStoreError::DestinationExists { .. }
        )
    ));
    assert_eq!(
        std::fs::read(destination.join("owner.txt")).unwrap(),
        b"keep me"
    );

    let mut invalid = bundle;
    invalid.station.callsign.clear();
    let invalid_destination = temp.path().join(format!("invalid{V2_BUNDLE_SUFFIX}"));
    assert!(BundleStore::new(&invalid_destination)
        .create_v2_checkpointed(&invalid)
        .is_err());
    assert!(!invalid_destination.exists());
    assert!(temp.path().read_dir().unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".antennabench-creating-")
    }));
}

#[test]
fn writer_lock_append_idempotency_and_plan_freeze_are_coherent() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    assert!(matches!(
        store.open_v2_writer(),
        Err(LivePersistenceError::WriterBusy)
    ));
    assert!(matches!(
        store.read_v2_checkpointed(),
        Err(LivePersistenceError::WriterBusy)
    ));

    let revision = writer.checkpoint().revision;
    let start = event_member(
        &writer,
        "event-start",
        0,
        OperatorEventPayloadV2::SessionStarted { note: None },
    );
    let start_mutation = LiveMutationV2 {
        expected_revision: revision,
        mutation_id: "mutation-start".into(),
        members: vec![start],
    };
    let receipt = writer.append(start_mutation.clone()).unwrap();
    assert_eq!(receipt.revision, revision + 1);
    assert_eq!(writer.checkpoint().lifecycle, SessionLifecycleV2::Running);

    let retry = writer.append(start_mutation).unwrap();
    assert!(retry.idempotent);
    assert_eq!(retry.revision, receipt.revision);
    assert!(matches!(
        writer.append(LiveMutationV2 {
            expected_revision: revision,
            mutation_id: "different-stale-mutation".into(),
            members: vec![event_member(
                &writer,
                "event-note-stale",
                0,
                OperatorEventPayloadV2::NoteAdded {
                    note: "stale".into()
                }
            )],
        }),
        Err(LivePersistenceError::StaleRevision { .. })
    ));
    assert!(matches!(
        writer.commit_plan(PlanCommitV2 {
            expected_revision: receipt.revision,
            generation_id: "plan-after-start".into(),
            station: store.read_v2().unwrap().station,
            antennas: store.read_v2().unwrap().antennas,
            schedule: store.read_v2().unwrap().schedule,
        }),
        Err(LivePersistenceError::PlanFrozen { .. })
    ));

    drop(writer);
    let reopened = store.open_v2_writer().unwrap();
    assert_eq!(reopened.checkpoint().revision, receipt.revision);
    assert_eq!(reopened.checkpoint().lifecycle, SessionLifecycleV2::Running);
}

#[test]
fn raw_and_normalized_members_commit_together_in_deterministic_order() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);

    let template = BundleStore::new(temp.path().join(format!("upgraded{V2_BUNDLE_SUFFIX}")))
        .read_v2()
        .unwrap();
    let mut adapter = template.adapter_records[0].clone();
    adapter.record_id = "adapter-live-0001".into();
    adapter.meta.mutation.member_index = 0;
    adapter.normalized_records = vec![NormalizedRecordLink {
        record_kind: NormalizedRecordKind::Observation,
        record_id: "observation-live-0001".into(),
    }];
    let mut observation = template.observations[0].clone();
    observation.observation_id = "observation-live-0001".into();
    observation.meta.mutation.member_index = 1;
    observation.adapter_record_ids = vec![adapter.record_id.clone()];
    observation.slot_id = None;
    observation.slot_label = None;
    observation.slot_confidence = Some(0.0);

    let revision = writer.checkpoint().revision;
    let receipt = writer
        .append(LiveMutationV2 {
            expected_revision: revision,
            mutation_id: "mutation-acquisition-0001".into(),
            members: vec![
                LiveMutationMemberV2::Observation(observation),
                LiveMutationMemberV2::AdapterRecord(adapter),
            ],
        })
        .unwrap();
    assert_eq!(receipt.revision, revision + 1);

    let points = hooks.points();
    let adapter_write = points
        .iter()
        .position(|point| {
            *point
                == LivePersistencePoint::BeforeStreamWrite(
                    antennabench_storage::LiveStreamV2::AdapterRecords,
                )
        })
        .unwrap();
    let observation_write = points
        .iter()
        .position(|point| {
            *point
                == LivePersistencePoint::BeforeStreamWrite(
                    antennabench_storage::LiveStreamV2::Observations,
                )
        })
        .unwrap();
    assert!(adapter_write < observation_write);

    let reopened = store.read_v2().unwrap();
    assert!(reopened
        .adapter_records
        .iter()
        .any(|record| record.record_id == "adapter-live-0001"));
    assert!(reopened
        .observations
        .iter()
        .any(|record| record.observation_id == "observation-live-0001"));
}

#[test]
fn failure_between_raw_and_normalized_members_rolls_back_the_whole_batch() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);
    let baseline = writer.checkpoint().revision;

    let template = BundleStore::new(temp.path().join(format!("upgraded{V2_BUNDLE_SUFFIX}")))
        .read_v2()
        .unwrap();
    let mut adapter = template.adapter_records[0].clone();
    adapter.record_id = "adapter-rolled-back".into();
    adapter.meta.mutation.member_index = 0;
    adapter.normalized_records = vec![NormalizedRecordLink {
        record_kind: NormalizedRecordKind::Observation,
        record_id: "observation-rolled-back".into(),
    }];
    let mut observation = template.observations[0].clone();
    observation.observation_id = "observation-rolled-back".into();
    observation.meta.mutation.member_index = 1;
    observation.adapter_record_ids = vec![adapter.record_id.clone()];
    observation.slot_id = None;
    observation.slot_label = None;
    observation.slot_confidence = Some(0.0);

    hooks.fail_once_at(LivePersistencePoint::BeforeStreamWrite(
        LiveStreamV2::Observations,
    ));
    assert!(writer
        .append(LiveMutationV2 {
            expected_revision: baseline,
            mutation_id: "mutation-rolled-back-batch".into(),
            members: vec![
                LiveMutationMemberV2::AdapterRecord(adapter),
                LiveMutationMemberV2::Observation(observation),
            ],
        })
        .is_err());
    drop(writer);

    let snapshot = store.read_v2_checkpointed().unwrap();
    assert_eq!(snapshot.session_state.revision, baseline);
    assert!(!snapshot
        .adapter_records
        .iter()
        .any(|record| record.record_id == "adapter-rolled-back"));
    assert!(!snapshot
        .observations
        .iter()
        .any(|record| record.observation_id == "observation-rolled-back"));
    assert!(store.read_v2().is_ok());
}

#[test]
fn failed_attachment_mutation_removes_uncommitted_exact_response() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);
    let baseline = writer.checkpoint().revision;
    let template = BundleStore::new(temp.path().join(format!("upgraded{V2_BUNDLE_SUFFIX}")))
        .read_v2()
        .unwrap();
    let mut adapter = template.adapter_records[0].clone();
    adapter.record_id = "adapter-attachment-rolled-back".into();
    adapter.meta.mutation.member_index = 0;
    adapter.normalized_records.clear();
    let mut digest = None;

    hooks.fail_once_at(LivePersistencePoint::BeforeStreamWrite(
        LiveStreamV2::AdapterRecords,
    ));
    let error = writer
        .append_with_attachment(
            br#"{"rows":[]}"#,
            "application/json",
            None,
            Some("clickhouse-format-json".into()),
            Some("selected.json".into()),
            |attachment| {
                digest = Some(attachment.sha256.clone());
                adapter.input = AdapterInput::Attachment { attachment };
                LiveMutationV2 {
                    expected_revision: baseline,
                    mutation_id: "mutation-attachment-rollback".into(),
                    members: vec![LiveMutationMemberV2::AdapterRecord(adapter)],
                }
            },
        )
        .unwrap_err();
    assert!(matches!(error, LivePersistenceError::Io { .. }));
    assert_eq!(writer.checkpoint().revision, baseline);
    let attachment = store
        .root()
        .join("attachments/sha256")
        .join(digest.unwrap());
    assert!(!attachment.exists());
}

#[test]
fn failpoints_leave_the_previous_or_complete_next_checkpoint() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);
    let baseline = writer.checkpoint().revision;

    hooks.fail_once_at(LivePersistencePoint::MidStreamWrite(
        antennabench_storage::LiveStreamV2::Events,
    ));
    let failed = note_mutation(&writer, baseline, "mutation-torn", "event-torn");
    assert!(writer.append(failed).is_err());
    assert_eq!(store.read_v2().unwrap().session_state.revision, baseline);

    hooks.fail_once_at(LivePersistencePoint::BeforeAcknowledge);
    let committed = note_mutation(&writer, baseline, "mutation-lost-reply", "event-lost-reply");
    assert!(writer.append(committed.clone()).is_err());
    assert_eq!(
        store.read_v2().unwrap().session_state.revision,
        baseline + 1
    );
    let retry = writer.append(committed).unwrap();
    assert!(retry.idempotent);
    assert_eq!(retry.revision, baseline + 1);
}

#[test]
fn draft_plan_generation_is_create_new_and_checkpoint_selected() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks).unwrap();
    let baseline = store.read_v2().unwrap();
    let root_station = std::fs::read(store.root().join("station.json")).unwrap();
    let mut station = baseline.station.clone();
    station.operator_notes = Some("Checkpoint-selected generation".into());
    let plan = PlanCommitV2 {
        expected_revision: baseline.session_state.revision,
        generation_id: "plan-generation-0002".into(),
        station: station.clone(),
        antennas: baseline.antennas,
        schedule: baseline.schedule,
    };

    let receipt = writer.commit_plan(plan.clone()).unwrap();
    assert_eq!(receipt.revision, baseline.session_state.revision + 1);
    assert_eq!(
        std::fs::read(store.root().join("station.json")).unwrap(),
        root_station
    );
    let reopened = store.read_v2().unwrap();
    assert_eq!(reopened.station, station);
    assert_eq!(
        reopened.session_state.active_plan.generation_id,
        "plan-generation-0002"
    );
    assert!(store.root().join("session-state.previous.json").is_file());
    let retry = writer.commit_plan(plan).unwrap();
    assert!(retry.idempotent);
    assert_eq!(retry.revision, receipt.revision);
}

#[test]
fn complete_tail_recovers_forward_once_and_records_interruption() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);
    let baseline = writer.checkpoint().revision;
    let session_id = store.read_v2().unwrap().manifest.session_id;
    drop(writer);

    let event = durable_note_event(
        &session_id,
        "mutation-recovery-forward",
        "event-recovery-forward",
    );
    append_jsonl(&store.root().join("events.jsonl"), &event);

    let report = store.recover_v2_with_hooks(hooks).unwrap();
    assert_eq!(report.disposition, RecoveryDispositionV2::RolledForward);
    assert_eq!(report.starting_revision, baseline);
    assert_eq!(report.recovered_revision, baseline + 1);
    assert_eq!(report.final_revision, baseline + 2);
    assert!(report.interruption.is_some());
    assert!(report.artifacts.is_empty());

    let reopened = store.read_v2().unwrap();
    assert_eq!(
        reopened.session_state.lifecycle,
        SessionLifecycleV2::Interrupted
    );
    assert_eq!(
        reopened
            .events
            .iter()
            .filter(|event| event.event_id == "event-recovery-forward")
            .count(),
        1
    );
    assert_eq!(
        reopened
            .events
            .iter()
            .filter(|event| matches!(
                event.payload,
                OperatorEventPayloadV2::InterruptionDetected { .. }
            ))
            .count(),
        1
    );
    let second = store.recover_v2().unwrap();
    assert_eq!(second.disposition, RecoveryDispositionV2::Clean);
    assert_eq!(second.final_revision, report.final_revision);
    assert!(second.interruption.is_none());
}

#[test]
fn torn_tail_is_preserved_exactly_before_rollback() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);
    let baseline = writer.checkpoint().revision;
    let session_id = store.read_v2().unwrap().manifest.session_id;
    drop(writer);

    let event = durable_note_event(&session_id, "mutation-torn-tail", "event-torn-tail");
    let complete = jsonl_bytes(&event);
    let torn = &complete[..complete.len() / 2];
    OpenOptions::new()
        .append(true)
        .open(store.root().join("events.jsonl"))
        .unwrap()
        .write_all(torn)
        .unwrap();

    let snapshot = store.read_v2_checkpointed().unwrap();
    assert_eq!(snapshot.session_state.revision, baseline);
    assert!(!snapshot
        .events
        .iter()
        .any(|event| event.event_id == "event-torn-tail"));

    let report = store.recover_v2_with_hooks(hooks).unwrap();
    assert_eq!(report.disposition, RecoveryDispositionV2::RolledBack);
    assert_eq!(report.recovered_revision, baseline);
    assert_eq!(report.final_revision, baseline + 1);
    assert_eq!(report.artifacts.len(), 1);
    let artifact = &report.artifacts[0];
    assert_eq!(artifact.source, "events");
    assert_eq!(
        store.read_attachment(&artifact.raw_attachment).unwrap(),
        torn
    );
    let metadata = store
        .read_attachment(&artifact.metadata_attachment)
        .unwrap();
    let metadata: serde_json::Value = serde_json::from_slice(&metadata).unwrap();
    assert_eq!(metadata["source"], "events");
    assert_eq!(
        metadata["raw_attachment"]["sha256"],
        artifact.raw_attachment.sha256
    );
    assert!(store.read_v2().is_ok());
}

#[test]
fn committed_prefix_corruption_fails_closed_without_tail_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks).unwrap();
    start_session(&mut writer);
    let revision = writer.checkpoint().revision;
    writer
        .append(note_mutation(
            &writer,
            revision,
            "mutation-after-start",
            "event-after-start",
        ))
        .unwrap();
    drop(writer);

    let path = store.root().join("events.jsonl");
    let mut bytes = std::fs::read(&path).unwrap();
    let index = bytes.iter().position(|byte| *byte == b'e').unwrap();
    bytes[index] = b'E';
    std::fs::write(&path, &bytes).unwrap();

    assert!(matches!(
        store.recover_v2(),
        Err(LivePersistenceError::ExternalModification { .. })
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn duplicate_committed_tail_is_removed_as_an_idempotent_retry() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);
    let baseline = writer.checkpoint().revision;
    drop(writer);
    let bundle = store.read_v2().unwrap();
    let start = bundle
        .events
        .iter()
        .find(|event| event.meta.mutation.mutation_id == "mutation-start")
        .unwrap();
    append_jsonl(&store.root().join("events.jsonl"), start);

    let report = store.recover_v2_with_hooks(hooks).unwrap();
    assert_eq!(
        report.disposition,
        RecoveryDispositionV2::IdempotentTailRemoved
    );
    assert_eq!(report.recovered_revision, baseline);
    assert!(report.artifacts.is_empty());
    assert_eq!(
        store
            .read_v2()
            .unwrap()
            .events
            .iter()
            .filter(|event| event.meta.mutation.mutation_id == "mutation-start")
            .count(),
        1
    );
}

#[test]
fn recovery_selects_the_previous_valid_checkpoint_when_current_is_malformed() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);
    drop(writer);
    std::fs::write(store.root().join("session-state.json"), b"{malformed").unwrap();

    let report = store.recover_v2_with_hooks(hooks).unwrap();
    assert_eq!(report.starting_revision, 1);
    assert_eq!(report.disposition, RecoveryDispositionV2::RolledForward);
    assert_eq!(report.recovered_revision, 2);
    assert_eq!(report.final_revision, 3);
    let reopened = store.read_v2().unwrap();
    assert_eq!(
        reopened.session_state.lifecycle,
        SessionLifecycleV2::Interrupted
    );
    assert_eq!(reopened.session_state.revision, 3);
}

#[test]
fn incomplete_cross_stream_tail_is_preserved_and_not_partially_committed() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);
    let template = BundleStore::new(temp.path().join(format!("upgraded{V2_BUNDLE_SUFFIX}")))
        .read_v2()
        .unwrap();
    let mut adapter = template.adapter_records[0].clone();
    adapter.record_id = "adapter-incomplete-tail".into();
    adapter.meta.session_id = store.read_v2().unwrap().manifest.session_id;
    adapter.meta.mutation = MutationMember {
        mutation_id: "mutation-incomplete-tail".into(),
        member_index: 0,
        member_count: 2,
    };
    adapter.normalized_records = vec![NormalizedRecordLink {
        record_kind: NormalizedRecordKind::Observation,
        record_id: "observation-missing-tail".into(),
    }];
    drop(writer);
    let raw = jsonl_bytes(&adapter);
    OpenOptions::new()
        .append(true)
        .open(store.root().join("adapter-records.jsonl"))
        .unwrap()
        .write_all(&raw)
        .unwrap();

    let report = store.recover_v2_with_hooks(hooks).unwrap();
    assert_eq!(report.disposition, RecoveryDispositionV2::RolledBack);
    assert_eq!(report.artifacts.len(), 1);
    assert_eq!(report.artifacts[0].source, "adapter_records");
    assert_eq!(
        store
            .read_attachment(&report.artifacts[0].raw_attachment)
            .unwrap(),
        raw
    );
    assert!(!store
        .read_v2()
        .unwrap()
        .adapter_records
        .iter()
        .any(|record| record.record_id == "adapter-incomplete-tail"));
}

#[test]
fn ignored_advisory_lock_external_append_freezes_the_writer() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks).unwrap();
    start_session(&mut writer);
    let baseline = writer.checkpoint().revision;
    let session_id = store.read_v2().unwrap().manifest.session_id;
    let external = durable_note_event(
        &session_id,
        "mutation-external-writer",
        "event-external-writer",
    );
    append_jsonl(&store.root().join("events.jsonl"), &external);

    assert!(matches!(
        writer.append(note_mutation(
            &writer,
            baseline,
            "mutation-after-external",
            "event-after-external",
        )),
        Err(LivePersistenceError::RecoveryRequired { .. })
    ));
    assert!(writer.is_frozen());
    assert_eq!(writer.checkpoint().revision, baseline);
    assert!(std::fs::read(store.root().join("events.jsonl"))
        .unwrap()
        .ends_with(&jsonl_bytes(&external)));
}

#[test]
fn checkpointed_export_excludes_tails_and_copies_durable_recovery_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    start_session(&mut writer);
    let baseline = writer.checkpoint().revision;
    let session_id = store.read_v2().unwrap().manifest.session_id;
    drop(writer);

    let tail = durable_note_event(&session_id, "mutation-export-tail", "event-export-tail");
    append_jsonl(&store.root().join("events.jsonl"), &tail);
    let before_recovery = temp
        .path()
        .join(format!("snapshot-before{V2_BUNDLE_SUFFIX}"));
    let exported = store.export_v2_checkpointed_to(&before_recovery).unwrap();
    let exported_bundle = exported.read_v2().unwrap();
    assert_eq!(exported_bundle.session_state.revision, baseline);
    assert!(!exported_bundle
        .events
        .iter()
        .any(|event| event.event_id == "event-export-tail"));
    assert!(std::fs::read(store.root().join("events.jsonl"))
        .unwrap()
        .ends_with(&jsonl_bytes(&tail)));

    let torn_path = store.root().join("events.jsonl");
    let checkpoint_len = usize::try_from(
        store.read_v2_checkpointed().unwrap().session_state.streams["events"].committed_bytes,
    )
    .unwrap();
    let mut bytes = std::fs::read(&torn_path).unwrap();
    bytes.truncate(checkpoint_len + (bytes.len() - checkpoint_len) / 2);
    std::fs::write(&torn_path, bytes).unwrap();
    let report = store.recover_v2_with_hooks(hooks).unwrap();
    assert_eq!(report.disposition, RecoveryDispositionV2::RolledBack);
    let artifact = report.artifacts[0].clone();

    let after_recovery = temp
        .path()
        .join(format!("snapshot-after{V2_BUNDLE_SUFFIX}"));
    let exported = store.export_v2_checkpointed_to(&after_recovery).unwrap();
    assert_eq!(
        exported.read_attachment(&artifact.raw_attachment).unwrap(),
        store.read_attachment(&artifact.raw_attachment).unwrap()
    );
    assert!(!after_recovery.join(".antennabench.lock").exists());
    assert!(!after_recovery.join(".session-state.next.json").exists());
    assert!(!after_recovery.join("session-state.previous.json").exists());
}

#[test]
fn checkpointed_export_collision_preserves_the_existing_destination() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let destination = temp
        .path()
        .join(format!("existing-export{V2_BUNDLE_SUFFIX}"));
    let exported = store.export_v2_checkpointed_to(&destination).unwrap();
    let expected = exported.read_v2_checkpointed().unwrap();
    std::fs::write(destination.join("owner.txt"), b"keep existing export").unwrap();

    assert!(store.export_v2_checkpointed_to(&destination).is_err());
    assert_eq!(
        std::fs::read(destination.join("owner.txt")).unwrap(),
        b"keep existing export"
    );
    assert_eq!(
        BundleStore::new(destination)
            .read_v2_checkpointed()
            .unwrap(),
        expected
    );
}

#[test]
fn append_failpoint_matrix_never_exposes_a_mixed_revision() {
    let points = [
        LivePersistencePoint::BeforeStreamWrite(LiveStreamV2::Events),
        LivePersistencePoint::MidStreamWrite(LiveStreamV2::Events),
        LivePersistencePoint::AfterStreamWrite(LiveStreamV2::Events),
        LivePersistencePoint::BeforeStreamSync(LiveStreamV2::Events),
        LivePersistencePoint::AfterStreamSync(LiveStreamV2::Events),
        LivePersistencePoint::BeforeCheckpointWrite,
        LivePersistencePoint::AfterCheckpointWrite,
        LivePersistencePoint::BeforeCheckpointSync,
        LivePersistencePoint::AfterCheckpointSync,
        LivePersistencePoint::BeforeCheckpointReplace,
        LivePersistencePoint::AfterCheckpointReplace,
        LivePersistencePoint::BeforeDirectorySync,
        LivePersistencePoint::AfterDirectorySync,
        LivePersistencePoint::BeforeCheckpointVerify,
        LivePersistencePoint::AfterCheckpointVerify,
        LivePersistencePoint::BeforeAcknowledge,
    ];
    for (index, point) in points.into_iter().enumerate() {
        let temp = tempfile::tempdir().unwrap();
        let store = ready_v2_store(temp.path());
        let hooks = Arc::new(DeterministicHooks::new());
        let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
        start_session(&mut writer);
        let baseline = writer.checkpoint().revision;
        let mutation_id = format!("mutation-failpoint-{index}");
        let event_id = format!("event-failpoint-{index}");
        hooks.fail_once_at(point);
        let result = writer.append(note_mutation(&writer, baseline, &mutation_id, &event_id));
        assert!(result.is_err(), "{point:?} did not inject a failure");
        drop(writer);

        let snapshot = store.read_v2_checkpointed().unwrap();
        assert!(
            snapshot.session_state.revision == baseline
                || snapshot.session_state.revision == baseline + 1,
            "{point:?} exposed unexpected revision {}",
            snapshot.session_state.revision
        );
        let committed = snapshot
            .events
            .iter()
            .any(|event| event.event_id == event_id);
        assert_eq!(committed, snapshot.session_state.revision == baseline + 1);
    }
}

#[test]
fn plan_failpoints_recover_complete_generations_or_preserve_partial_ones() {
    let points = [
        LivePersistencePoint::BeforePlanWrite(LivePlanFile::Station),
        LivePersistencePoint::AfterPlanWrite(LivePlanFile::Station),
        LivePersistencePoint::BeforePlanSync(LivePlanFile::Station),
        LivePersistencePoint::AfterPlanSync(LivePlanFile::Station),
        LivePersistencePoint::BeforePlanWrite(LivePlanFile::Antennas),
        LivePersistencePoint::AfterPlanWrite(LivePlanFile::Antennas),
        LivePersistencePoint::BeforePlanSync(LivePlanFile::Antennas),
        LivePersistencePoint::AfterPlanSync(LivePlanFile::Antennas),
        LivePersistencePoint::BeforePlanWrite(LivePlanFile::Schedule),
        LivePersistencePoint::AfterPlanWrite(LivePlanFile::Schedule),
        LivePersistencePoint::BeforePlanSync(LivePlanFile::Schedule),
        LivePersistencePoint::AfterPlanSync(LivePlanFile::Schedule),
        LivePersistencePoint::BeforePlanWrite(LivePlanFile::GenerationMetadata),
        LivePersistencePoint::AfterPlanWrite(LivePlanFile::GenerationMetadata),
        LivePersistencePoint::BeforePlanSync(LivePlanFile::GenerationMetadata),
        LivePersistencePoint::AfterPlanSync(LivePlanFile::GenerationMetadata),
        LivePersistencePoint::BeforeCheckpointWrite,
        LivePersistencePoint::AfterCheckpointWrite,
        LivePersistencePoint::BeforeCheckpointSync,
        LivePersistencePoint::AfterCheckpointSync,
        LivePersistencePoint::BeforeCheckpointReplace,
        LivePersistencePoint::AfterCheckpointReplace,
        LivePersistencePoint::BeforeDirectorySync,
        LivePersistencePoint::AfterDirectorySync,
        LivePersistencePoint::BeforeCheckpointVerify,
        LivePersistencePoint::AfterCheckpointVerify,
        LivePersistencePoint::BeforeAcknowledge,
    ];
    for (index, point) in points.into_iter().enumerate() {
        let temp = tempfile::tempdir().unwrap();
        let store = ready_v2_store(temp.path());
        let hooks = Arc::new(DeterministicHooks::new());
        let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
        let baseline = store.read_v2().unwrap();
        let generation_id = format!("plan-failpoint-{index}");
        let mut station = baseline.station.clone();
        station.operator_notes = Some(generation_id.clone());
        hooks.fail_once_at(point);
        let result = writer.commit_plan(PlanCommitV2 {
            expected_revision: baseline.session_state.revision,
            generation_id: generation_id.clone(),
            station,
            antennas: baseline.antennas,
            schedule: baseline.schedule,
        });
        assert!(result.is_err(), "{point:?} did not inject a failure");
        drop(writer);

        let report = store.recover_v2_with_hooks(hooks).unwrap();
        let pending_generation = matches!(
            point,
            LivePersistencePoint::AfterPlanWrite(LivePlanFile::GenerationMetadata)
                | LivePersistencePoint::BeforePlanSync(LivePlanFile::GenerationMetadata)
                | LivePersistencePoint::AfterPlanSync(LivePlanFile::GenerationMetadata)
                | LivePersistencePoint::BeforeCheckpointWrite
                | LivePersistencePoint::AfterCheckpointWrite
                | LivePersistencePoint::BeforeCheckpointSync
                | LivePersistencePoint::AfterCheckpointSync
                | LivePersistencePoint::BeforeCheckpointReplace
        );
        let already_checkpointed = matches!(
            point,
            LivePersistencePoint::AfterCheckpointReplace
                | LivePersistencePoint::BeforeDirectorySync
                | LivePersistencePoint::AfterDirectorySync
                | LivePersistencePoint::BeforeCheckpointVerify
                | LivePersistencePoint::AfterCheckpointVerify
                | LivePersistencePoint::BeforeAcknowledge
        );
        assert_eq!(
            report.disposition,
            if pending_generation {
                RecoveryDispositionV2::RolledForward
            } else if already_checkpointed {
                RecoveryDispositionV2::Clean
            } else {
                RecoveryDispositionV2::RolledBack
            },
            "unexpected recovery for {point:?}"
        );
        let reopened = store.read_v2().unwrap();
        if pending_generation || already_checkpointed {
            assert_eq!(
                reopened.session_state.active_plan.generation_id,
                generation_id
            );
        } else {
            assert_ne!(
                reopened.session_state.active_plan.generation_id,
                generation_id
            );
            assert!(!store
                .root()
                .join("plan-generations")
                .join(generation_id)
                .exists());
        }
    }
}

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/session-bundles")
}

fn ready_v2_store(root: &Path) -> BundleStore {
    let source = fixtures_root().join("minimal-whole-station.session.wsprabundle");
    let upgraded_path = root.join(format!("upgraded{V2_BUNDLE_SUFFIX}"));
    let upgraded = BundleStore::new(source)
        .upgrade_v1_to_v2(&upgraded_path)
        .unwrap();
    let mut bundle = upgraded.read_v2().unwrap();
    bundle.events.clear();
    bundle.adapter_records.clear();
    bundle.observations.clear();
    bundle.rig.clear();
    bundle.propagation.clear();
    bundle.session_state.lifecycle = SessionLifecycleV2::Ready;
    bundle.session_state.revision = 1;
    bundle.session_state.last_committed_mutation_id = None;
    BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();
    let destination = root.join(format!("live{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(destination);
    store.write_v2(&bundle).unwrap();
    store
}

fn start_session(writer: &mut LiveSessionV2) {
    let revision = writer.checkpoint().revision;
    writer
        .append(LiveMutationV2 {
            expected_revision: revision,
            mutation_id: "mutation-start".into(),
            members: vec![event_member(
                writer,
                "event-start",
                0,
                OperatorEventPayloadV2::SessionStarted { note: None },
            )],
        })
        .unwrap();
}

fn note_mutation(
    writer: &LiveSessionV2,
    revision: u64,
    mutation_id: &str,
    event_id: &str,
) -> LiveMutationV2 {
    LiveMutationV2 {
        expected_revision: revision,
        mutation_id: mutation_id.into(),
        members: vec![event_member(
            writer,
            event_id,
            0,
            OperatorEventPayloadV2::NoteAdded {
                note: event_id.into(),
            },
        )],
    }
}

fn event_member(
    _writer: &LiveSessionV2,
    event_id: &str,
    member_index: u32,
    payload: OperatorEventPayloadV2,
) -> LiveMutationMemberV2 {
    LiveMutationMemberV2::Event(OperatorEventV2 {
        meta: RecordMetaV2 {
            schema_version: 0,
            session_id: String::new(),
            recorded_at: Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap(),
            provenance: Provenance::from_legacy(RecordSource::Operator, "test"),
            mutation: MutationMember {
                mutation_id: String::new(),
                member_index,
                member_count: 0,
            },
            runtime_context_id: None,
        },
        event_id: event_id.into(),
        occurred_at: Utc.with_ymd_and_hms(2026, 7, 14, 20, 0, 0).unwrap(),
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: None,
        payload,
    })
}

fn durable_note_event(session_id: &str, mutation_id: &str, event_id: &str) -> OperatorEventV2 {
    OperatorEventV2 {
        meta: RecordMetaV2 {
            schema_version: SCHEMA_VERSION_V2,
            session_id: session_id.into(),
            recorded_at: Utc.with_ymd_and_hms(2026, 7, 14, 20, 1, 0).unwrap(),
            provenance: Provenance::from_legacy(RecordSource::Operator, "test"),
            mutation: MutationMember {
                mutation_id: mutation_id.into(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        event_id: event_id.into(),
        occurred_at: Utc.with_ymd_and_hms(2026, 7, 14, 20, 1, 0).unwrap(),
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: None,
        payload: OperatorEventPayloadV2::NoteAdded {
            note: "recovered note".into(),
        },
    }
}

fn append_jsonl<T: serde::Serialize>(path: &Path, value: &T) {
    OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(&jsonl_bytes(value))
        .unwrap();
}

fn jsonl_bytes<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}
