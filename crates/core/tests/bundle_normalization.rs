use antennabench_core::{
    annotate_bundle_observations, normalize_bundle, validate_bundle, AnalysisFile, AnalysisStatus,
    Antenna, AntennasFile, Band, BundleContents, BundleFiles, BundleManifest, ExperimentMode,
    ObservationKind, ObservationRecord, OperatorEvent, OperatorEventType, PlannedSlot,
    PropagationRecord, RecordMeta, RecordSource, RigRecord, Schedule, SessionGoal, Station,
    WsjtXRecord,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

const SESSION_ID: &str = "session-normalization-test";

#[test]
fn normalize_bundle_rewrites_missing_stale_and_already_correct_annotations() {
    let starts_at = fixture_start();
    let mut bundle = bundle_with_observations(vec![
        observation(
            "obs-missing",
            starts_at + chrono::Duration::seconds(60),
            Band::M20,
            None,
            None,
            None,
        ),
        observation(
            "obs-stale",
            starts_at + chrono::Duration::seconds(180),
            Band::M20,
            Some("slot-001"),
            Some("A"),
            Some(0.25),
        ),
        observation(
            "obs-correct",
            starts_at + chrono::Duration::seconds(300),
            Band::M20,
            Some("slot-003"),
            Some("A"),
            Some(0.95),
        ),
    ]);
    bundle.events = vec![
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
        operator_event(
            "event-003",
            "slot-003",
            OperatorEventType::Switched,
            starts_at + chrono::Duration::seconds(243),
        ),
    ];

    let normalized = normalize_bundle(bundle);

    insta::assert_json_snapshot!(
        annotation_snapshot(&normalized),
        @r###"
        [
          {
            "observation_id": "obs-missing",
            "slot_confidence": 0.95,
            "slot_id": "slot-001",
            "slot_label": "A"
          },
          {
            "observation_id": "obs-stale",
            "slot_confidence": 0.95,
            "slot_id": "slot-002",
            "slot_label": "B"
          },
          {
            "observation_id": "obs-correct",
            "slot_confidence": 0.95,
            "slot_id": "slot-003",
            "slot_label": "A"
          }
        ]
        "###
    );
    validate_bundle(&normalized).unwrap();
}

#[test]
fn annotate_bundle_observations_only_mutates_alignment_fields_and_returns_alignment() {
    let starts_at = fixture_start();
    let mut bundle = bundle_with_observations(vec![
        observation(
            "obs-preserved",
            starts_at + chrono::Duration::seconds(60),
            Band::M20,
            Some("stale-slot"),
            Some("stale-label"),
            Some(0.01),
        ),
        observation(
            "obs-preserved-too",
            starts_at + chrono::Duration::seconds(180),
            Band::M20,
            None,
            None,
            None,
        ),
    ]);
    bundle.observations[0].frequency_hz = Some(14_095_600);
    bundle.observations[0].reporter_call = Some("K1ABC".to_string());
    bundle.observations[0].heard_call = Some("N1RWJ".to_string());
    bundle.observations[0].distance_km = Some(150.0);
    bundle.observations[0].snr_db = Some(-12.5);
    bundle.observations[0].raw = json!({"payload": "unchanged", "sequence": 1});
    bundle.events = vec![
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
    ];
    let before = bundle.clone();

    let alignment = annotate_bundle_observations(&mut bundle);

    assert_eq!(bundle.manifest, before.manifest);
    assert_eq!(bundle.station, before.station);
    assert_eq!(bundle.antennas, before.antennas);
    assert_eq!(bundle.schedule, before.schedule);
    assert_eq!(bundle.events, before.events);
    assert_eq!(bundle.wsjtx, before.wsjtx);
    assert_eq!(bundle.rig, before.rig);
    assert_eq!(bundle.propagation, before.propagation);
    assert_eq!(bundle.analysis, before.analysis);
    assert_eq!(
        observations_without_alignment(&bundle.observations),
        observations_without_alignment(&before.observations)
    );

    insta::assert_json_snapshot!(
        json!({
            "returned_assignments": alignment.observation_assignments.iter().map(|assignment| {
                json!({
                    "observation_id": assignment.observation_id,
                    "slot_id": assignment.slot_id,
                    "slot_label": assignment.slot_label,
                    "confidence": snapshot_confidence(assignment.confidence),
                    "reason": assignment.reason,
                })
            }).collect::<Vec<_>>(),
            "mutated_annotations": annotation_snapshot(&bundle),
        }),
        @r###"
        {
          "mutated_annotations": [
            {
              "observation_id": "obs-preserved",
              "slot_confidence": 0.95,
              "slot_id": "slot-001",
              "slot_label": "A"
            },
            {
              "observation_id": "obs-preserved-too",
              "slot_confidence": 0.95,
              "slot_id": "slot-002",
              "slot_label": "B"
            }
          ],
          "returned_assignments": [
            {
              "confidence": 0.95,
              "observation_id": "obs-preserved",
              "reason": "interior",
              "slot_id": "slot-001",
              "slot_label": "A"
            },
            {
              "confidence": 0.95,
              "observation_id": "obs-preserved-too",
              "reason": "interior",
              "slot_id": "slot-002",
              "slot_label": "B"
            }
          ]
        }
        "###
    );
    validate_bundle(&bundle).unwrap();
}

#[test]
fn normalization_reuses_current_alignment_semantics() {
    let starts_at = fixture_start();
    let mut bundle = bundle_with_observations(vec![
        observation(
            "obs-outside",
            starts_at - chrono::Duration::seconds(5),
            Band::M20,
            Some("slot-001"),
            Some("A"),
            Some(0.95),
        ),
        observation(
            "obs-wrong-band",
            starts_at + chrono::Duration::seconds(70),
            Band::M40,
            Some("slot-001"),
            Some("A"),
            Some(0.95),
        ),
        observation(
            "obs-guard",
            starts_at + chrono::Duration::seconds(485),
            Band::M20,
            None,
            None,
            None,
        ),
        observation(
            "obs-bad",
            starts_at + chrono::Duration::seconds(180),
            Band::M20,
            Some("slot-002"),
            Some("B"),
            Some(0.95),
        ),
        observation(
            "obs-missed",
            starts_at + chrono::Duration::seconds(300),
            Band::M20,
            Some("slot-003"),
            Some("A"),
            Some(0.95),
        ),
        observation(
            "obs-before-late-switch",
            starts_at + chrono::Duration::seconds(370),
            Band::M20,
            Some("slot-004"),
            Some("B"),
            Some(0.95),
        ),
        observation(
            "obs-after-late-switch",
            starts_at + chrono::Duration::seconds(390),
            Band::M20,
            None,
            None,
            None,
        ),
    ]);
    bundle.events = vec![
        operator_event(
            "event-001",
            "slot-001",
            OperatorEventType::Switched,
            starts_at + chrono::Duration::seconds(3),
        ),
        operator_event(
            "event-002",
            "slot-002",
            OperatorEventType::BadSlot,
            starts_at + chrono::Duration::seconds(140),
        ),
        operator_event(
            "event-003",
            "slot-003",
            OperatorEventType::MissedSlot,
            starts_at + chrono::Duration::seconds(250),
        ),
        operator_event(
            "event-004",
            "slot-004",
            OperatorEventType::Switched,
            starts_at + chrono::Duration::seconds(385),
        ),
    ];

    let alignment = annotate_bundle_observations(&mut bundle);

    insta::assert_json_snapshot!(
        alignment.observation_assignments.iter().map(|assignment| {
            json!({
                "observation_id": assignment.observation_id,
                "slot_id": assignment.slot_id,
                "slot_label": assignment.slot_label,
                "confidence": snapshot_confidence(assignment.confidence),
                "reason": assignment.reason,
            })
        }).collect::<Vec<_>>(),
        @r###"
        [
          {
            "confidence": 0.0,
            "observation_id": "obs-outside",
            "reason": "outside_schedule",
            "slot_id": null,
            "slot_label": null
          },
          {
            "confidence": 0.0,
            "observation_id": "obs-wrong-band",
            "reason": "band_mismatch",
            "slot_id": null,
            "slot_label": null
          },
          {
            "confidence": 0.25,
            "observation_id": "obs-guard",
            "reason": "guard_time",
            "slot_id": "slot-005",
            "slot_label": null
          },
          {
            "confidence": 0.0,
            "observation_id": "obs-bad",
            "reason": "bad_slot",
            "slot_id": "slot-002",
            "slot_label": null
          },
          {
            "confidence": 0.0,
            "observation_id": "obs-missed",
            "reason": "missed_slot",
            "slot_id": "slot-003",
            "slot_label": null
          },
          {
            "confidence": 0.1,
            "observation_id": "obs-before-late-switch",
            "reason": "before_observed_switch",
            "slot_id": "slot-004",
            "slot_label": null
          },
          {
            "confidence": 0.7,
            "observation_id": "obs-after-late-switch",
            "reason": "late_switch",
            "slot_id": "slot-004",
            "slot_label": "B"
          }
        ]
        "###
    );
    insta::assert_json_snapshot!(
        annotation_snapshot(&bundle),
        @r###"
        [
          {
            "observation_id": "obs-outside",
            "slot_confidence": 0.0,
            "slot_id": null,
            "slot_label": null
          },
          {
            "observation_id": "obs-wrong-band",
            "slot_confidence": 0.0,
            "slot_id": null,
            "slot_label": null
          },
          {
            "observation_id": "obs-guard",
            "slot_confidence": 0.25,
            "slot_id": "slot-005",
            "slot_label": null
          },
          {
            "observation_id": "obs-bad",
            "slot_confidence": 0.0,
            "slot_id": "slot-002",
            "slot_label": null
          },
          {
            "observation_id": "obs-missed",
            "slot_confidence": 0.0,
            "slot_id": "slot-003",
            "slot_label": null
          },
          {
            "observation_id": "obs-before-late-switch",
            "slot_confidence": 0.1,
            "slot_id": "slot-004",
            "slot_label": null
          },
          {
            "observation_id": "obs-after-late-switch",
            "slot_confidence": 0.7,
            "slot_id": "slot-004",
            "slot_label": "B"
          }
        ]
        "###
    );
    validate_bundle(&bundle).unwrap();
}

fn bundle_with_observations(observations: Vec<ObservationRecord>) -> BundleContents {
    let starts_at = fixture_start();

    BundleContents {
        manifest: BundleManifest {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            created_at: starts_at - chrono::Duration::seconds(120),
            app_version: "0.1.0".to_string(),
            files: BundleFiles::default(),
        },
        station: Station {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            callsign: "N1RWJ".to_string(),
            grid: "FN42".to_string(),
            power_watts: Some(5.0),
            operator_notes: Some("fixture station details".to_string()),
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
                planned_slot(
                    "slot-003",
                    3,
                    starts_at + chrono::Duration::seconds(240),
                    "A",
                ),
                planned_slot(
                    "slot-004",
                    4,
                    starts_at + chrono::Duration::seconds(360),
                    "B",
                ),
                planned_slot(
                    "slot-005",
                    5,
                    starts_at + chrono::Duration::seconds(480),
                    "A",
                ),
            ],
        },
        events: Vec::new(),
        observations,
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
            notes: vec!["not yet analyzed".to_string()],
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
        actual_antenna_label: None,
    }
}

