use std::{collections::BTreeMap, io, sync::Arc};

use antennabench_core::{
    reduce_operator_events_v3, AcquisitionChannelId, AdapterId, AnalysisFile, AnalysisStatus,
    Antenna, AntennasFile, Band, BundleFilesV3, BundleManifestV3, BundleV3Contents,
    CorrectableOperatorEventPayloadV3, CounterbalanceBlockIdV3, EventCorrectionActionV3,
    EventTimeBasisV2, ExperimentMode, MutationMember, OperatorEventPayloadV3, OperatorEventV3,
    PlanGenerationV2, PlannedSlotV3, Provenance, ProviderId, RecordMetaV3,
    ReplacementOperatorEventV3, ScheduleV3, SessionGoal, SessionLifecycleV2, SessionStateV3,
    SignalAllocationV3, SignalCadenceV3, SignalCollectionProfileV3, SignalModeV3, SignalPlanIdV3,
    SignalPlanV3, SignalStateConfirmationV3, SignalVariantIdV3, SourceId, Station,
    SCHEMA_VERSION_V3, V2_BUNDLE_SUFFIX,
};
use antennabench_storage::{
    BundleStore, LiveEventMutationV3, LivePersistenceError, LivePersistenceHooks,
    LivePersistencePoint, LiveStreamV2,
};
use chrono::{DateTime, TimeZone, Utc};

#[derive(Debug)]
struct Hooks {
    now: DateTime<Utc>,
    fail_at: Option<LivePersistencePoint>,
}

impl LivePersistenceHooks for Hooks {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }

    fn new_id(&self, kind: &str) -> String {
        format!("{kind}-test")
    }

    fn check(&self, point: LivePersistencePoint) -> io::Result<()> {
        if self.fail_at == Some(point) {
            Err(io::Error::other("injected v3 persistence failure"))
        } else {
            Ok(())
        }
    }
}

fn meta(now: DateTime<Utc>, session_id: &str) -> RecordMetaV3 {
    RecordMetaV3 {
        schema_version: SCHEMA_VERSION_V3,
        session_id: session_id.into(),
        recorded_at: now,
        provenance: Provenance {
            provider_id: ProviderId::new("antennabench").unwrap(),
            source_id: SourceId::new("operator-evidence").unwrap(),
            acquisition_channel: AcquisitionChannelId::new("operator-entry").unwrap(),
            adapter_id: AdapterId::new("antennabench.operator").unwrap(),
            adapter_version: "3".into(),
        },
        mutation: MutationMember {
            mutation_id: "pending".into(),
            member_index: 0,
            member_count: 1,
        },
    }
}

