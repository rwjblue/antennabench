use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{self, Write},
    sync::Arc,
};

use antennabench_core::{
    v2::{
        AcquisitionChannelId, AdapterDisposition, AdapterId, AdapterInput, AdapterReasonId,
        AttachmentReference, EventTimeBasisV2, MutationMember, NormalizedRecordKind,
        NormalizedRecordLink, PlanGenerationV2, Provenance, ProviderId, RecordMetaV2,
        SessionLifecycleV2, SourceId, V2_BUNDLE_SUFFIX,
    },
    v3::{
        reduce_operator_events_v3, AdapterRecordV3, BundleFilesV3, BundleManifestV3,
        BundleV3Contents, CorrectableOperatorEventPayloadV3, CounterbalanceBlockIdV3,
        EventCorrectionActionV3, ObservationRecordV3, OperatorEventPayloadV3, OperatorEventV3,
        PlannedSlotV3, RecordMetaV3, ReplacementOperatorEventV3, RigRecordV3, ScheduleV3,
        SessionStateV3, SignalAllocationV3, SignalCadenceV3, SignalCollectionProfileV3,
        SignalModeV3, SignalPlanIdV3, SignalPlanV3, SignalStateConfirmationV3, SignalVariantIdV3,
        WsprCycleDirection, WsprCycleIntentV3,
    },
    v5::{
        upgrade_v3_bundle_model_to_v5, AntennaControlCommandV5, AntennaControlContextV5,
        AntennaControlDispositionV5, AntennaControlInvocationPolicyV5, AntennaControlInvocationV5,
        AntennaControlOutputEncodingV5, AntennaControlOutputV5, AntennaControlPolicyV5,
        AntennaControlRoleV5, WsprReadinessBasisV5, COMMAND_OUTPUT_MAX_BYTES,
    },
    v6::{
        upgrade_v5_bundle_model_to_v6, BuildChannelV6, BuildIdentityV6, DiagnosticCauseV6,
        DiagnosticDetailStateV6, DiagnosticDetailStatusV6, DiagnosticOperationV6,
        DiagnosticOutcomeV6, DiagnosticPhaseV6, DiagnosticRetryV6, DiagnosticSeverityV6,
        DiagnosticTargetV6, EvidenceEffectV6, OperationalDiagnosticV6, RetryDispositionV6,
        RuntimeContextV6, RuntimePlatformV6, SourceStateV6, OPERATIONAL_DIAGNOSTIC_SCHEMA_V1,
    },
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, ExperimentMode, ObservationKind,
    SessionGoal, Station, SCHEMA_VERSION_V3, SCHEMA_VERSION_V5,
};
use antennabench_storage::{
    BundleStore, DiagnosticPersistenceStatusV6, LiveAntennaControlMutationV5,
    LiveDiagnosticMutationV6, LiveEventMutationV3, LiveEvidenceMutationV3, LivePersistenceError,
    LivePersistenceHooks, LivePersistencePoint, LiveStreamV2, RecoveryDispositionV2,
};
use chrono::{DateTime, TimeZone, Utc};

fn runtime_context(now: DateTime<Utc>, os_version: &str) -> RuntimeContextV6 {
    RuntimeContextV6::new(
        now,
        MutationMember {
            mutation_id: "pending-context".into(),
            member_index: 0,
            member_count: 1,
        },
        BuildIdentityV6 {
            app_version: Some("0.1.0-dev".into()),
            source_commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
            source_state: SourceStateV6::Clean,
            build_channel: BuildChannelV6::Development,
            release_tag: None,
            target_triple: Some("x86_64-unknown-linux-gnu".into()),
            build_architecture: Some("x86_64".into()),
            build_timestamp: None,
        },
        RuntimePlatformV6 {
            os_family: Some("linux".into()),
            os_version: Some(os_version.into()),
            runtime_architecture: Some("x86_64".into()),
            application_id: Some("com.rwjblue.antennabench".into()),
        },
    )
}

