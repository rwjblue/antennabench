//! WSPR timing, readiness, occupancy, intention, and signal-evidence projection rules.

use antennabench_core::{
    v2::{CorrectableOperatorEventPayloadV2, SessionLifecycleV2},
    v3::{CorrectableOperatorEventPayloadV3, WsprCycleDirection},
};
use antennabench_wsjtx::WSPR_LIVE_INGESTION_GRACE_SECONDS;
use chrono::{DateTime, Duration, Utc};

use super::{
    transition::TransitionPlan, ConductorPhase, ConductorSlotView, SlotEvidenceStatus,
    TransitionDisposition,
};

pub(super) fn slot_evidence(
    facts: &[&CorrectableOperatorEventPayloadV2],
) -> (SlotEvidenceStatus, Option<String>) {
    let mut confirmed = Vec::<&str>::new();
    let mut missed = false;
    let mut bad = false;
    for fact in facts {
        match fact {
            CorrectableOperatorEventPayloadV2::AntennaStateConfirmed { antenna_label, .. } => {
                confirmed.push(antenna_label)
            }
            CorrectableOperatorEventPayloadV2::SlotMissed { .. } => missed = true,
            CorrectableOperatorEventPayloadV2::SlotBad { .. } => bad = true,
            CorrectableOperatorEventPayloadV2::NoteAdded { .. } => {}
        }
    }
    confirmed.sort_unstable();
    confirmed.dedup();
    let fact_kinds = usize::from(!confirmed.is_empty()) + usize::from(missed) + usize::from(bad);
    if confirmed.len() > 1 || fact_kinds > 1 {
        (SlotEvidenceStatus::Conflicting, None)
    } else if let Some(label) = confirmed.first() {
        (SlotEvidenceStatus::Confirmed, Some((*label).to_string()))
    } else if missed {
        (SlotEvidenceStatus::Missed, None)
    } else if bad {
        (SlotEvidenceStatus::Bad, None)
    } else {
        (SlotEvidenceStatus::Unknown, None)
    }
}

pub(super) fn slot_evidence_v3(
    facts: &[&CorrectableOperatorEventPayloadV3],
) -> (SlotEvidenceStatus, Option<String>) {
    let mut confirmed = Vec::<&str>::new();
    let mut missed = false;
    let mut bad = false;
    for fact in facts {
        match fact {
            CorrectableOperatorEventPayloadV3::AntennaStateConfirmed { antenna_label, .. } => {
                confirmed.push(antenna_label)
            }
            CorrectableOperatorEventPayloadV3::SlotMissed { .. } => missed = true,
            CorrectableOperatorEventPayloadV3::SlotBad { .. } => bad = true,
            CorrectableOperatorEventPayloadV3::SignalStateConfirmed { .. }
            | CorrectableOperatorEventPayloadV3::NoteAdded { .. } => {}
        }
    }
    confirmed.sort_unstable();
    confirmed.dedup();
    let fact_kinds = usize::from(!confirmed.is_empty()) + usize::from(missed) + usize::from(bad);
    if confirmed.len() > 1 || fact_kinds > 1 {
        (SlotEvidenceStatus::Conflicting, None)
    } else if let Some(label) = confirmed.first() {
        (SlotEvidenceStatus::Confirmed, Some((*label).to_string()))
    } else if missed {
        (SlotEvidenceStatus::Missed, None)
    } else if bad {
        (SlotEvidenceStatus::Bad, None)
    } else {
        (SlotEvidenceStatus::Unknown, None)
    }
}

