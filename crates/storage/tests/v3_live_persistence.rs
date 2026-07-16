use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{self, Write},
    sync::Arc,
};

use antennabench_core::{
    reduce_operator_events_v3, AcquisitionChannelId, AdapterDisposition, AdapterId, AdapterInput,
    AdapterReasonId, AdapterRecordV3, AnalysisFile, AnalysisStatus, Antenna, AntennasFile,
    AttachmentReference, Band, BundleFilesV3, BundleManifestV3, BundleV3Contents,
    CorrectableOperatorEventPayloadV3, CounterbalanceBlockIdV3, EventCorrectionActionV3,
    EventTimeBasisV2, ExperimentMode, MutationMember, NormalizedRecordKind, NormalizedRecordLink,
    ObservationKind, ObservationRecordV3, OperatorEventPayloadV3, OperatorEventV3,
    PlanGenerationV2, PlannedSlotV3, Provenance, ProviderId, RecordMetaV2, RecordMetaV3,
    ReplacementOperatorEventV3, ScheduleV3, SessionGoal, SessionLifecycleV2, SessionStateV3,
    SignalAllocationV3, SignalCadenceV3, SignalCollectionProfileV3, SignalModeV3, SignalPlanIdV3,
    SignalPlanV3, SignalStateConfirmationV3, SignalVariantIdV3, SourceId, Station,
    SCHEMA_VERSION_V3, V2_BUNDLE_SUFFIX,
};
use antennabench_storage::{
    BundleStore, LiveEventMutationV3, LiveEvidenceMutationV3, LivePersistenceError,
    LivePersistenceHooks, LivePersistencePoint, LiveStreamV2, RecoveryDispositionV2,
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

fn evidence(
    revision: u64,
    mutation_id: &str,
    now: DateTime<Utc>,
    attachment: AttachmentReference,
) -> LiveEvidenceMutationV3 {
    let provenance = Provenance {
        provider_id: ProviderId::new("reverse-beacon-network").unwrap(),
        source_id: SourceId::new("rbn-daily-archive").unwrap(),
        acquisition_channel: AcquisitionChannelId::new("file-import").unwrap(),
        adapter_id: AdapterId::new("antennabench.rbn-daily-archive").unwrap(),
        adapter_version: "test".into(),
    };
    let record_meta = RecordMetaV2 {
        schema_version: SCHEMA_VERSION_V3,
        session_id: "pending".into(),
        recorded_at: now,
        provenance: provenance.clone(),
        mutation: MutationMember {
            mutation_id: "pending".into(),
            member_index: 0,
            member_count: 1,
        },
    };
    let capture = AdapterRecordV3 {
        meta: record_meta.clone(),
        record_id: format!("capture-{mutation_id}"),
        source_time: Some(now),
        record_type: "rbn_archive_capture".into(),
        disposition: AdapterDisposition::Accepted,
        reason: AdapterReasonId::new("rbn.capture").unwrap(),
        normalized_records: Vec::new(),
        input: AdapterInput::Attachment { attachment },
    };
    let row_id = format!("row-{mutation_id}");
    let observation_id = format!("observation-{mutation_id}");
    let row = AdapterRecordV3 {
        meta: record_meta.clone(),
        record_id: row_id.clone(),
        source_time: Some(now),
        record_type: "rbn_archive_row".into(),
        disposition: AdapterDisposition::Accepted,
        reason: AdapterReasonId::new("rbn.accepted").unwrap(),
        normalized_records: vec![NormalizedRecordLink {
            record_kind: NormalizedRecordKind::Observation,
            record_id: observation_id.clone(),
        }],
        input: AdapterInput::Inline {
            data: "[\"synthetic\"]".into(),
            media_type: "application/json".into(),
            encoding: None,
            source_locator: Some("synthetic.zip".into()),
        },
    };
    let observation = ObservationRecordV3 {
        meta: record_meta,
        observation_id,
        adapter_record_ids: vec![row_id],
        observation_kind: ObservationKind::PublicReport,
        band: Band::M20,
        frequency_hz: Some(14_050_000),
        mode: Some("CW".into()),
        reporter_call: Some("K1ABC-1".into()),
        heard_call: Some("N1RWJ".into()),
        reporter_grid: None,
        heard_grid: None,
        distance_km: None,
        azimuth_degrees: None,
        snr_db: Some(18.0),
        drift_hz_per_minute: None,
        power_watts: None,
        slot_id: None,
        slot_label: None,
        slot_confidence: None,
        raw: serde_json::json!({"synthetic": true}),
    };
    LiveEvidenceMutationV3 {
        expected_revision: revision,
        mutation_id: mutation_id.into(),
        adapter_records: vec![capture, row],
        observations: vec![observation],
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
fn v3_recovery_preserves_and_rolls_back_an_uncommitted_torn_event() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(format!("recovery{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let mut initial = bundle();
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();
    {
        let mut writer = store.open_v3_writer().unwrap();
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

    OpenOptions::new()
        .append(true)
        .open(path.join("events.jsonl"))
        .unwrap()
        .write_all(b"{\"torn\":true")
        .unwrap();
    assert!(store.read_v3_checkpointed().is_err());

    let hooks = Arc::new(Hooks {
        now: initial.manifest.created_at,
        fail_at: None,
    });
    let report = store.recover_v3_with_hooks(hooks).unwrap();
    assert_eq!(report.starting_revision, 1);
    assert_eq!(report.recovered_revision, 1);
    assert_eq!(report.final_revision, 2);
    assert_eq!(report.disposition, RecoveryDispositionV2::RolledBack);
    assert_eq!(report.artifacts.len(), 1);
    assert!(report.interruption.is_some());
    assert_eq!(
        store
            .read_attachment(&report.artifacts[0].raw_attachment)
            .unwrap(),
        b"{\"torn\":true"
    );

    let recovered = store.read_v3_checkpointed().unwrap();
    assert_eq!(recovered.session_state.revision, 2);
    assert_eq!(
        recovered.session_state.lifecycle,
        SessionLifecycleV2::Interrupted
    );
    assert_eq!(recovered.events.len(), 2);
    assert!(matches!(
        recovered.events[1].payload,
        OperatorEventPayloadV3::InterruptionDetected { .. }
    ));
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

#[test]
fn v3_evidence_and_exact_attachment_commit_atomically_and_export_losslessly() {
    let temp = tempfile::tempdir().unwrap();
    let source = BundleStore::new(temp.path().join(format!("evidence{V2_BUNDLE_SUFFIX}")));
    let mut initial = bundle();
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    source.create_v3_checkpointed(&initial).unwrap();
    let archive = b"synthetic exact RBN ZIP bytes";

    let attachment;
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
        let (stored, receipt) = writer
            .append_evidence_with_attachment(
                archive,
                "application/zip",
                None,
                Some("zip-single-csv".into()),
                Some("synthetic.zip".into()),
                |attachment| evidence(1, "rbn-import-one", initial.manifest.created_at, attachment),
            )
            .unwrap();
        assert_eq!(receipt.revision, 2);
        assert!(!receipt.idempotent);
        assert_eq!(writer.snapshot().adapter_records.len(), 2);
        assert_eq!(writer.snapshot().observations.len(), 1);
        attachment = stored;

        let replay = writer
            .append_evidence(evidence(
                2,
                "rbn-import-one",
                initial.manifest.created_at,
                attachment.clone(),
            ))
            .unwrap();
        assert!(replay.idempotent);
        assert_eq!(replay.revision, 2);

        let mut conflict = evidence(
            2,
            "rbn-import-one",
            initial.manifest.created_at,
            attachment.clone(),
        );
        conflict.observations[0].snr_db = Some(17.0);
        assert!(matches!(
            writer.append_evidence(conflict),
            Err(LivePersistenceError::MutationConflict { .. })
        ));
    }

    let reopened = source.read_v3_checkpointed().unwrap();
    assert_eq!(reopened.session_state.revision, 2);
    assert_eq!(reopened.adapter_records.len(), 2);
    assert_eq!(reopened.observations.len(), 1);
    assert_eq!(source.read_attachment(&attachment).unwrap(), archive);

    let destination = temp
        .path()
        .join(format!("evidence-export{V2_BUNDLE_SUFFIX}"));
    let exported = source.export_v3_checkpointed_to(&destination).unwrap();
    assert_eq!(exported.read_v3_checkpointed().unwrap(), reopened);
    assert_eq!(exported.read_attachment(&attachment).unwrap(), archive);
}

#[test]
fn v3_evidence_failure_rolls_back_both_streams_and_new_attachment() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp
        .path()
        .join(format!("evidence-rollback{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let mut initial = bundle();
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();
    {
        let mut writer = store.open_v3_writer().unwrap();
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
    let baseline = store.read_v3_checkpointed().unwrap();
    let hooks = Arc::new(Hooks {
        now: initial.manifest.created_at,
        fail_at: Some(LivePersistencePoint::MidStreamWrite(
            LiveStreamV2::Observations,
        )),
    });
    let mut writer = store.open_v3_writer_with_hooks(hooks).unwrap();
    assert!(matches!(
        writer.append_evidence_with_attachment(
            b"uncommitted RBN ZIP",
            "application/zip",
            None,
            Some("zip-single-csv".into()),
            Some("synthetic.zip".into()),
            |attachment| evidence(
                1,
                "rbn-import-failed",
                initial.manifest.created_at,
                attachment,
            ),
        ),
        Err(LivePersistenceError::Io { .. })
    ));
    drop(writer);

    assert_eq!(store.read_v3_checkpointed().unwrap(), baseline);
    let attachment_entries = std::fs::read_dir(path.join("attachments/sha256"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(attachment_entries, 0);
}