fn diagnostic(attempt_id: &str, revision: u64, now: DateTime<Utc>) -> OperationalDiagnosticV6 {
    OperationalDiagnosticV6 {
        schema: OPERATIONAL_DIAGNOSTIC_SCHEMA_V1.into(),
        diagnostic_id: format!("diagnostic-{attempt_id}"),
        correlation_id: "test-operation".into(),
        attempt_id: attempt_id.into(),
        mutation: MutationMember {
            mutation_id: "pending".into(),
            member_index: 0,
            member_count: 1,
        },
        runtime_context_id: String::new(),
        occurred_at: now,
        operation: DiagnosticOperationV6::SessionMutation,
        phase: DiagnosticPhaseV6::Checkpoint,
        code: "session.persistence_io".into(),
        summary: "The test mutation did not commit.".into(),
        outcome: DiagnosticOutcomeV6::Failed,
        severity: DiagnosticSeverityV6::Error,
        revision_before: Some(revision),
        revision_after: Some(revision),
        diagnostic_revision: revision,
        evidence_effect: EvidenceEffectV6::NoneCommitted,
        retry: DiagnosticRetryV6 {
            disposition: RetryDispositionV6::Retryable,
            guidance_code: "retry_when_storage_is_available".into(),
        },
        targets: vec![DiagnosticTargetV6::Mutation {
            id: "test-primary-mutation".into(),
        }],
        causes: vec![DiagnosticCauseV6 {
            code: "session.persistence_io".into(),
            phase: DiagnosticPhaseV6::Checkpoint,
            facts: Vec::new(),
        }],
        detail_status: DiagnosticDetailStatusV6 {
            state: DiagnosticDetailStateV6::Complete,
            omitted_fact_count: 0,
        },
    }
}

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
        runtime_context_id: None,
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
        runtime_context_id: None,
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
            creator_runtime_context_id: None,
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
            active_runtime_context_id: None,
            diagnostics_status: None,
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
            antenna_control: None,
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
            wspr_cycle_intents: Vec::new(),
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
        runtime_contexts: Vec::new(),
        diagnostics: Vec::new(),
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

