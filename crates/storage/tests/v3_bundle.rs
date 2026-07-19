use std::{collections::BTreeMap, fs, path::Path};

use antennabench_core::{
    v2::{
        AcquisitionChannelId, AdapterId, AdapterInput, EventTimeBasisV2, MutationMember,
        PlanGenerationV2, Provenance, ProviderId, SessionLifecycleV2, SourceId, V2_BUNDLE_SUFFIX,
    },
    v3::{
        BundleFilesV3, BundleManifestV3, BundleV3Contents, CorrectableOperatorEventPayloadV3,
        CounterbalanceBlockIdV3, EventCorrectionActionV3, OperatorEventPayloadV3, OperatorEventV3,
        PlannedSlotV3, RecordMetaV3, ReplacementOperatorEventV3, RigRecordV3, ScheduleV3,
        SessionStateV3, SignalAllocationV3, SignalCadenceV3, SignalCollectionProfileV3,
        SignalModeV3, SignalPlanIdV3, SignalPlanV3, SignalStateConfirmationV3, SignalVariantIdV3,
        WsprCycleDirection, WsprCycleIntentV3,
    },
    v5::{upgrade_v3_bundle_model_to_v5, WsprReadinessBasisV5},
    v6::{BuildChannelV6, BuildIdentityV6, RuntimeContextV6, RuntimePlatformV6, SourceStateV6},
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, ExperimentMode, SessionGoal,
    Station, SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5, SCHEMA_VERSION_V6,
};
use antennabench_storage::{BundleAttachment, BundleStore, BundleStoreError};
use chrono::{TimeZone, Utc};

fn bundle() -> BundleV3Contents {
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
    let session_id = "session-v3-static".to_string();
    let signal_plan_id = SignalPlanIdV3::new("manual-cw").unwrap();
    let slot_id = "slot-1".to_string();
    let meta = RecordMetaV3 {
        schema_version: SCHEMA_VERSION_V3,
        session_id: session_id.clone(),
        recorded_at: now,
        provenance: Provenance {
            provider_id: ProviderId::new("antennabench").unwrap(),
            source_id: SourceId::new("operator-evidence").unwrap(),
            acquisition_channel: AcquisitionChannelId::new("operator-entry").unwrap(),
            adapter_id: AdapterId::new("antennabench.operator").unwrap(),
            adapter_version: "3".into(),
        },
        mutation: MutationMember {
            mutation_id: "mutation-1".into(),
            member_index: 0,
            member_count: 1,
        },
        runtime_context_id: None,
    };

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
            revision: 1,
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
            last_committed_mutation_id: Some("mutation-1".into()),
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
                signal_plan_id: signal_plan_id.clone(),
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
                slot_id: slot_id.clone(),
                sequence_number: 1,
                starts_at: now,
                duration_seconds: 20,
                guard_seconds: 5,
                band: Band::M20,
                antenna_label: "A".into(),
                signal: Some(SignalAllocationV3 {
                    signal_plan_id,
                    frequency_hz: 14_050_000,
                    frequency_variant_id: SignalVariantIdV3::new("fixed").unwrap(),
                    counterbalance_block_id: CounterbalanceBlockIdV3::new("block-1").unwrap(),
                    counterbalance_position: 0,
                }),
            }],
        },
        events: vec![
            OperatorEventV3 {
                meta: meta.clone(),
                event_id: "signal-confirmation-1".into(),
                occurred_at: now,
                time_basis: EventTimeBasisV2::OperatorReported,
                uncertainty_seconds: None,
                slot_id: Some(slot_id.clone()),
                payload: OperatorEventPayloadV3::SignalStateConfirmed {
                    confirmation: SignalStateConfirmationV3 {
                        frequency_hz: Some(14_050_100),
                        mode: Some(SignalModeV3::Cw),
                        power_watts: Some(10.0),
                        transmitted_callsign: Some("N1RWJ".into()),
                        cadence_followed: Some(true),
                        note: Some("initial entry".into()),
                    },
                },
            },
            OperatorEventV3 {
                meta: RecordMetaV3 {
                    mutation: MutationMember {
                        mutation_id: "mutation-2".into(),
                        member_index: 0,
                        member_count: 1,
                    },
                    ..meta.clone()
                },
                event_id: "signal-correction-1".into(),
                occurred_at: now,
                time_basis: EventTimeBasisV2::OperatorReported,
                uncertainty_seconds: None,
                slot_id: None,
                payload: OperatorEventPayloadV3::EventCorrected {
                    target_event_id: "signal-confirmation-1".into(),
                    correction: EventCorrectionActionV3::Replace {
                        replacement: ReplacementOperatorEventV3 {
                            occurred_at: now,
                            time_basis: EventTimeBasisV2::OperatorReported,
                            uncertainty_seconds: None,
                            slot_id: Some(slot_id),
                            payload: CorrectableOperatorEventPayloadV3::SignalStateConfirmed {
                                confirmation: SignalStateConfirmationV3 {
                                    frequency_hz: Some(14_050_000),
                                    mode: Some(SignalModeV3::Cw),
                                    power_watts: Some(10.0),
                                    transmitted_callsign: Some("N1RWJ".into()),
                                    cadence_followed: Some(true),
                                    note: Some("corrected from operator log".into()),
                                },
                            },
                        },
                    },
                    reason: "frequency was entered incorrectly".into(),
                },
            },
        ],
        observations: Vec::new(),
        adapter_records: Vec::new(),
        rig: vec![RigRecordV3 {
            meta: RecordMetaV3 {
                mutation: MutationMember {
                    mutation_id: "mutation-3".into(),
                    member_index: 0,
                    member_count: 1,
                },
                ..meta
            },
            record_id: "rig-1".into(),
            adapter_record_ids: Vec::new(),
            status: "observed".into(),
            frequency_hz: Some(14_050_000),
            mode: Some("CW".into()),
            power_watts: Some(9.5),
            antenna_control: None,
            raw: serde_json::json!({"source": "manual read-back"}),
        }],
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