fn observation(
    observation_id: &str,
    timestamp: chrono::DateTime<Utc>,
    band: Band,
    slot_id: Option<&str>,
    slot_label: Option<&str>,
    slot_confidence: Option<f32>,
) -> ObservationRecord {
    ObservationRecord {
        meta: record_meta(timestamp, RecordSource::Wsprnet),
        observation_id: observation_id.to_string(),
        observation_kind: ObservationKind::PublicReport,
        band,
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
        raw: json!({"observation_id": observation_id}),
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

fn observations_without_alignment(observations: &[ObservationRecord]) -> Vec<ObservationRecord> {
    observations
        .iter()
        .cloned()
        .map(|mut observation| {
            observation.slot_id = None;
            observation.slot_label = None;
            observation.slot_confidence = None;
            observation
        })
        .collect()
}

fn annotation_snapshot(bundle: &BundleContents) -> Vec<serde_json::Value> {
    bundle
        .observations
        .iter()
        .map(|observation| {
            json!({
                "observation_id": observation.observation_id,
                "slot_id": observation.slot_id,
                "slot_label": observation.slot_label,
                "slot_confidence": observation.slot_confidence.map(snapshot_confidence),
            })
        })
        .collect()
}

fn snapshot_confidence(confidence: f32) -> f64 {
    (f64::from(confidence) * 100.0).round() / 100.0
}

fn fixture_start() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap()
}