fn command_v5_bundle() -> BundleV3Contents {
    let mut bundle = upgrade_v3_bundle_model_to_v5(bundle());
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
    bundle.schedule.signal_plans.clear();
    bundle.schedule.slots.clear();
    bundle.schedule.wspr_cycle_intents = vec![WsprCycleIntentV3 {
        intent_id: "intent-1".into(),
        sequence_number: 1,
        band: Band::M20,
        antenna_label: "A".into(),
        direction: Some(WsprCycleDirection::Transmit),
        signal: None,
    }];
    bundle.schedule.antenna_control = Some(AntennaControlPolicyV5::CommandControlled {
        invocation: AntennaControlInvocationPolicyV5::OperatorTriggered,
        manual_review_required: false,
    });
    bundle.events = vec![OperatorEventV3 {
        meta: RecordMetaV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: bundle.manifest.session_id.clone(),
            recorded_at: now,
            provenance: meta(now, &bundle.manifest.session_id).provenance,
            mutation: MutationMember {
                mutation_id: "start-mutation".into(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        event_id: "start-event".into(),
        occurred_at: now,
        time_basis: EventTimeBasisV2::OperatorReported,
        uncertainty_seconds: None,
        slot_id: None,
        payload: OperatorEventPayloadV3::SessionStarted { note: None },
    }];
    bundle.session_state.lifecycle = SessionLifecycleV2::Running;
    bundle
}

fn invocation_record(
    record_id: &str,
    role: AntennaControlRoleV5,
    started_at: DateTime<Utc>,
    disposition: AntennaControlDispositionV5,
) -> RigRecordV3 {
    RigRecordV3 {
        meta: meta(started_at, "session-v3-live"),
        record_id: record_id.into(),
        adapter_record_ids: Vec::new(),
        status: "antenna_control_attempt".into(),
        frequency_hz: None,
        mode: None,
        power_watts: None,
        antenna_control: Some(AntennaControlInvocationV5 {
            role,
            controller_profile_name: "bench-switch".into(),
            controller_profile_revision: "revision-7".into(),
            command: AntennaControlCommandV5 {
                program_template: "/opt/bin/switch".into(),
                argument_templates: vec![
                    "--target".into(),
                    "{target}".into(),
                    "--mode".into(),
                    "{mode}".into(),
                ],
                resolved_program: "/opt/bin/switch".into(),
                resolved_arguments: vec![
                    "--target".into(),
                    "relay-a".into(),
                    "--mode".into(),
                    "tx_focused".into(),
                ],
            },
            context: AntennaControlContextV5 {
                antenna: "A".into(),
                target: "relay-a".into(),
                mode: ExperimentMode::TxFocused,
                direction: WsprCycleDirection::Transmit,
                band: Band::M20,
                frequency_hz: None,
                sequence: 1,
                intent_id: "intent-1".into(),
                session_id: "session-v3-live".into(),
                callsign: "N1RWJ".into(),
            },
            started_at,
            completed_at: started_at + chrono::Duration::milliseconds(25),
            elapsed_milliseconds: 25,
            disposition,
            stdout: AntennaControlOutputV5 {
                encoding: AntennaControlOutputEncodingV5::Utf8,
                data: "ok".into(),
                truncated: false,
            },
            stderr: AntennaControlOutputV5 {
                encoding: AntennaControlOutputEncodingV5::Base64,
                data: "AAE=".into(),
                truncated: false,
            },
        }),
        raw: serde_json::Value::Null,
    }
}

fn verified_mutation(revision: u64, now: DateTime<Utc>) -> LiveAntennaControlMutationV5 {
    LiveAntennaControlMutationV5 {
        expected_revision: revision,
        mutation_id: "control-mutation".into(),
        rig_records: vec![
            invocation_record(
                "switch-record",
                AntennaControlRoleV5::Switch,
                now,
                AntennaControlDispositionV5::Exit { code: 0 },
            ),
            invocation_record(
                "verify-record",
                AntennaControlRoleV5::Verification,
                now + chrono::Duration::milliseconds(25),
                AntennaControlDispositionV5::Exit { code: 0 },
            ),
        ],
        armed_event: Some(event(
            now + chrono::Duration::milliseconds(50),
            "armed-event",
            Some("intent-1"),
            OperatorEventPayloadV3::WsprCycleArmed {
                antenna_label: "A".into(),
                cycle_starts_at: now + chrono::Duration::minutes(3) + chrono::Duration::seconds(1),
                readiness: Some(WsprReadinessBasisV5::CommandVerified {
                    switch_record_id: "switch-record".into(),
                    verification_record_id: "verify-record".into(),
                }),
            },
        )),
    }
}

#[test]
fn schema_v5_atomically_commits_command_verified_readiness_and_retries_idempotently() {
    let temp = tempfile::tempdir().unwrap();
    let store = BundleStore::new(temp.path().join(format!("control{V2_BUNDLE_SUFFIX}")));
    let mut initial = command_v5_bundle();
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();
    let portable_plan = std::fs::read_to_string(store.root().join("schedule.json")).unwrap();
    assert!(portable_plan.contains("antenna_control"));
    for local_only in ["program_template", "target", "timeout"] {
        assert!(!portable_plan.contains(local_only));
    }
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 1, 0).unwrap();
    let hooks = Arc::new(Hooks {
        now: now + chrono::Duration::milliseconds(50),
        fail_at: None,
    });
    let mut writer = store.open_v3_writer_with_hooks(hooks).unwrap();
    let mutation = verified_mutation(writer.checkpoint().revision, now);
    let receipt = writer.append_antenna_control(mutation.clone()).unwrap();
    assert!(!receipt.idempotent);
    let retry = writer.append_antenna_control(mutation).unwrap();
    assert!(retry.idempotent);
    drop(writer);

    let reopened = store.read_v3_checkpointed().unwrap();
    assert_eq!(reopened.rig.len(), 2);
    assert_eq!(reopened.events.len(), 2);
    assert!(matches!(
        reopened.events[1].payload,
        OperatorEventPayloadV3::WsprCycleArmed {
            readiness: Some(WsprReadinessBasisV5::CommandVerified { .. }),
            ..
        }
    ));
    let exported = store
        .export_v3_checkpointed_to(
            temp.path()
                .join(format!("control-export{V2_BUNDLE_SUFFIX}")),
        )
        .unwrap();
    assert_eq!(exported.read_v3_checkpointed().unwrap(), reopened);
}

#[test]
fn schema_v5_failed_attempt_commits_without_arming_or_occupancy() {
    let temp = tempfile::tempdir().unwrap();
    let store = BundleStore::new(temp.path().join(format!("failed{V2_BUNDLE_SUFFIX}")));
    let mut initial = command_v5_bundle();
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 1, 0).unwrap();
    let hooks = Arc::new(Hooks {
        now: now + chrono::Duration::seconds(1),
        fail_at: None,
    });
    let mut writer = store.open_v3_writer_with_hooks(hooks).unwrap();
    writer
        .append_antenna_control(LiveAntennaControlMutationV5 {
            expected_revision: writer.checkpoint().revision,
            mutation_id: "failed-control".into(),
            rig_records: vec![invocation_record(
                "timed-out-switch",
                AntennaControlRoleV5::Switch,
                now,
                AntennaControlDispositionV5::Timeout,
            )],
            armed_event: None,
        })
        .unwrap();
    drop(writer);

    let reopened = store.read_v3_checkpointed().unwrap();
    assert_eq!(reopened.rig.len(), 1);
    assert_eq!(reopened.events.len(), 1);
    assert!(
        reduce_operator_events_v3(SessionLifecycleV2::Ready, &reopened.events)
            .effective_events
            .is_empty()
    );
    assert!(
        antennabench_core::v3::project_wspr_run_v3(&reopened.schedule, &reopened.events)
            .cycles
            .is_empty()
    );
}

