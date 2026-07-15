use std::{collections::BTreeMap, fs};

use antennabench_core::{
    AcquisitionChannelId, AdapterId, AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band,
    BundleFilesV3, BundleManifestV3, BundleV3Contents, CounterbalanceBlockIdV3, EventTimeBasisV2,
    ExperimentMode, MutationMember, OperatorEventPayloadV3, OperatorEventV3, PlanGenerationV2,
    PlannedSlotV3, Provenance, ProviderId, RecordMetaV3, ScheduleV3, SessionGoal,
    SessionLifecycleV2, SessionStateV3, SignalAllocationV3, SignalCadenceV3,
    SignalCollectionProfileV3, SignalModeV3, SignalPlanIdV3, SignalPlanV3,
    SignalStateConfirmationV3, SignalVariantIdV3, SourceId, Station, SCHEMA_VERSION_V3,
    V2_BUNDLE_SUFFIX,
};
use antennabench_storage::{BundleStore, BundleStoreError};
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
    };

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
        events: vec![OperatorEventV3 {
            meta,
            event_id: "signal-confirmation-1".into(),
            occurred_at: now,
            time_basis: EventTimeBasisV2::OperatorReported,
            uncertainty_seconds: None,
            slot_id: Some(slot_id),
            payload: OperatorEventPayloadV3::SignalStateConfirmed {
                confirmation: SignalStateConfirmationV3 {
                    frequency_hz: Some(14_050_000),
                    mode: Some(SignalModeV3::Cw),
                    power_watts: Some(10.0),
                    transmitted_callsign: Some("N1RWJ".into()),
                    cadence_followed: Some(true),
                    note: None,
                },
            },
        }],
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

#[test]
fn v3_static_bundle_round_trips_with_signal_plan_and_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(format!("test{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(&path);
    let mut bundle = bundle();
    BundleStore::refresh_v3_checkpoint(&mut bundle).unwrap();

    store.write_v3(&bundle).unwrap();

    assert_eq!(store.read_v3().unwrap(), bundle);
    assert_eq!(
        store.read_v3().unwrap().schedule.signal_plans[0].signal_plan_id,
        SignalPlanIdV3::new("manual-cw").unwrap()
    );

    let copy_path = temp.path().join(format!("copy{V2_BUNDLE_SUFFIX}"));
    let copied = store.copy_losslessly_to(&copy_path).unwrap();
    assert_eq!(copied.read_v3().unwrap(), bundle);
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