fn bundle() -> BundleV3Contents {
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
    let session_id = "session-v3-live".to_string();
    let plan_id = SignalPlanIdV3::new("manual-cw").unwrap();
    BundleV3Contents {
        manifest: BundleManifestV3 {
            schema_version: SCHEMA_VERSION_V3,
            session_id: session_id.clone(),
            created_at: now,
            app_version: "test".into(),
            files: BundleFilesV3::default(),
        },
        session_state: SessionStateV3 {
            schema_version: SCHEMA_VERSION_V3,
            session_id: session_id.clone(),
            revision: 0,
            lifecycle: SessionLifecycleV2::Ready,
            wspr_live_acquisition_enabled: false,
            active_plan: PlanGenerationV2 {
                generation_id: "generation-1".into(),
                station_sha256: String::new(),
                antennas_sha256: String::new(),
                schedule_sha256: String::new(),
                root_sha256: String::new(),
            },
            streams: BTreeMap::new(),
            last_committed_mutation_id: None,
        },
        station: Station {
            schema_version: SCHEMA_VERSION_V3,
            session_id: session_id.clone(),
            callsign: "N1RWJ".into(),
            grid: "FN42".into(),
            power_watts: Some(10.0),
            operator_notes: None,
        },
        antennas: AntennasFile {
            schema_version: SCHEMA_VERSION_V3,
            session_id: session_id.clone(),
            antennas: vec![Antenna {
                label: "A".into(),
                facets: Vec::new(),
                height_m: None,
                radial_count: None,
                radial_length_m: None,
                orientation_degrees: None,
                tuner: None,
                feedline: None,
                notes: None,
            }],
        },
        schedule: ScheduleV3 {
            schema_version: SCHEMA_VERSION_V3,
            session_id: session_id.clone(),
            mode: ExperimentMode::TxFocused,
            goal: SessionGoal::GeneralCoverage,
            signal_plans: vec![SignalPlanV3 {
                signal_plan_id: plan_id.clone(),
                mode: SignalModeV3::Cw,
                planned_power_watts: Some(10.0),
                transmitted_callsign: "N1RWJ".into(),
                differing_identity_validated: false,
                cadence: SignalCadenceV3 {
                    message: "CQ CQ N1RWJ N1RWJ TEST".into(),
                    repetition_count: 2,
                    key_speed_wpm: Some(20),
                    transmit_seconds: 20,
                    interval_seconds: 30,
                },
                collection_profile: SignalCollectionProfileV3::ManualObservation,
            }],
            slots: vec![PlannedSlotV3 {
                slot_id: "slot-1".into(),
                sequence_number: 1,
                starts_at: now,
                duration_seconds: 20,
                guard_seconds: 5,
                band: Band::M20,
                antenna_label: "A".into(),
                signal: Some(SignalAllocationV3 {
                    signal_plan_id: plan_id,
                    frequency_hz: 14_050_000,
                    frequency_variant_id: SignalVariantIdV3::new("fixed").unwrap(),
                    counterbalance_block_id: CounterbalanceBlockIdV3::new("block-1").unwrap(),
                    counterbalance_position: 0,
                }),
            }],
        },
        events: Vec::new(),
        observations: Vec::new(),
        adapter_records: Vec::new(),
        rig: Vec::new(),
        propagation: Vec::new(),
        analysis: AnalysisFile {
            schema_version: SCHEMA_VERSION_V3,
            session_id,
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: Vec::new(),
        },
    }
}

fn event(
    now: DateTime<Utc>,
    event_id: &str,
    slot_id: Option<&str>,
    payload: OperatorEventPayloadV3,
) -> OperatorEventV3 {
    OperatorEventV3 {
        meta: meta(now, "session-v3-live"),
        event_id: event_id.into(),
        occurred_at: now,
        time_basis: EventTimeBasisV2::OperatorReported,
        uncertainty_seconds: None,
        slot_id: slot_id.map(str::to_string),
        payload,
    }
}

fn mutation(revision: u64, mutation_id: &str, event: OperatorEventV3) -> LiveEventMutationV3 {
    LiveEventMutationV3 {
        expected_revision: revision,
        mutation_id: mutation_id.into(),
        event,
    }
}