fn upgrader_context() -> RuntimeContextV6 {
    RuntimeContextV6::new(
        Utc.with_ymd_and_hms(2026, 7, 19, 12, 0, 0).unwrap(),
        MutationMember {
            mutation_id: "pending-upgrade".into(),
            member_index: 0,
            member_count: 1,
        },
        BuildIdentityV6 {
            app_version: Some("0.2.0-dev".into()),
            source_commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
            source_state: SourceStateV6::Dirty,
            build_channel: BuildChannelV6::Development,
            release_tag: None,
            target_triple: Some("aarch64-apple-darwin".into()),
            build_architecture: Some("aarch64".into()),
            build_timestamp: None,
        },
        RuntimePlatformV6 {
            os_family: Some("macos".into()),
            os_version: Some("15.6".into()),
            runtime_architecture: Some("aarch64".into()),
            application_id: Some("com.rwjblue.antennabench".into()),
        },
    )
}

#[test]
fn v3_static_bundle_round_trips_with_signal_plan_and_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(format!("test{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let mut bundle = bundle();
    BundleStore::refresh_v3_checkpoint(&mut bundle).unwrap();

    store.write_v3(&bundle).unwrap();
    assert!(!fs::read_to_string(path.join("schedule.json"))
        .unwrap()
        .contains("antenna_control"));

    assert_eq!(store.read_v3().unwrap(), bundle);
    assert_eq!(
        store.read_v3().unwrap().schedule.signal_plans[0].signal_plan_id,
        SignalPlanIdV3::new("manual-cw").unwrap()
    );
    assert_eq!(store.read_v3().unwrap().events.len(), 2);
    assert_eq!(store.read_v3().unwrap().rig[0].power_watts, Some(9.5));
    let current = store.read_current().unwrap();
    assert!(current.bundle.events.is_empty());
    assert!(current
        .record_provenance
        .iter()
        .any(|record| record.record_id == "signal-confirmation-1"));

    let copy_path = temp.path().join(format!("copy{V2_BUNDLE_SUFFIX}"));
    let copied = store.copy_losslessly_to(&copy_path).unwrap();
    assert_eq!(copied.read_v3().unwrap(), bundle);
}

#[test]
fn v5_upgrade_to_v6_keeps_legacy_identity_unknown_and_records_real_upgrader() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join(format!("source{V2_BUNDLE_SUFFIX}"));
    let destination_path = temp.path().join(format!("upgraded{V2_BUNDLE_SUFFIX}"));
    let source = BundleStore::new(&source_path);
    let mut v5 = upgrade_v3_bundle_model_to_v5(bundle());
    BundleStore::refresh_v3_checkpoint(&mut v5).unwrap();
    source.write_v3(&v5).unwrap();

    let upgraded_store = source
        .upgrade_v5_to_v6(&destination_path, upgrader_context())
        .unwrap();
    let upgraded = upgraded_store.read_v3().unwrap();
    assert_eq!(upgraded.manifest.schema_version, SCHEMA_VERSION_V6);
    assert_eq!(upgraded.runtime_contexts.len(), 2);
    let legacy = &upgraded.runtime_contexts[0];
    assert_eq!(legacy.build.app_version.as_deref(), Some("test"));
    assert_eq!(legacy.build.source_state, SourceStateV6::Unknown);
    assert_eq!(legacy.build.build_channel, BuildChannelV6::Unknown);
    assert!(legacy.build.source_commit.is_none());
    assert!(legacy.platform.os_family.is_none());
    assert_eq!(
        upgraded.manifest.creator_runtime_context_id.as_deref(),
        Some(legacy.context_id.as_str())
    );
    assert_eq!(
        upgraded.session_state.active_runtime_context_id.as_deref(),
        Some(upgraded.runtime_contexts[1].context_id.as_str())
    );
    assert!(upgraded.events.iter().all(|event| {
        event.meta.runtime_context_id.as_deref() == Some(legacy.context_id.as_str())
    }));
    assert_eq!(source.read_v3().unwrap(), v5);
}

