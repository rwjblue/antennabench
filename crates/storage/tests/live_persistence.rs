use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use antennabench_core::{
    EventTimeBasisV2, MutationMember, NormalizedRecordKind, NormalizedRecordLink,
    OperatorEventPayloadV2, OperatorEventV2, Provenance, RecordMetaV2, RecordSource,
    SessionLifecycleV2, V2_BUNDLE_SUFFIX,
};
use antennabench_storage::{
    BundleStore, LiveMutationMemberV2, LiveMutationV2, LivePersistenceError, LivePersistenceHooks,
    LivePersistencePoint, LiveSessionV2, PlanCommitV2,
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
fn writer_lock_append_idempotency_and_plan_freeze_are_coherent() {
    let temp = tempfile::tempdir().unwrap();
    let store = ready_v2_store(temp.path());
    let hooks = Arc::new(DeterministicHooks::new());
    let mut writer = store.open_v2_writer_with_hooks(hooks.clone()).unwrap();
    assert!(matches!(
        store.open_v2_writer(),
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

    let receipt = writer
        .commit_plan(PlanCommitV2 {
            expected_revision: baseline.session_state.revision,
            generation_id: "plan-generation-0002".into(),
            station: station.clone(),
            antennas: baseline.antennas,
            schedule: baseline.schedule,
        })
        .unwrap();
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
        },
        event_id: event_id.into(),
        occurred_at: Utc.with_ymd_and_hms(2026, 7, 14, 20, 0, 0).unwrap(),
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: None,
        payload,
    })
}
