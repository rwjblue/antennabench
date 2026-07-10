use antennabench_core::{
    align_schedule_slots, Band, ExperimentMode, OperatorEvent, OperatorEventType, PlannedSlot,
    RecordMeta, RecordSource, Schedule, SessionGoal, SlotAlignmentPolicy,
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