#[test]
fn schema_v6_reader_rejects_n_plus_one() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join(format!("source{V2_BUNDLE_SUFFIX}"));
    let destination_path = temp.path().join(format!("upgraded{V2_BUNDLE_SUFFIX}"));
    let source = BundleStore::new(&source_path);
    let mut v5 = upgrade_v3_bundle_model_to_v5(bundle());
    BundleStore::refresh_v3_checkpoint(&mut v5).unwrap();
    source.write_v3(&v5).unwrap();
    let upgraded = source
        .upgrade_v5_to_v6(&destination_path, upgrader_context())
        .unwrap();
    let manifest_path = destination_path.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["schema_version"] = serde_json::json!(SCHEMA_VERSION_V6 + 1);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        upgraded.read_v3(),
        Err(BundleStoreError::UnsupportedSchemaVersion { actual })
            if actual == SCHEMA_VERSION_V6 + 1
    ));
}

#[test]
fn schema_v3_and_v4_upgrade_to_v5_without_inventing_command_evidence() {
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
    let mut source = bundle();
    for version in [SCHEMA_VERSION_V3, SCHEMA_VERSION_V4] {
        source.manifest.schema_version = version;
        source.session_state.schema_version = version;
        source.station.schema_version = version;
        source.antennas.schema_version = version;
        source.schedule.schema_version = version;
        source.analysis.schema_version = version;
        source.schedule.signal_plans.clear();
        source.schedule.slots.clear();
        source.schedule.wspr_cycle_intents = vec![WsprCycleIntentV3 {
            intent_id: "historical-intent".into(),
            sequence_number: 1,
            band: Band::M20,
            antenna_label: "A".into(),
            direction: (version >= SCHEMA_VERSION_V4).then_some(WsprCycleDirection::Transmit),
            signal: None,
        }];
        source.events = vec![OperatorEventV3 {
            meta: RecordMetaV3 {
                schema_version: version,
                session_id: source.manifest.session_id.clone(),
                recorded_at: now,
                provenance: source.rig[0].meta.provenance.clone(),
                mutation: MutationMember {
                    mutation_id: "historical-ready".into(),
                    member_index: 0,
                    member_count: 1,
                },
                runtime_context_id: None,
            },
            event_id: "historical-ready-event".into(),
            occurred_at: now,
            time_basis: EventTimeBasisV2::OperatorReported,
            uncertainty_seconds: None,
            slot_id: Some("historical-intent".into()),
            payload: OperatorEventPayloadV3::WsprCycleArmed {
                antenna_label: "A".into(),
                cycle_starts_at: now + chrono::Duration::seconds(1),
                readiness: None,
            },
        }];
        let upgraded = upgrade_v3_bundle_model_to_v5(source.clone());
        assert_eq!(upgraded.manifest.schema_version, SCHEMA_VERSION_V5);
        assert_eq!(
            upgraded.schedule.antenna_control,
            Some(antennabench_core::v5::AntennaControlPolicyV5::Manual)
        );
        assert!(upgraded
            .rig
            .iter()
            .all(|record| record.antenna_control.is_none()));
        assert!(matches!(
            upgraded.events[0].payload,
            OperatorEventPayloadV3::WsprCycleArmed {
                readiness: Some(WsprReadinessBasisV5::OperatorConfirmed),
                ..
            }
        ));
    }
}

