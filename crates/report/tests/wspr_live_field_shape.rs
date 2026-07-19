use std::collections::BTreeMap;

use antennabench_analysis::{summarize_bundle, ComparisonAvailability};
use antennabench_core::{
    v2::{
        AcquisitionChannelId, AdapterDisposition, AdapterId, AdapterInput, AdapterReasonId,
        AdapterRecordV2, EventTimeBasisV2, MutationMember, NormalizedRecordKind,
        NormalizedRecordLink, ObservationRecordV2, PlanGenerationV2, Provenance, ProviderId,
        RecordMetaV2, SessionLifecycleV2, SourceId,
    },
    v3::{
        BundleFilesV3, BundleManifestV3, BundleV3Contents, OperatorEventPayloadV3, OperatorEventV3,
        RecordMetaV3, ScheduleV3, SessionStateV3, WsprCycleDirection, WsprCycleIntentV3,
    },
    v5::{AntennaControlPolicyV5, WsprReadinessBasisV5},
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, ExperimentMode, ObservationKind,
    SessionGoal, Station, SCHEMA_VERSION_V5,
};
use antennabench_report::{build_report, render_compact_summary_html, render_standalone_html};
use chrono::{DateTime, Duration, TimeZone, Utc};

const SESSION_ID: &str = "session-synthetic-wspr-live-field-shape";
const FIRST_SLOT_ID: &str = "wspr-cycle-a";
const SECOND_SLOT_ID: &str = "wspr-cycle-b";
const FIRST_SPOT_COUNT: usize = 145;
const SECOND_SPOT_COUNT: usize = 43;
const OVERLAPPING_REPORTER_COUNT: usize = 33;

#[test]
fn confirmed_source_cycles_survive_projection_analysis_and_both_reports() {
    let durable = field_shape_fixture();
    assert_eq!(durable.observations.len(), 188);
    assert!(durable.observations.iter().all(|observation| {
        observation.meta.recorded_at > utc(12, 2, 1)
            && observation.slot_id.is_none()
            && observation.slot_label.is_none()
            && observation.slot_confidence.is_none()
    }));

    let current = durable.into_current().bundle;
    let first_count = current
        .observations
        .iter()
        .filter(|observation| observation.slot_id.as_deref() == Some(FIRST_SLOT_ID))
        .count();
    let second_count = current
        .observations
        .iter()
        .filter(|observation| observation.slot_id.as_deref() == Some(SECOND_SLOT_ID))
        .count();
    assert_eq!((first_count, second_count), (145, 43));
    assert!(current.observations.iter().all(|observation| {
        observation.slot_confidence == Some(0.95)
            && matches!(
                observation.meta.timestamp,
                timestamp if timestamp == utc(12, 0, 1) || timestamp == utc(12, 2, 1)
            )
    }));

    let summary = summarize_bundle(&current).expect("field-shape fixture should analyze");
    assert_eq!(summary.overall.observation_counts.total, 188);
    assert_eq!(summary.overall.observation_counts.usable, 188);
    assert_eq!(summary.overall.observation_counts.excluded, 0);
    assert_eq!(
        summary.comparison.availability,
        ComparisonAvailability::DescriptivePairsAvailable
    );
    assert_eq!(summary.comparison.paired_rows.len(), 33);
    assert_eq!(summary.comparison.diagnostics.unique_path_count, 33);

    let report = build_report(&current).expect("field-shape fixture should build a report");
    assert_eq!(report.evidence.overall.observation_counts.usable, 188);
    assert_eq!(report.comparison.paired_rows.len(), 33);
    for html in [
        render_standalone_html(&report).expect("full report should render"),
        render_compact_summary_html(&report).expect("compact report should render"),
    ] {
        assert!(!html.contains("0 usable"));
        assert!(!html.contains("No matched paths"));
    }
}