#[test]
fn schema_v5_persists_every_command_termination_disposition_and_output_form() {
    let temp = tempfile::tempdir().unwrap();
    let store = BundleStore::new(temp.path().join(format!("dispositions{V2_BUNDLE_SUFFIX}")));
    let mut initial = command_v5_bundle();
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 1, 0).unwrap();
    let hooks = Arc::new(Hooks {
        now: now + chrono::Duration::seconds(1),
        fail_at: None,
    });
    let mut writer = store.open_v3_writer_with_hooks(hooks).unwrap();
    let dispositions = [
        AntennaControlDispositionV5::Exit { code: 7 },
        AntennaControlDispositionV5::SpawnError {
            message: "not found".into(),
        },
        AntennaControlDispositionV5::Signaled { signal: Some(15) },
        AntennaControlDispositionV5::Timeout,
    ];
    for (index, disposition) in dispositions.iter().cloned().enumerate() {
        let mut record = invocation_record(
            &format!("disposition-{index}"),
            AntennaControlRoleV5::Switch,
            now,
            disposition,
        );
        record.antenna_control.as_mut().unwrap().stdout.truncated = index == 3;
        writer
            .append_antenna_control(LiveAntennaControlMutationV5 {
                expected_revision: writer.checkpoint().revision,
                mutation_id: format!("disposition-mutation-{index}"),
                rig_records: vec![record],
                armed_event: None,
            })
            .unwrap();
    }
    drop(writer);
    let reopened = store.read_v3_checkpointed().unwrap();
    let actual = reopened
        .rig
        .iter()
        .map(|record| record.antenna_control.as_ref().unwrap().disposition.clone())
        .collect::<Vec<_>>();
    assert_eq!(actual, dispositions);
    assert_eq!(
        reopened.rig[0]
            .antenna_control
            .as_ref()
            .unwrap()
            .stderr
            .encoding,
        AntennaControlOutputEncodingV5::Base64
    );
    assert!(
        reopened.rig[3]
            .antenna_control
            .as_ref()
            .unwrap()
            .stdout
            .truncated
    );
}