#[test]
fn schema_v3_storage_upgrade_to_v5_is_non_destructive() {
    let temp = tempfile::tempdir().unwrap();
    for version in [SCHEMA_VERSION_V3, SCHEMA_VERSION_V4] {
        let source_path = temp
            .path()
            .join(format!("source-v{version}{V2_BUNDLE_SUFFIX}"));
        let destination_path = temp
            .path()
            .join(format!("destination-v{version}{V2_BUNDLE_SUFFIX}"));
        let source_store = BundleStore::new(&source_path);
        let mut source = bundle();
        source.manifest.schema_version = version;
        source.session_state.schema_version = version;
        source.station.schema_version = version;
        source.antennas.schema_version = version;
        source.schedule.schema_version = version;
        source.analysis.schema_version = version;
        for event in &mut source.events {
            event.meta.schema_version = version;
        }
        for record in &mut source.rig {
            record.meta.schema_version = version;
        }
        BundleStore::refresh_v3_checkpoint(&mut source).unwrap();
        source_store.write_v3(&source).unwrap();
        let before = fs::read(source_path.join("manifest.json")).unwrap();
        let destination = source_store.upgrade_v3_to_v5(&destination_path).unwrap();
        assert_eq!(
            destination.read_v3().unwrap().manifest.schema_version,
            SCHEMA_VERSION_V5
        );
        assert_eq!(fs::read(source_path.join("manifest.json")).unwrap(), before);
    }
}

#[test]
fn v3_read_fails_closed_on_checkpoint_corruption() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(format!("test{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let mut bundle = bundle();
    BundleStore::refresh_v3_checkpoint(&mut bundle).unwrap();
    store.write_v3(&bundle).unwrap();

    let schedule_path = path.join("schedule.json");
    let mut bytes = fs::read(&schedule_path).unwrap();
    bytes.push(b'\n');
    fs::write(schedule_path, bytes).unwrap();

    assert!(matches!(
        store.read_v3(),
        Err(BundleStoreError::InvalidV3Bundle { .. })
    ));
}

#[test]
fn v2_upgrade_preserves_evidence_without_inventing_signal_facts() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    let v2_path = temp.path().join(format!("source{V2_BUNDLE_SUFFIX}"));
    let v3_path = temp.path().join(format!("destination{V2_BUNDLE_SUFFIX}"));
    let v2_store = BundleStore::new(&fixture)
        .upgrade_v1_to_v2(&v2_path)
        .unwrap();
    let before = v2_store.read_v2().unwrap();

    let v3_store = v2_store.upgrade_v2_to_v3(&v3_path).unwrap();
    let after = v3_store.read_v3().unwrap();

    assert_eq!(v2_store.read_v2().unwrap(), before);
    assert_eq!(after.manifest.schema_version, SCHEMA_VERSION_V3);
    assert_eq!(after.session_state.schema_version, SCHEMA_VERSION_V3);
    assert_eq!(after.station.schema_version, SCHEMA_VERSION_V3);
    assert_eq!(after.antennas.schema_version, SCHEMA_VERSION_V3);
    assert_eq!(after.schedule.schema_version, SCHEMA_VERSION_V3);
    assert_eq!(after.analysis.schema_version, SCHEMA_VERSION_V3);
    assert_eq!(after.schedule.slots.len(), before.schedule.slots.len());
    assert_eq!(after.events.len(), before.events.len());
    assert_eq!(after.observations.len(), before.observations.len());
    assert_eq!(after.adapter_records.len(), before.adapter_records.len());
    assert_eq!(after.rig.len(), before.rig.len());
    assert_eq!(after.propagation.len(), before.propagation.len());
    assert!(after.schedule.signal_plans.is_empty());
    assert!(after
        .schedule
        .slots
        .iter()
        .all(|slot| slot.signal.is_none()));
    assert!(after.rig.iter().all(|record| record.power_watts.is_none()));
    assert!(after.events.iter().all(|event| !matches!(
        event.payload,
        OperatorEventPayloadV3::SignalStateConfirmed { .. }
    )));
    assert!(after
        .events
        .iter()
        .all(|record| record.meta.schema_version == SCHEMA_VERSION_V3));
    assert!(after
        .observations
        .iter()
        .all(|record| record.meta.schema_version == SCHEMA_VERSION_V3));
    assert!(after
        .adapter_records
        .iter()
        .all(|record| record.meta.schema_version == SCHEMA_VERSION_V3));
    assert!(after
        .rig
        .iter()
        .all(|record| record.meta.schema_version == SCHEMA_VERSION_V3));
    assert!(after
        .propagation
        .iter()
        .all(|record| record.meta.schema_version == SCHEMA_VERSION_V3));
}