fn field_shape_fixture() -> BundleV3Contents {
    let first_cycle = utc(12, 0, 1);
    let second_cycle = utc(12, 2, 1);
    let captured_at = utc(12, 7, 2);
    let mut adapter_records = Vec::with_capacity(FIRST_SPOT_COUNT + SECOND_SPOT_COUNT);
    let mut observations = Vec::with_capacity(FIRST_SPOT_COUNT + SECOND_SPOT_COUNT);

    append_cycle_spots(
        &mut adapter_records,
        &mut observations,
        FIRST_SLOT_ID,
        first_cycle - Duration::seconds(1),
        captured_at,
        0..FIRST_SPOT_COUNT,
    );
    append_cycle_spots(
        &mut adapter_records,
        &mut observations,
        SECOND_SLOT_ID,
        second_cycle - Duration::seconds(1),
        captured_at,
        (0..OVERLAPPING_REPORTER_COUNT).chain(FIRST_SPOT_COUNT..155),
    );

    BundleV3Contents {
        manifest: BundleManifestV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            created_at: utc(11, 59, 0),
            app_version: "synthetic-fixture".into(),
            files: BundleFilesV3::default(),
            creator_runtime_context_id: None,
        },
        session_state: SessionStateV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            revision: 192,
            lifecycle: SessionLifecycleV2::Ended,
            wspr_live_acquisition_enabled: true,
            active_plan: PlanGenerationV2 {
                generation_id: "synthetic-plan".into(),
                station_sha256: String::new(),
                antennas_sha256: String::new(),
                schedule_sha256: String::new(),
                root_sha256: String::new(),
            },
            streams: BTreeMap::new(),
            last_committed_mutation_id: Some("session-ended".into()),
            active_runtime_context_id: None,
            diagnostics_status: None,
        },
        station: Station {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            callsign: "N0CALL".into(),
            grid: "AA00".into(),
            power_watts: Some(5.0),
            operator_notes: None,
        },
        antennas: AntennasFile {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            antennas: vec![antenna("A"), antenna("B")],
        },
        schedule: ScheduleV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            antenna_control: Some(AntennaControlPolicyV5::Manual),
            signal_plans: Vec::new(),
            wspr_cycle_intents: vec![
                cycle_intent(FIRST_SLOT_ID, 1, "A"),
                cycle_intent(SECOND_SLOT_ID, 2, "B"),
            ],
            slots: Vec::new(),
        },
        events: vec![
            event(
                "session-started",
                utc(11, 59, 20),
                None,
                OperatorEventPayloadV3::SessionStarted { note: None },
            ),
            event(
                "cycle-a-armed",
                utc(11, 59, 50),
                Some(FIRST_SLOT_ID),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label: "A".into(),
                    cycle_starts_at: first_cycle,
                    readiness: Some(WsprReadinessBasisV5::OperatorConfirmed),
                },
            ),
            event(
                "cycle-b-armed",
                utc(12, 1, 55),
                Some(SECOND_SLOT_ID),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label: "B".into(),
                    cycle_starts_at: second_cycle,
                    readiness: Some(WsprReadinessBasisV5::OperatorConfirmed),
                },
            ),
            event(
                "session-ended",
                utc(12, 3, 55),
                None,
                OperatorEventPayloadV3::SessionEnded { reason: None },
            ),
        ],
        observations,
        adapter_records,
        rig: Vec::new(),
        propagation: Vec::new(),
        analysis: AnalysisFile {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: Vec::new(),
        },
        runtime_contexts: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn append_cycle_spots(
    adapter_records: &mut Vec<AdapterRecordV2>,
    observations: &mut Vec<ObservationRecordV2>,
    slot_id: &str,
    source_time: DateTime<Utc>,
    captured_at: DateTime<Utc>,
    reporters: impl IntoIterator<Item = usize>,
) {
    for reporter_index in reporters {
        let observation_id = format!("{slot_id}-spot-{reporter_index:03}");
        let adapter_id = format!("{slot_id}-adapter-{reporter_index:03}");
        let mutation_id = format!("{slot_id}-mutation-{reporter_index:03}");
        let reporter_call = synthetic_callsign(reporter_index);
        adapter_records.push(AdapterRecordV2 {
            meta: record_meta(captured_at, &mutation_id, 0),
            record_id: adapter_id.clone(),
            source_time: Some(source_time),
            record_type: "wspr_live_spot".into(),
            disposition: AdapterDisposition::Accepted,
            reason: AdapterReasonId::new("wspr-live.accepted").unwrap(),
            normalized_records: vec![NormalizedRecordLink {
                record_kind: NormalizedRecordKind::Observation,
                record_id: observation_id.clone(),
            }],
            input: AdapterInput::Inline {
                data: format!(r#"{{"time":"{source_time}","rx_sign":"{reporter_call}"}}"#),
                media_type: "application/json".into(),
                encoding: None,
                source_locator: Some("synthetic-field-shape.json".into()),
            },
        });
        observations.push(ObservationRecordV2 {
            meta: record_meta(captured_at, &mutation_id, 1),
            observation_id,
            adapter_record_ids: vec![adapter_id],
            observation_kind: ObservationKind::ImportedSpot,
            band: Band::M20,
            frequency_hz: Some(14_095_600),
            mode: Some("WSPR".into()),
            reporter_call: Some(reporter_call),
            heard_call: Some("N0CALL".into()),
            reporter_grid: Some("AA00".into()),
            heard_grid: Some("AA00".into()),
            distance_km: Some(1000.0 + reporter_index as f64),
            azimuth_degrees: Some((reporter_index % 360) as f64),
            snr_db: Some(-25.0 + (reporter_index % 15) as f32),
            drift_hz_per_minute: Some(0.0),
            power_watts: Some(5.0),
            slot_id: None,
            slot_label: None,
            slot_confidence: None,
            raw: serde_json::json!({
                "provider_spot_id": format!("{slot_id}-{reporter_index}"),
                "provider": "wspr-live",
                "source": "wsprnet-spots-mirror",
                "direction": "transmit",
            }),
        });
    }
}

fn record_meta(recorded_at: DateTime<Utc>, mutation_id: &str, member_index: u32) -> RecordMetaV2 {
    RecordMetaV2 {
        schema_version: SCHEMA_VERSION_V5,
        session_id: SESSION_ID.into(),
        recorded_at,
        provenance: wspr_live_provenance(),
        mutation: MutationMember {
            mutation_id: mutation_id.into(),
            member_index,
            member_count: 2,
        },
        runtime_context_id: None,
    }
}

fn wspr_live_provenance() -> Provenance {
    Provenance {
        provider_id: ProviderId::new("wspr-live").unwrap(),
        source_id: SourceId::new("wsprnet-spots-mirror").unwrap(),
        acquisition_channel: AcquisitionChannelId::new("https-query").unwrap(),
        adapter_id: AdapterId::new("antennabench.wspr-live-json").unwrap(),
        adapter_version: "synthetic-fixture".into(),
    }
}

fn event(
    event_id: &str,
    occurred_at: DateTime<Utc>,
    slot_id: Option<&str>,
    payload: OperatorEventPayloadV3,
) -> OperatorEventV3 {
    OperatorEventV3 {
        meta: RecordMetaV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            recorded_at: occurred_at,
            provenance: Provenance {
                provider_id: ProviderId::new("antennabench").unwrap(),
                source_id: SourceId::new("operator-evidence").unwrap(),
                acquisition_channel: AcquisitionChannelId::new("operator-entry").unwrap(),
                adapter_id: AdapterId::new("antennabench.operator").unwrap(),
                adapter_version: "synthetic-fixture".into(),
            },
            mutation: MutationMember {
                mutation_id: event_id.into(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        event_id: event_id.into(),
        occurred_at,
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: slot_id.map(str::to_string),
        payload,
    }
}

fn cycle_intent(id: &str, sequence_number: u32, antenna_label: &str) -> WsprCycleIntentV3 {
    WsprCycleIntentV3 {
        intent_id: id.into(),
        sequence_number,
        band: Band::M20,
        antenna_label: antenna_label.into(),
        direction: Some(WsprCycleDirection::Transmit),
        signal: None,
    }
}

fn antenna(label: &str) -> Antenna {
    Antenna {
        label: label.into(),
        facets: Vec::new(),
        height_m: None,
        radial_count: None,
        radial_length_m: None,
        orientation_degrees: None,
        tuner: None,
        feedline: None,
        notes: None,
    }
}

fn synthetic_callsign(index: usize) -> String {
    let first = char::from(b'A' + u8::try_from(index / 26).unwrap());
    let second = char::from(b'A' + u8::try_from(index % 26).unwrap());
    format!("K0{first}{second}")
}

fn utc(hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 19, hour, minute, second)
        .unwrap()
}
