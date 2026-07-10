use antennabench_core::{
    validate_bundle, AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, BundleContents,
    BundleFiles, BundleManifest, ExperimentMode, ObservationKind, ObservationRecord, OperatorEvent,
    OperatorEventType, PlannedSlot, PropagationRecord, RecordMeta, RecordSource, RigRecord,
    Schedule, SessionGoal, Station, WsjtXRecord,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

const SESSION_ID: &str = "session-validation-test";

#[test]
fn accepts_a_valid_bundle() {
    let bundle = valid_bundle();

    validate_bundle(&bundle).unwrap();
}

fn valid_bundle() -> BundleContents {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();

    BundleContents {
        manifest: BundleManifest {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 7, 10, 19, 58, 0).unwrap(),
            app_version: "0.1.0".to_string(),
            files: BundleFiles::default(),
        },
        station: Station {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            callsign: "N1RWJ".to_string(),
            grid: "FN42".to_string(),
            power_watts: Some(5.0),
            operator_notes: None,
        },
        antennas: AntennasFile {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            antennas: vec![antenna("A", "vertical"), antenna("B", "dipole")],
        },
        schedule: Schedule {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            slots: vec![
                planned_slot("slot-001", 1, starts_at, "A"),
                planned_slot(
                    "slot-002",
                    2,
                    starts_at + chrono::Duration::seconds(120),
                    "B",
                ),
            ],
        },
        events: vec![
            operator_event(
                "event-001",
                "slot-001",
                OperatorEventType::Switched,
                starts_at + chrono::Duration::seconds(3),
            ),
            operator_event(
                "event-002",
                "slot-002",
                OperatorEventType::Switched,
                starts_at + chrono::Duration::seconds(123),
            ),
        ],
        observations: vec![
            observation(
                "obs-001",
                starts_at + chrono::Duration::seconds(60),
                Some("slot-001"),
                Some("A"),
                Some(0.95),
            ),
            observation(
                "obs-002",
                starts_at + chrono::Duration::seconds(180),
                Some("slot-002"),
                Some("B"),
                Some(0.95),
            ),
        ],
        wsjtx: vec![WsjtXRecord {
            meta: record_meta(
                starts_at - chrono::Duration::seconds(5),
                RecordSource::WsjtxLog,
            ),
            record_id: "wsjtx-001".to_string(),
            message_type: "status_snapshot".to_string(),
            raw: json!({"mode": "WSPR"}),
        }],
        rig: vec![RigRecord {
            meta: record_meta(
                starts_at - chrono::Duration::seconds(4),
                RecordSource::RigAdapter,
            ),
            record_id: "rig-001".to_string(),
            status: "manual_confirmation".to_string(),
            frequency_hz: Some(14_095_600),
            mode: Some("WSPR".to_string()),
            raw: json!({"operator_confirmed": true}),
        }],
        propagation: vec![PropagationRecord {
            meta: record_meta(starts_at, RecordSource::ImportedFile),
            record_id: "prop-001".to_string(),
            observed_at: starts_at,
            solar_flux_f107: Some(145.0),
            sunspot_number: Some(88),
            kp_index: Some(2.0),
            a_index: Some(8),
            solar_wind_speed_kms: None,
            bz_nt: None,
            alerts: Vec::new(),
            daylight_state: Some("mixed_path".to_string()),
            raw: json!({"fixture": true}),
        }],
        analysis: AnalysisFile {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: Vec::new(),
        },
    }
}

fn antenna(label: &str, facet: &str) -> Antenna {
    Antenna {
        label: label.to_string(),
        facets: vec![facet.to_string()],
        height_m: None,
        radial_count: None,
        radial_length_m: None,
        orientation_degrees: None,
        tuner: None,
        feedline: None,
        notes: None,
    }
}

fn planned_slot(
    slot_id: &str,
    sequence_number: u32,
    starts_at: chrono::DateTime<Utc>,
    antenna_label: &str,
) -> PlannedSlot {
    PlannedSlot {
        slot_id: slot_id.to_string(),
        sequence_number,
        starts_at,
        duration_seconds: 120,
        guard_seconds: 15,
        band: Band::M20,
        antenna_label: antenna_label.to_string(),
    }
}

fn operator_event(
    event_id: &str,
    slot_id: &str,
    event_type: OperatorEventType,
    timestamp: chrono::DateTime<Utc>,
) -> OperatorEvent {
    OperatorEvent {
        meta: record_meta(timestamp, RecordSource::Operator),
        event_id: event_id.to_string(),
        slot_id: Some(slot_id.to_string()),
        event_type,
        note: None,
    }
}

fn observation(
    observation_id: &str,
    timestamp: chrono::DateTime<Utc>,
    slot_id: Option<&str>,
    slot_label: Option<&str>,
    slot_confidence: Option<f32>,
) -> ObservationRecord {
    ObservationRecord {
        meta: record_meta(timestamp, RecordSource::Wsprnet),
        observation_id: observation_id.to_string(),
        observation_kind: ObservationKind::PublicReport,
        band: Band::M20,
        frequency_hz: Some(14_095_600),
        mode: Some("WSPR".to_string()),
        reporter_call: Some("K1ABC".to_string()),
        heard_call: Some("N1RWJ".to_string()),
        reporter_grid: Some("FN31".to_string()),
        heard_grid: Some("FN42".to_string()),
        distance_km: Some(150.0),
        azimuth_degrees: Some(240.0),
        snr_db: Some(-18.0),
        drift_hz_per_minute: Some(0.0),
        power_watts: Some(5.0),
        slot_id: slot_id.map(str::to_string),
        slot_label: slot_label.map(str::to_string),
        slot_confidence,
        raw: json!({}),
    }
}

fn record_meta(timestamp: chrono::DateTime<Utc>, source: RecordSource) -> RecordMeta {
    RecordMeta {
        schema_version: 1,
        session_id: SESSION_ID.to_string(),
        timestamp,
        source,
    }
}