#[test]
fn schema_v5_rejects_reference_and_output_failures_before_writing() {
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 1, 0).unwrap();
    for case in [
        "missing",
        "mismatch",
        "mode",
        "cross-intention",
        "role",
        "future",
        "nonzero",
        "oversized",
        "duplicate",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let store = BundleStore::new(temp.path().join(format!("{case}{V2_BUNDLE_SUFFIX}")));
        let mut initial = command_v5_bundle();
        BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
        store.create_v3_checkpointed(&initial).unwrap();
        let hooks = Arc::new(Hooks {
            now: now + chrono::Duration::milliseconds(50),
            fail_at: None,
        });
        let mut writer = store.open_v3_writer_with_hooks(hooks).unwrap();
        let mut mutation = verified_mutation(writer.checkpoint().revision, now);
        match case {
            "missing" => {
                let event = mutation.armed_event.as_mut().unwrap();
                let OperatorEventPayloadV3::WsprCycleArmed { readiness, .. } = &mut event.payload
                else {
                    unreachable!()
                };
                *readiness = Some(WsprReadinessBasisV5::CommandVerified {
                    switch_record_id: "missing-record".into(),
                    verification_record_id: "verify-record".into(),
                });
            }
            "mismatch" => {
                mutation.rig_records[1]
                    .antenna_control
                    .as_mut()
                    .unwrap()
                    .context
                    .target = "relay-b".into()
            }
            "mode" => {
                mutation.rig_records[1]
                    .antenna_control
                    .as_mut()
                    .unwrap()
                    .context
                    .mode = ExperimentMode::RxFocused
            }
            "cross-intention" => {
                mutation.rig_records[1]
                    .antenna_control
                    .as_mut()
                    .unwrap()
                    .context
                    .intent_id = "other-intent".into()
            }
            "role" => {
                mutation.rig_records[1]
                    .antenna_control
                    .as_mut()
                    .unwrap()
                    .role = AntennaControlRoleV5::Switch
            }
            "future" => {
                let invocation = mutation.rig_records[1].antenna_control.as_mut().unwrap();
                invocation.completed_at = now + chrono::Duration::milliseconds(75);
                invocation.elapsed_milliseconds = 50;
            }
            "nonzero" => {
                mutation.rig_records[0]
                    .antenna_control
                    .as_mut()
                    .unwrap()
                    .disposition = AntennaControlDispositionV5::Exit { code: 1 }
            }
            "oversized" => {
                mutation.rig_records[0]
                    .antenna_control
                    .as_mut()
                    .unwrap()
                    .stdout
                    .data = "x".repeat(COMMAND_OUTPUT_MAX_BYTES + 1)
            }
            "duplicate" => {
                let event = mutation.armed_event.as_mut().unwrap();
                let OperatorEventPayloadV3::WsprCycleArmed { readiness, .. } = &mut event.payload
                else {
                    unreachable!()
                };
                *readiness = Some(WsprReadinessBasisV5::CommandVerified {
                    switch_record_id: "switch-record".into(),
                    verification_record_id: "switch-record".into(),
                });
            }
            _ => unreachable!(),
        }
        assert!(
            writer.append_antenna_control(mutation).is_err(),
            "case {case}"
        );
        drop(writer);
        let reopened = store.read_v3_checkpointed().unwrap();
        assert!(reopened.rig.is_empty(), "case {case}");
        assert_eq!(reopened.events.len(), 1, "case {case}");
    }
}

