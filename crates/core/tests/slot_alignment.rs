use antennabench_core::{
    align_schedule_slots, apply_slot_assignments, Band, ExperimentMode, ObservationKind,
    ObservationRecord, OperatorEvent, OperatorEventType, PlannedSlot, RecordMeta, RecordSource,
    Schedule, SessionGoal, SlotAlignmentPolicy,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

const SESSION_ID: &str = "session-alignment-test";

#[test]
fn derives_actual_slot_state_from_operator_events() {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();
    let schedule = schedule_with_slots(starts_at);
    let events = vec![
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
            "event-002-switch",
            "slot-002",
            OperatorEventType::Switched,
            starts_at + chrono::Duration::seconds(125),
        ),
        operator_event(
            "event-002-missed",
            "slot-002",
            OperatorEventType::MissedSlot,
            starts_at + chrono::Duration::seconds(130),
        ),
        operator_event(
            "event-003",
            "slot-003",
            OperatorEventType::MissedSlot,
            starts_at + chrono::Duration::seconds(240),
        ),
        operator_event(
            "event-003-switch",
            "slot-003",
            OperatorEventType::Switched,
            starts_at + chrono::Duration::seconds(243),
        ),
        operator_event(
            "event-004",
            "slot-004",
            OperatorEventType::Switched,
            starts_at + chrono::Duration::seconds(385),
        ),
    ];

    let result = align_schedule_slots(&schedule, &events, &[], SlotAlignmentPolicy::default());

    insta::assert_json_snapshot!(
        result.slots.iter().map(|slot| {
            json!({
                "slot_id": &slot.slot_id,
                "status": &slot.status,
                "planned_label": &slot.planned_label,
                "actual_label": &slot.actual_label,
                "switch_event_id": &slot.switch_event_id,
                "switch_delay_seconds": slot.switch_delay_seconds,
                "usable_start": slot.usable_start,
            })
        }).collect::<Vec<_>>(),
        @r###"
        [
          {
            "actual_label": "A",
            "planned_label": "A",
            "slot_id": "slot-001",
            "status": "switched",
            "switch_delay_seconds": 3,
            "switch_event_id": "event-001",
            "usable_start": "2026-07-10T20:00:15Z"
          },
          {
            "actual_label": null,
            "planned_label": "B",
            "slot_id": "slot-002",
            "status": "bad",
            "switch_delay_seconds": null,
            "switch_event_id": null,
            "usable_start": "2026-07-10T20:02:15Z"
          },
          {
            "actual_label": null,
            "planned_label": "A",
            "slot_id": "slot-003",
            "status": "missed",
            "switch_delay_seconds": null,
            "switch_event_id": null,
            "usable_start": "2026-07-10T20:04:15Z"
          },
          {
            "actual_label": "B",
            "planned_label": "B",
            "slot_id": "slot-004",
            "status": "late_switch",
            "switch_delay_seconds": 25,
            "switch_event_id": "event-004",
            "usable_start": "2026-07-10T20:06:25Z"
          },
          {
            "actual_label": "A",
            "planned_label": "A",
            "slot_id": "slot-005",
            "status": "planned_no_switch_event",
            "switch_delay_seconds": null,
            "switch_event_id": null,
            "usable_start": "2026-07-10T20:08:15Z"
          }
        ]
        "###
    );
}