pub(super) fn timing_projection(
    lifecycle: SessionLifecycleV2,
    wspr_live_acquisition_enabled: bool,
    slots: &[ConductorSlotView],
    now: DateTime<Utc>,
) -> (
    ConductorPhase,
    String,
    Option<i64>,
    Option<usize>,
    Option<usize>,
) {
    match lifecycle {
        SessionLifecycleV2::Ready => {
            return (
                ConductorPhase::Ready,
                "Start the session when the station is ready. No rig or network is required."
                    .into(),
                None,
                None,
                slots.first().map(|_| 0),
            )
        }
        SessionLifecycleV2::Interrupted => {
            return (
                ConductorPhase::Interrupted,
                "The durable session is interrupted. Review recovery details, then resume or end."
                    .into(),
                None,
                None,
                slots.iter().position(|slot| slot.starts_at > now),
            )
        }
        SessionLifecycleV2::Ended => {
            return (
                ConductorPhase::Ended,
                "The session is ended and no further evidence can be appended.".into(),
                None,
                None,
                None,
            )
        }
        SessionLifecycleV2::Abandoned => {
            return (
                ConductorPhase::Abandoned,
                "The session is abandoned and its existing evidence remains preserved.".into(),
                None,
                None,
                None,
            )
        }
        SessionLifecycleV2::Draft => {
            return (
                ConductorPhase::Ready,
                "Complete and validate the plan before starting this session.".into(),
                None,
                None,
                slots.first().map(|_| 0),
            )
        }
        SessionLifecycleV2::Running => {}
    }

    if let Some(index) = slots
        .iter()
        .position(|slot| now >= slot.starts_at && now < slot.ends_at)
    {
        let slot = &slots[index];
        let next = (index + 1 < slots.len()).then_some(index + 1);
        if now < slot.usable_at {
            return (
                ConductorPhase::Guard,
                format!(
                    "Confirm the actual antenna for slot {}. Guard time remains before evidence is usable.",
                    slot.sequence_number
                ),
                Some((slot.usable_at - now).num_seconds().max(0)),
                Some(index),
                next,
            );
        }
        return (
            ConductorPhase::Active,
            format!(
                "Slot {} is active. Planned antenna: {}. Confirm the actual antenna explicitly.",
                slot.sequence_number, slot.planned_antenna
            ),
            Some((slot.ends_at - now).num_seconds().max(0)),
            Some(index),
            next,
        );
    }

    if let Some(next) = slots.iter().position(|slot| slot.starts_at > now) {
        let phase = if next == 0 {
            ConductorPhase::AwaitingSlot
        } else {
            ConductorPhase::BetweenSlots
        };
        return (
            phase,
            format!(
                "Prepare {} for slot {}. The planned label is guidance only until explicitly confirmed.",
                slots[next].planned_antenna, slots[next].sequence_number
            ),
            Some((slots[next].starts_at - now).num_seconds().max(0)),
            None,
            Some(next),
        );
    }

    if let Some(final_slot) = wspr_live_acquisition_enabled
        .then(|| slots.last())
        .flatten()
        .filter(|slot| slot.evidence_status == SlotEvidenceStatus::Confirmed)
    {
        let acquisition_at =
            final_slot.ends_at + Duration::seconds(WSPR_LIVE_INGESTION_GRACE_SECONDS);
        return (
            ConductorPhase::Finalizing,
            if now < acquisition_at {
                "The final antenna state is confirmed. Waiting for WSPR.live ingestion, then AntennaBench will capture cumulative public spots and end automatically."
                    .into()
            } else {
                "Final WSPR.live acquisition is due. AntennaBench will finish automatically, or show recovery actions if the source fails."
                    .into()
            },
            Some((acquisition_at - now).num_seconds().max(0)),
            None,
            None,
        );
    }

    (
        ConductorPhase::Complete,
        "All planned slot windows have elapsed. Confirm the final actual antenna to authorize public-spot finalization, or end explicitly without it."
            .into(),
        None,
        None,
        None,
    )
}