#[test]
fn checkpointed_v3_writer_persists_idempotent_correctable_signal_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(format!("live{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let mut initial = bundle();
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();

    let now = initial.manifest.created_at;
    let hooks = Arc::new(Hooks { now, fail_at: None });
    let mut writer = store.open_v3_writer_with_hooks(hooks).unwrap();
    assert!(matches!(
        store.open_v3_writer(),
        Err(LivePersistenceError::WriterBusy)
    ));

    writer
        .append_event(mutation(
            0,
            "mutation-start",
            event(
                now,
                "event-start",
                None,
                OperatorEventPayloadV3::SessionStarted { note: None },
            ),
        ))
        .unwrap();
    let confirmation = event(
        now,
        "event-signal",
        Some("slot-1"),
        OperatorEventPayloadV3::SignalStateConfirmed {
            confirmation: SignalStateConfirmationV3 {
                frequency_hz: Some(14_050_100),
                mode: Some(SignalModeV3::Cw),
                power_watts: Some(10.0),
                transmitted_callsign: Some("N1RWJ".into()),
                cadence_followed: Some(true),
                note: Some("initial entry".into()),
            },
        },
    );
    writer
        .append_event(mutation(1, "mutation-signal", confirmation.clone()))
        .unwrap();
    let retry = writer
        .append_event(mutation(1, "mutation-signal", confirmation))
        .unwrap();
    assert!(retry.idempotent);
    assert_eq!(retry.revision, 2);

    let correction = event(
        now,
        "event-correction",
        None,
        OperatorEventPayloadV3::EventCorrected {
            target_event_id: "event-signal".into(),
            correction: EventCorrectionActionV3::Replace {
                replacement: ReplacementOperatorEventV3 {
                    occurred_at: now,
                    time_basis: EventTimeBasisV2::OperatorReported,
                    uncertainty_seconds: None,
                    slot_id: Some("slot-1".into()),
                    payload: CorrectableOperatorEventPayloadV3::SignalStateConfirmed {
                        confirmation: SignalStateConfirmationV3 {
                            frequency_hz: Some(14_050_000),
                            mode: Some(SignalModeV3::Cw),
                            power_watts: Some(10.0),
                            transmitted_callsign: Some("N1RWJ".into()),
                            cadence_followed: Some(true),
                            note: Some("corrected from log".into()),
                        },
                    },
                },
            },
            reason: "frequency was entered incorrectly".into(),
        },
    );
    writer
        .append_event(mutation(2, "mutation-correction", correction))
        .unwrap();
    assert!(matches!(
        writer.append_event(mutation(
            2,
            "mutation-stale",
            event(
                now,
                "event-note",
                None,
                OperatorEventPayloadV3::NoteAdded {
                    note: "late".into()
                },
            ),
        )),
        Err(LivePersistenceError::StaleRevision {
            expected: 2,
            actual: 3
        })
    ));
    drop(writer);

    let reopened = store.read_v3_checkpointed().unwrap();
    assert_eq!(reopened.session_state.revision, 3);
    assert_eq!(
        reopened.session_state.lifecycle,
        SessionLifecycleV2::Running
    );
    assert_eq!(reopened.events.len(), 3);
    let reduction = reduce_operator_events_v3(SessionLifecycleV2::Ready, &reopened.events);
    assert!(reduction.diagnostics.is_empty());
    assert!(reduction.effective_events.iter().any(|effective| matches!(
        &effective.payload,
        CorrectableOperatorEventPayloadV3::SignalStateConfirmed { confirmation }
            if confirmation.frequency_hz == Some(14_050_000)
    )));
}

#[test]
fn v3_checkpoint_failure_rolls_back_the_uncommitted_event() {
    let temp = tempfile::tempdir().unwrap();
    for (index, fail_at) in [
        LivePersistencePoint::MidStreamWrite(LiveStreamV2::Events),
        LivePersistencePoint::BeforeCheckpointWrite,
    ]
    .into_iter()
    .enumerate()
    {
        let path = temp
            .path()
            .join(format!("rollback-{index}{V2_BUNDLE_SUFFIX}"));
        let store = BundleStore::new(&path);
        let mut initial = bundle();
        BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
        store.create_v3_checkpointed(&initial).unwrap();
        let hooks = Arc::new(Hooks {
            now: initial.manifest.created_at,
            fail_at: Some(fail_at),
        });
        let mut writer = store.open_v3_writer_with_hooks(hooks).unwrap();

        assert!(matches!(
            writer.append_event(mutation(
                0,
                "mutation-start",
                event(
                    initial.manifest.created_at,
                    "event-start",
                    None,
                    OperatorEventPayloadV3::SessionStarted { note: None },
                ),
            )),
            Err(LivePersistenceError::Io { .. })
        ));
        drop(writer);

        assert_eq!(store.read_v3_checkpointed().unwrap(), initial);
    }
}

#[test]
fn checkpointed_v3_export_reopens_as_the_exact_revision() {
    let temp = tempfile::tempdir().unwrap();
    let source = BundleStore::new(temp.path().join(format!("source{V2_BUNDLE_SUFFIX}")));
    let mut initial = bundle();
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    source.create_v3_checkpointed(&initial).unwrap();
    {
        let mut writer = source.open_v3_writer().unwrap();
        writer
            .append_event(mutation(
                0,
                "mutation-start",
                event(
                    initial.manifest.created_at,
                    "event-start",
                    None,
                    OperatorEventPayloadV3::SessionStarted { note: None },
                ),
            ))
            .unwrap();
    }

    let destination = temp.path().join(format!("export{V2_BUNDLE_SUFFIX}"));
    let exported = source.export_v3_checkpointed_to(&destination).unwrap();

    assert_eq!(
        exported.read_v3_checkpointed().unwrap(),
        source.read_v3_checkpointed().unwrap()
    );
}