#[test]
fn schema_v5_control_checkpoint_failures_expose_only_prior_or_complete_revision() {
    let points = [
        LivePersistencePoint::BeforeStreamWrite(LiveStreamV2::Rig),
        LivePersistencePoint::MidStreamWrite(LiveStreamV2::Rig),
        LivePersistencePoint::AfterStreamWrite(LiveStreamV2::Rig),
        LivePersistencePoint::BeforeStreamSync(LiveStreamV2::Rig),
        LivePersistencePoint::AfterStreamSync(LiveStreamV2::Rig),
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
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 1, 0).unwrap();
    for (index, point) in points.into_iter().enumerate() {
        let temp = tempfile::tempdir().unwrap();
        let store = BundleStore::new(temp.path().join(format!("fault-{index}{V2_BUNDLE_SUFFIX}")));
        let mut initial = command_v5_bundle();
        BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
        store.create_v3_checkpointed(&initial).unwrap();
        let hooks = Arc::new(Hooks {
            now: now + chrono::Duration::milliseconds(50),
            fail_at: Some(point),
        });
        let mut writer = store.open_v3_writer_with_hooks(hooks).unwrap();
        let revision = writer.checkpoint().revision;
        assert!(writer
            .append_antenna_control(verified_mutation(revision, now))
            .is_err());
        drop(writer);
        let reopened = store.read_v3_checkpointed().unwrap();
        assert!(matches!(reopened.rig.len(), 0 | 2), "point {point:?}");
        assert_eq!(
            reopened.rig.len() == 2,
            reopened.events.len() == 2,
            "point {point:?}"
        );
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

#[test]
fn v6_first_mutation_by_a_new_context_commits_context_before_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(format!("v6-context{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let now = Utc.with_ymd_and_hms(2026, 7, 19, 12, 0, 0).unwrap();
    let mut initial = upgrade_v5_bundle_model_to_v6(
        upgrade_v3_bundle_model_to_v5(bundle()),
        runtime_context(now, "1"),
    );
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();

    let next_context = runtime_context(now, "2");
    let next_context_id = next_context.context_id.clone();
    let mut writer = store
        .open_v3_writer_with_hooks_in_context(Arc::new(Hooks { now, fail_at: None }), next_context)
        .unwrap();
    writer
        .append_event(mutation(
            1,
            "v6-start-mutation",
            event(
                now,
                "v6-start-event",
                None,
                OperatorEventPayloadV3::SessionStarted { note: None },
            ),
        ))
        .unwrap();
    drop(writer);

    let reopened = store.read_v3_checkpointed().unwrap();
    assert_eq!(reopened.runtime_contexts.len(), 3);
    let context = reopened.runtime_contexts.last().unwrap();
    let event = reopened.events.last().unwrap();
    assert_eq!(context.context_id, next_context_id);
    assert_eq!(
        event.meta.runtime_context_id.as_deref(),
        Some(next_context_id.as_str())
    );
    assert_eq!(
        context.mutation.mutation_id,
        event.meta.mutation.mutation_id
    );
    assert_eq!(context.mutation.member_index, 0);
    assert_eq!(event.meta.mutation.member_index, 1);
    assert_eq!(context.mutation.member_count, 2);
    assert_eq!(event.meta.mutation.member_count, 2);
    assert_eq!(
        reopened.session_state.streams["runtimeContexts"].record_count,
        3
    );
}

#[test]
fn v6_recovery_by_a_new_context_commits_context_before_interruption() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp
        .path()
        .join(format!("v6-recovery-context{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let now = Utc.with_ymd_and_hms(2026, 7, 19, 12, 0, 0).unwrap();
    let mut initial = upgrade_v5_bundle_model_to_v6(
        upgrade_v3_bundle_model_to_v5(bundle()),
        runtime_context(now, "1"),
    );
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();
    let hooks = Arc::new(Hooks { now, fail_at: None });
    let mut writer = store.open_v3_writer_with_hooks(hooks.clone()).unwrap();
    writer
        .append_event(mutation(
            1,
            "v6-start-before-recovery",
            event(
                now,
                "v6-start-event-before-recovery",
                None,
                OperatorEventPayloadV3::SessionStarted { note: None },
            ),
        ))
        .unwrap();
    drop(writer);

    let recovery_context = runtime_context(now, "2");
    let recovery_context_id = recovery_context.context_id.clone();
    let report = store
        .recover_v3_with_hooks_in_context(hooks, recovery_context)
        .unwrap();
    assert!(report.interruption.is_some());
    assert!(matches!(
        report.diagnostic_persistence,
        DiagnosticPersistenceStatusV6::Persisted { .. }
    ));

    let reopened = store.read_v3_checkpointed().unwrap();
    let context = reopened.runtime_contexts.last().unwrap();
    let interruption = reopened.events.last().unwrap();
    assert_eq!(context.context_id, recovery_context_id);
    assert_eq!(
        interruption.meta.runtime_context_id.as_deref(),
        Some(recovery_context_id.as_str())
    );
    assert_eq!(
        context.mutation.mutation_id,
        interruption.meta.mutation.mutation_id
    );
    assert_eq!(context.mutation.member_index, 0);
    assert_eq!(interruption.meta.mutation.member_index, 1);
    assert_eq!(context.mutation.member_count, 2);
    assert_eq!(interruption.meta.mutation.member_count, 2);
    let diagnostic = reopened.diagnostics.last().unwrap();
    assert_eq!(
        diagnostic.operation,
        DiagnosticOperationV6::CheckpointRecovery
    );
    assert_eq!(diagnostic.outcome, DiagnosticOutcomeV6::Recovered);
}

#[test]
fn v6_diagnostic_append_is_atomic_idempotent_and_conflict_detecting() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(format!("v6-diagnostic{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let now = Utc.with_ymd_and_hms(2026, 7, 19, 12, 0, 0).unwrap();
    let mut initial = upgrade_v5_bundle_model_to_v6(
        upgrade_v3_bundle_model_to_v5(bundle()),
        runtime_context(now, "1"),
    );
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();
    let revision = initial.session_state.revision;
    let proposed = diagnostic("attempt-1", revision, now);
    let mutation = LiveDiagnosticMutationV6 {
        expected_revision: revision,
        mutation_id: "diagnostic-mutation-1".into(),
        diagnostic: proposed.clone(),
    };
    let mut writer = store.open_v3_writer().unwrap();
    let first = writer.append_diagnostic(mutation.clone()).unwrap();
    assert!(!first.idempotent);
    assert_eq!(first.revision, revision + 1);
    let repeated = writer.append_diagnostic(mutation).unwrap();
    assert!(repeated.idempotent);
    assert_eq!(repeated.diagnostic_id, first.diagnostic_id);
    let mut conflict = proposed;
    conflict.code = "session.stale_revision".into();
    assert!(matches!(
        writer.append_diagnostic(LiveDiagnosticMutationV6 {
            expected_revision: revision,
            mutation_id: "diagnostic-mutation-1".into(),
            diagnostic: conflict,
        }),
        Err(LivePersistenceError::MutationConflict { .. })
    ));
    drop(writer);

    let reopened = store.read_v3_checkpointed().unwrap();
    assert_eq!(reopened.diagnostics.len(), initial.diagnostics.len() + 1);
    assert_eq!(
        reopened.session_state.streams["diagnostics"].record_count,
        u64::try_from(reopened.diagnostics.len()).unwrap()
    );
}

#[test]
fn v6_diagnostic_persistence_failure_rolls_back_without_recursing() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp
        .path()
        .join(format!("v6-diagnostic-failure{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let now = Utc.with_ymd_and_hms(2026, 7, 19, 12, 0, 0).unwrap();
    let mut initial = upgrade_v5_bundle_model_to_v6(
        upgrade_v3_bundle_model_to_v5(bundle()),
        runtime_context(now, "1"),
    );
    BundleStore::refresh_v3_checkpoint(&mut initial).unwrap();
    store.create_v3_checkpointed(&initial).unwrap();
    let revision = initial.session_state.revision;
    let hooks = Arc::new(Hooks {
        now,
        fail_at: Some(LivePersistencePoint::BeforeStreamWrite(
            LiveStreamV2::Diagnostics,
        )),
    });
    let mut writer = store.open_v3_writer_with_hooks(hooks).unwrap();
    assert!(matches!(
        writer.append_diagnostic(LiveDiagnosticMutationV6 {
            expected_revision: revision,
            mutation_id: "diagnostic-mutation-failed".into(),
            diagnostic: diagnostic("attempt-failed", revision, now),
        }),
        Err(LivePersistenceError::Io { .. })
    ));
    drop(writer);
    assert_eq!(store.read_v3_checkpointed().unwrap(), initial);

    let mut retry = store.open_v3_writer().unwrap();
    retry
        .append_diagnostic(LiveDiagnosticMutationV6 {
            expected_revision: revision,
            mutation_id: "diagnostic-mutation-failed".into(),
            diagnostic: diagnostic("attempt-failed", revision, now),
        })
        .unwrap();
    drop(retry);
    assert_eq!(
        store.read_v3_checkpointed().unwrap().diagnostics.len(),
        initial.diagnostics.len() + 1
    );
}