pub(super) fn timing_projection_v3(
    lifecycle: SessionLifecycleV2,
    wspr_live_acquisition_enabled: bool,
    slots: &[ConductorSlotView],
    next_intent: Option<&antennabench_core::v3::WsprCycleIntentV3>,
    transition: &TransitionPlan,
    switching: bool,
    now: DateTime<Utc>,
) -> (
    ConductorPhase,
    String,
    Option<i64>,
    Option<usize>,
    Option<usize>,
) {
    match lifecycle {
        SessionLifecycleV2::Ready | SessionLifecycleV2::Draft => {
            return (
                ConductorPhase::Ready,
                "Start when you are ready.".into(),
                None,
                None,
                None,
            );
        }
        SessionLifecycleV2::Interrupted => {
            return (
                ConductorPhase::Interrupted,
                "The session is interrupted and antenna occupancy is unknown. Resume, then confirm an antenna again."
                    .into(),
                None,
                None,
                None,
            );
        }
        SessionLifecycleV2::Ended => {
            return (
                ConductorPhase::Ended,
                "The session is ended and no further evidence can be appended.".into(),
                None,
                None,
                None,
            );
        }
        SessionLifecycleV2::Abandoned => {
            return (
                ConductorPhase::Abandoned,
                "The session is abandoned and its existing evidence remains preserved.".into(),
                None,
                None,
                None,
            );
        }
        SessionLifecycleV2::Running => {}
    }

    if let Some(index) = slots
        .iter()
        .position(|slot| now >= slot.starts_at && now < slot.ends_at)
    {
        let direction = slots[index].direction;
        let operation = match direction {
            Some(WsprCycleDirection::Receive) => "receiving",
            Some(WsprCycleDirection::Transmit) => "transmitting",
            None => "active",
        };
        return (
            ConductorPhase::Active,
            format!(
                "WSPR cycle {} is {operation} on {}. Keep {} connected until the WSPR period completes.",
                slots[index].sequence_number,
                slots[index].band,
                slots[index]
                    .actual_antenna
                    .as_deref()
                    .unwrap_or(&slots[index].planned_antenna)
            ),
            Some((slots[index].ends_at - now).num_seconds().max(0)),
            Some(index),
            None,
        );
    }

    if let Some(index) = slots.iter().position(|slot| slot.starts_at > now) {
        let operation = match slots[index].direction {
            Some(WsprCycleDirection::Receive) => "receive",
            Some(WsprCycleDirection::Transmit) => "transmit",
            None => "WSPR",
        };
        return (
            ConductorPhase::AwaitingSlot,
            format!(
                "{} is ready to {operation}. WSPR cycle {} starts at the next even-minute WSPR time.",
                slots[index]
                    .actual_antenna
                    .as_deref()
                    .unwrap_or(&slots[index].planned_antenna),
                slots[index].sequence_number
            ),
            Some((slots[index].starts_at - now).num_seconds().max(0)),
            None,
            Some(index),
        );
    }

    let completed_index = slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.ends_at <= now)
        .max_by_key(|(_, slot)| slot.ends_at)
        .map(|(index, _)| index);
    if let Some(intent) = next_intent {
        if transition.can_continue() {
            return (
                ConductorPhase::BetweenSlots,
                format!(
                    "{} remains ready with unchanged WSPR settings. AntennaBench will continue automatically.",
                    intent.antenna_label
                ),
                transition
                    .prior_transmission_ends_at()
                    .map(|deadline| (deadline - now).num_seconds().max(0)),
                completed_index,
                None,
            );
        }

        let mut instructions = Vec::new();
        let antenna_instruction = match transition.view.antenna {
            TransitionDisposition::OperatorActionRequired
            | TransitionDisposition::UnknownBlocked => {
                Some(format!("Switch to {}", intent.antenna_label))
            }
            TransitionDisposition::AutomaticActionPending => Some(format!(
                "Waiting for automatic switch to {} and independent verification",
                intent.antenna_label
            )),
            TransitionDisposition::NoChangeNeeded
            | TransitionDisposition::AutomaticallyCompletedAndVerified => None,
        };
        let direction_instruction = match intent.direction {
            Some(WsprCycleDirection::Receive) => (
                "In WSJT-X, turn Enable Tx off and keep Monitor on",
                format!("Receive on {}", intent.antenna_label),
            ),
            Some(WsprCycleDirection::Transmit) => (
                "In WSJT-X, set Tx Pct to 100% and turn Enable Tx on",
                format!("Transmit on {}", intent.antenna_label),
            ),
            None => (
                "Prepare WSJT-X for the next WSPR period",
                intent.antenna_label.clone(),
            ),
        };
        if matches!(
            transition.view.direction,
            TransitionDisposition::OperatorActionRequired | TransitionDisposition::UnknownBlocked
        ) {
            instructions.push(direction_instruction.0.into());
        }
        if transition.view.band == TransitionDisposition::OperatorActionRequired {
            let band = serde_json::to_value(intent.band)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_else(|| format!("{:?}", intent.band));
            instructions.push(format!("Set the station to {band}"));
        }
        if transition.view.signal == TransitionDisposition::OperatorActionRequired {
            instructions.push("Prepare the next controlled-signal allocation".into());
        }
        if let Some(instruction) = antenna_instruction {
            instructions.push(instruction);
        }
        if transition.operator_action_required {
            instructions.push(format!("then click {} ready", direction_instruction.1));
        }
        let guidance = if instructions.is_empty() {
            "Waiting for the Rust-owned transition coordinator.".into()
        } else {
            format!(
                "{}.",
                instructions
                    .join(". ")
                    .replace(". then click", ", then click")
            )
        };
        return (
            if switching {
                ConductorPhase::Switching
            } else {
                ConductorPhase::BetweenSlots
            },
            guidance,
            None,
            completed_index,
            None,
        );
    }

    if let Some(final_slot) = wspr_live_acquisition_enabled
        .then(|| slots.last())
        .flatten()
    {
        let acquisition_at =
            final_slot.ends_at + Duration::seconds(WSPR_LIVE_INGESTION_GRACE_SECONDS);
        return (
            ConductorPhase::Finalizing,
            if now < acquisition_at {
                "All intended cycles are complete. Waiting briefly for WSPR.live ingestion before final capture."
                    .into()
            } else {
                "Final WSPR.live acquisition is due. AntennaBench will finish automatically or show a retry action."
                    .into()
            },
            Some((acquisition_at - now).num_seconds().max(0)),
            completed_index,
            None,
        );
    }

    (
        ConductorPhase::Complete,
        "All intended WSPR cycles are complete. End the session when you are satisfied with the evidence."
            .into(),
        None,
        completed_index,
        None,
    )
}