#[test]
fn assigns_observations_with_guard_boundary_band_and_outside_reasons() {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();
    let schedule = schedule_with_slots(starts_at);
    let events = vec![
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
    let observations = vec![
        observation(
            "obs-good-a",
            starts_at + chrono::Duration::seconds(60),
            Band::M20,
        ),
        observation(
            "obs-boundary-a",
            starts_at + chrono::Duration::seconds(118),
            Band::M20,
        ),
        observation(
            "obs-guard-b",
            starts_at + chrono::Duration::seconds(125),
            Band::M20,
        ),
        observation(
            "obs-wrong-band",
            starts_at + chrono::Duration::seconds(70),
            Band::M40,
        ),
        observation(
            "obs-outside",
            starts_at - chrono::Duration::seconds(5),
            Band::M20,
        ),
    ];

    let result = align_schedule_slots(
        &schedule,
        &events,
        &observations,
        SlotAlignmentPolicy::default(),
    );

    insta::assert_json_snapshot!(
        result.observation_assignments.iter().map(|assignment| {
            json!({
                "observation_id": &assignment.observation_id,
                "slot_id": &assignment.slot_id,
                "slot_label": &assignment.slot_label,
                "confidence": snapshot_confidence(assignment.confidence),
                "reason": &assignment.reason,
            })
        }).collect::<Vec<_>>(),
        @r###"
        [
          {
            "confidence": 0.95,
            "observation_id": "obs-good-a",
            "reason": "interior",
            "slot_id": "slot-001",
            "slot_label": "A"
          },
          {
            "confidence": 0.6,
            "observation_id": "obs-boundary-a",
            "reason": "near_boundary",
            "slot_id": "slot-001",
            "slot_label": "A"
          },
          {
            "confidence": 0.25,
            "observation_id": "obs-guard-b",
            "reason": "guard_time",
            "slot_id": "slot-002",
            "slot_label": "B"
          },
          {
            "confidence": 0.0,
            "observation_id": "obs-wrong-band",
            "reason": "band_mismatch",
            "slot_id": null,
            "slot_label": null
          },
          {
            "confidence": 0.0,
            "observation_id": "obs-outside",
            "reason": "outside_schedule",
            "slot_id": null,
            "slot_label": null
          }
        ]
        "###
    );
}

#[test]
fn assigns_bad_missed_and_late_switch_observations_conservatively() {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();
    let schedule = schedule_with_slots(starts_at);
    let events = vec![
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
    let observations = vec![
        observation(
            "obs-bad",
            starts_at + chrono::Duration::seconds(180),
            Band::M20,
        ),
        observation(
            "obs-missed",
            starts_at + chrono::Duration::seconds(300),
            Band::M20,
        ),
        observation(
            "obs-before-late-switch",
            starts_at + chrono::Duration::seconds(370),
            Band::M20,
        ),
        observation(
            "obs-after-late-switch",
            starts_at + chrono::Duration::seconds(390),
            Band::M20,
        ),
    ];

    let result = align_schedule_slots(
        &schedule,
        &events,
        &observations,
        SlotAlignmentPolicy::default(),
    );

    insta::assert_json_snapshot!(
        result.observation_assignments.iter().map(|assignment| {
            json!({
                "observation_id": &assignment.observation_id,
                "slot_id": &assignment.slot_id,
                "slot_label": &assignment.slot_label,
                "confidence": snapshot_confidence(assignment.confidence),
                "reason": &assignment.reason,
            })
        }).collect::<Vec<_>>(),
        @r###"
        [
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
}

#[test]
fn applies_slot_assignments_to_observation_records_without_changing_raw_observations() {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();
    let schedule = schedule_with_slots(starts_at);
    let events = vec![operator_event(
        "event-001",
        "slot-001",
        OperatorEventType::Switched,
        starts_at + chrono::Duration::seconds(3),
    )];
    let observations = vec![observation(
        "obs-good-a",
        starts_at + chrono::Duration::seconds(60),
        Band::M20,
    )];

    let result = align_schedule_slots(
        &schedule,
        &events,
        &observations,
        SlotAlignmentPolicy::default(),
    );
    let annotated = apply_slot_assignments(&observations, &result.observation_assignments);

    assert_eq!(observations[0].slot_id, None);
    assert_eq!(observations[0].slot_label, None);
    assert_eq!(observations[0].slot_confidence, None);

    assert_eq!(annotated[0].slot_id, Some("slot-001".to_string()));
    assert_eq!(annotated[0].slot_label, Some("A".to_string()));
    assert_eq!(annotated[0].slot_confidence, Some(0.95));
    assert_eq!(annotated[0].raw, observations[0].raw);
    assert_eq!(annotated[0].snr_db, observations[0].snr_db);
}

fn schedule_with_slots(starts_at: chrono::DateTime<Utc>) -> Schedule {
    Schedule {
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
        meta: RecordMeta {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            timestamp,
            source: RecordSource::Operator,
        },
        event_id: event_id.to_string(),
        slot_id: Some(slot_id.to_string()),
        event_type,
        note: None,
    }
}

fn observation(
    observation_id: &str,
    timestamp: chrono::DateTime<Utc>,
    band: Band,
) -> ObservationRecord {
    ObservationRecord {
        meta: RecordMeta {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            timestamp,
            source: RecordSource::Wsprnet,
        },
        observation_id: observation_id.to_string(),
        observation_kind: ObservationKind::PublicReport,
        band,
        frequency_hz: None,
        mode: Some("WSPR".to_string()),
        reporter_call: None,
        heard_call: None,
        reporter_grid: None,
        heard_grid: None,
        distance_km: None,
        azimuth_degrees: None,
        snr_db: None,
        drift_hz_per_minute: None,
        power_watts: None,
        slot_id: None,
        slot_label: None,
        slot_confidence: None,
        raw: json!({}),
    }
}

fn snapshot_confidence(confidence: f32) -> f64 {
    (f64::from(confidence) * 100.0).round() / 100.0
}
