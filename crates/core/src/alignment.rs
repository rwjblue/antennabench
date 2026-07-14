use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::{Band, ObservationRecord, OperatorEvent, OperatorEventType, PlannedSlot, Schedule};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotAlignmentPolicy {
    pub boundary_seconds: i64,
}

impl Default for SlotAlignmentPolicy {
    fn default() -> Self {
        Self {
            boundary_seconds: 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleSlotAlignment {
    pub slots: Vec<AlignedSlot>,
    pub observation_assignments: Vec<ObservationSlotAssignment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignedSlot {
    pub slot_id: String,
    pub sequence_number: u32,
    pub band: Band,
    pub planned_label: String,
    pub actual_label: Option<String>,
    pub status: AlignedSlotStatus,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub usable_start: DateTime<Utc>,
    pub switch_event_id: Option<String>,
    pub switch_timestamp: Option<DateTime<Utc>>,
    pub switch_delay_seconds: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignedSlotStatus {
    PlannedNoSwitchEvent,
    UnknownActualState,
    Switched,
    LateSwitch,
    Missed,
    Bad,
    ConflictingEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationSlotAssignment {
    pub observation_id: String,
    pub slot_id: Option<String>,
    pub slot_label: Option<String>,
    pub confidence: f32,
    pub reason: SlotAssignmentReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotAssignmentReason {
    Interior,
    GuardTime,
    NearBoundary,
    LateSwitch,
    BeforeObservedSwitch,
    MissedSlot,
    BadSlot,
    UnknownActualState,
    ConflictingEvidence,
    BandMismatch,
    OutsideSchedule,
}

pub fn align_schedule_slots(
    schedule: &Schedule,
    events: &[OperatorEvent],
    observations: &[ObservationRecord],
    policy: SlotAlignmentPolicy,
) -> ScheduleSlotAlignment {
    let slots = schedule
        .slots
        .iter()
        .map(|slot| {
            align_slot(
                slot,
                events,
                schedule.schema_version >= crate::SCHEMA_VERSION_V2,
            )
        })
        .collect::<Vec<_>>();
    let observation_assignments = observations
        .iter()
        .map(|observation| assign_observation_to_slot(observation, &slots, policy))
        .collect();

    ScheduleSlotAlignment {
        slots,
        observation_assignments,
    }
}

pub fn apply_slot_assignments(
    observations: &[ObservationRecord],
    assignments: &[ObservationSlotAssignment],
) -> Vec<ObservationRecord> {
    observations
        .iter()
        .map(|observation| {
            let mut annotated = observation.clone();
            if let Some(assignment) = assignments
                .iter()
                .find(|assignment| assignment.observation_id == observation.observation_id)
            {
                annotated.slot_id = assignment.slot_id.clone();
                annotated.slot_label = assignment.slot_label.clone();
                annotated.slot_confidence = Some(assignment.confidence);
            }
            annotated
        })
        .collect()
}

fn align_slot(slot: &PlannedSlot, events: &[OperatorEvent], schema_v2: bool) -> AlignedSlot {
    let slot_events = events
        .iter()
        .filter(|event| event.slot_id.as_deref() == Some(slot.slot_id.as_str()))
        .collect::<Vec<_>>();
    let switch_events = slot_events
        .iter()
        .copied()
        .filter(|event| event.event_type == OperatorEventType::Switched)
        .collect::<Vec<_>>();
    let bad_events = slot_events
        .iter()
        .filter(|event| event.event_type == OperatorEventType::BadSlot)
        .count();
    let missed_events = slot_events
        .iter()
        .filter(|event| event.event_type == OperatorEventType::MissedSlot)
        .count();
    let guard_end = slot.starts_at + Duration::seconds(slot.guard_seconds.into());
    let ends_at = slot.starts_at + Duration::seconds(slot.duration_seconds.into());
    let active_fact_count = switch_events.len() + bad_events + missed_events;
    if schema_v2 && active_fact_count > 1 {
        return AlignedSlot {
            slot_id: slot.slot_id.clone(),
            sequence_number: slot.sequence_number,
            band: slot.band,
            planned_label: slot.antenna_label.clone(),
            actual_label: None,
            status: AlignedSlotStatus::ConflictingEvidence,
            starts_at: slot.starts_at,
            ends_at,
            usable_start: guard_end,
            switch_event_id: None,
            switch_timestamp: None,
            switch_delay_seconds: None,
        };
    }

    if bad_events > 0 {
        return AlignedSlot {
            slot_id: slot.slot_id.clone(),
            sequence_number: slot.sequence_number,
            band: slot.band,
            planned_label: slot.antenna_label.clone(),
            actual_label: None,
            status: AlignedSlotStatus::Bad,
            starts_at: slot.starts_at,
            ends_at,
            usable_start: guard_end,
            switch_event_id: None,
            switch_timestamp: None,
            switch_delay_seconds: None,
        };
    }

    if missed_events > 0 {
        return AlignedSlot {
            slot_id: slot.slot_id.clone(),
            sequence_number: slot.sequence_number,
            band: slot.band,
            planned_label: slot.antenna_label.clone(),
            actual_label: None,
            status: AlignedSlotStatus::Missed,
            starts_at: slot.starts_at,
            ends_at,
            usable_start: guard_end,
            switch_event_id: None,
            switch_timestamp: None,
            switch_delay_seconds: None,
        };
    }

    match switch_events
        .into_iter()
        .min_by_key(|event| event.meta.timestamp)
    {
        Some(event) => {
            let status = if event.meta.timestamp <= guard_end {
                AlignedSlotStatus::Switched
            } else {
                AlignedSlotStatus::LateSwitch
            };
            AlignedSlot {
                slot_id: slot.slot_id.clone(),
                sequence_number: slot.sequence_number,
                band: slot.band,
                planned_label: slot.antenna_label.clone(),
                actual_label: event
                    .actual_antenna_label
                    .clone()
                    .or_else(|| (!schema_v2).then(|| slot.antenna_label.clone())),
                status,
                starts_at: slot.starts_at,
                ends_at,
                usable_start: guard_end.max(event.meta.timestamp),
                switch_event_id: Some(event.event_id.clone()),
                switch_timestamp: Some(event.meta.timestamp),
                switch_delay_seconds: Some(switch_delay_seconds(slot, event)),
            }
        }
        None if schema_v2 => AlignedSlot {
            slot_id: slot.slot_id.clone(),
            sequence_number: slot.sequence_number,
            band: slot.band,
            planned_label: slot.antenna_label.clone(),
            actual_label: None,
            status: AlignedSlotStatus::UnknownActualState,
            starts_at: slot.starts_at,
            ends_at,
            usable_start: guard_end,
            switch_event_id: None,
            switch_timestamp: None,
            switch_delay_seconds: None,
        },
        None => AlignedSlot {
            slot_id: slot.slot_id.clone(),
            sequence_number: slot.sequence_number,
            band: slot.band,
            planned_label: slot.antenna_label.clone(),
            actual_label: Some(slot.antenna_label.clone()),
            status: AlignedSlotStatus::PlannedNoSwitchEvent,
            starts_at: slot.starts_at,
            ends_at,
            usable_start: guard_end,
            switch_event_id: None,
            switch_timestamp: None,
            switch_delay_seconds: None,
        },
    }
}

fn switch_delay_seconds(slot: &PlannedSlot, event: &OperatorEvent) -> i64 {
    (event.meta.timestamp - slot.starts_at).num_seconds()
}

fn assign_observation_to_slot(
    observation: &ObservationRecord,
    slots: &[AlignedSlot],
    policy: SlotAlignmentPolicy,
) -> ObservationSlotAssignment {
    let timestamp = observation.meta.timestamp;
    let Some(slot) = slots
        .iter()
        .find(|slot| timestamp >= slot.starts_at && timestamp < slot.ends_at)
    else {
        return assignment(
            observation,
            None,
            None,
            0.0,
            SlotAssignmentReason::OutsideSchedule,
        );
    };

    if observation.band != slot.band {
        return assignment(
            observation,
            None,
            None,
            0.0,
            SlotAssignmentReason::BandMismatch,
        );
    }

    match slot.status {
        AlignedSlotStatus::ConflictingEvidence => {
            return assignment(
                observation,
                Some(slot),
                None,
                0.0,
                SlotAssignmentReason::ConflictingEvidence,
            );
        }
        AlignedSlotStatus::UnknownActualState => {
            return assignment(
                observation,
                Some(slot),
                None,
                0.0,
                SlotAssignmentReason::UnknownActualState,
            );
        }
        AlignedSlotStatus::Bad => {
            return assignment(
                observation,
                Some(slot),
                None,
                0.0,
                SlotAssignmentReason::BadSlot,
            );
        }
        AlignedSlotStatus::Missed => {
            return assignment(
                observation,
                Some(slot),
                None,
                0.0,
                SlotAssignmentReason::MissedSlot,
            );
        }
        AlignedSlotStatus::LateSwitch if timestamp < slot.usable_start => {
            return assignment(
                observation,
                Some(slot),
                None,
                0.10,
                SlotAssignmentReason::BeforeObservedSwitch,
            );
        }
        _ => {}
    }

    if timestamp < slot.usable_start {
        let label = if slot
            .switch_timestamp
            .is_some_and(|switch_timestamp| switch_timestamp <= timestamp)
        {
            slot.actual_label.as_deref()
        } else {
            None
        };
        return assignment(
            observation,
            Some(slot),
            label,
            0.25,
            SlotAssignmentReason::GuardTime,
        );
    }

    if (slot.ends_at - timestamp).num_seconds() <= policy.boundary_seconds {
        return assignment(
            observation,
            Some(slot),
            slot.actual_label.as_deref(),
            0.60,
            SlotAssignmentReason::NearBoundary,
        );
    }

    match slot.status {
        AlignedSlotStatus::LateSwitch => assignment(
            observation,
            Some(slot),
            slot.actual_label.as_deref(),
            0.70,
            SlotAssignmentReason::LateSwitch,
        ),
        AlignedSlotStatus::PlannedNoSwitchEvent => assignment(
            observation,
            Some(slot),
            slot.actual_label.as_deref(),
            0.80,
            SlotAssignmentReason::Interior,
        ),
        AlignedSlotStatus::Switched => assignment(
            observation,
            Some(slot),
            slot.actual_label.as_deref(),
            0.95,
            SlotAssignmentReason::Interior,
        ),
        AlignedSlotStatus::Bad
        | AlignedSlotStatus::Missed
        | AlignedSlotStatus::UnknownActualState
        | AlignedSlotStatus::ConflictingEvidence => unreachable!("handled above"),
    }
}

fn assignment(
    observation: &ObservationRecord,
    slot: Option<&AlignedSlot>,
    slot_label: Option<&str>,
    confidence: f32,
    reason: SlotAssignmentReason,
) -> ObservationSlotAssignment {
    ObservationSlotAssignment {
        observation_id: observation.observation_id.clone(),
        slot_id: slot.map(|slot| slot.slot_id.clone()),
        slot_label: slot_label.map(str::to_string),
        confidence,
        reason,
    }
}