#[test]
fn v2_upgrade_copies_and_verifies_referenced_attachments() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    let baseline_path = temp.path().join(format!("baseline{V2_BUNDLE_SUFFIX}"));
    let baseline = BundleStore::new(&fixture)
        .upgrade_v1_to_v2(&baseline_path)
        .unwrap();
    let mut bundle = baseline.read_v2().unwrap();
    let attachment = BundleAttachment::new(
        b"exact attachment evidence".to_vec(),
        "application/octet-stream",
        None,
        Some("opaque".into()),
        Some("capture.bin".into()),
    );
    bundle.adapter_records[0].input = AdapterInput::Attachment {
        attachment: attachment.reference.clone(),
    };
    BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();
    let source_path = temp.path().join(format!("source{V2_BUNDLE_SUFFIX}"));
    let source = BundleStore::new(&source_path);
    source
        .write_v2_with_attachments(&bundle, std::slice::from_ref(&attachment))
        .unwrap();

    let destination_path = temp.path().join(format!("destination{V2_BUNDLE_SUFFIX}"));
    let destination = source.upgrade_v2_to_v3(&destination_path).unwrap();
    let upgraded = destination.read_v3().unwrap();

    assert!(matches!(
        upgraded.adapter_records[0].input,
        AdapterInput::Attachment { .. }
    ));
    assert_eq!(
        destination.read_attachment(&attachment.reference).unwrap(),
        attachment.bytes
    );
}

#[test]
fn direct_v1_upgrade_matches_the_deterministic_two_step_model() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    let direct_path = temp.path().join(format!("direct{V2_BUNDLE_SUFFIX}"));
    let intermediate_path = temp.path().join(format!("intermediate{V2_BUNDLE_SUFFIX}"));
    let two_step_path = temp.path().join(format!("two-step{V2_BUNDLE_SUFFIX}"));

    let direct = BundleStore::new(&fixture)
        .upgrade_v1_to_v3(&direct_path)
        .unwrap();
    let intermediate = BundleStore::new(&fixture)
        .upgrade_v1_to_v2(&intermediate_path)
        .unwrap();
    let two_step = intermediate.upgrade_v2_to_v3(&two_step_path).unwrap();

    assert_eq!(direct.read_v3().unwrap(), two_step.read_v3().unwrap());

    let direct_v5_path = temp.path().join(format!("direct-v5{V2_BUNDLE_SUFFIX}"));
    let two_step_v5_path = temp.path().join(format!("two-step-v5{V2_BUNDLE_SUFFIX}"));
    let direct_v5 = BundleStore::new(&fixture)
        .upgrade_v1_to_v5(&direct_v5_path)
        .unwrap();
    let two_step_v5 = intermediate.upgrade_v2_to_v5(&two_step_v5_path).unwrap();
    assert_eq!(direct_v5.read_v3().unwrap(), two_step_v5.read_v3().unwrap());
    assert_eq!(
        direct_v5.read_v3().unwrap().manifest.schema_version,
        SCHEMA_VERSION_V5
    );
}
