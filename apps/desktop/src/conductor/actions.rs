//! Typed operator-action validation and explicit schema-v2/v3/v5 event translation.

use antennabench_core::{
    next_wspr_cycle_after_ready,
    v2::{
        CorrectableOperatorEventPayloadV2, EventCorrectionActionV2, EventTimeBasisV2,
        MutationMember, OperatorEventPayloadV2, OperatorEventV2, Provenance, RecordMetaV2,
        ReplacementOperatorEventV2,
    },
    v3::{
        CorrectableOperatorEventPayloadV3, EventCorrectionActionV3, OperatorEventPayloadV3,
        OperatorEventV3, RecordMetaV3, ReplacementOperatorEventV3, SignalModeV3,
        SignalStateConfirmationV3,
    },
    v5::WsprReadinessBasisV5,
    RecordSource, SCHEMA_VERSION_V2, SCHEMA_VERSION_V5,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

use crate::open_session::{SessionErrorKind, SessionErrorPayload};

use super::live_session::PendingAction;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConductorMutationRequest {
    pub(super) action_token: String,
    pub(super) expected_revision: u64,
    pub(super) action: ConductorAction,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(super) enum ConductorAction {
    Start {
        note: Option<String>,
    },
    Interrupt {
        reason: Option<String>,
    },
    Resume {
        note: Option<String>,
    },
    End {
        reason: Option<String>,
    },
    Abandon {
        reason: Option<String>,
    },
    ArmWsprCycle {
        intent_id: String,
        antenna_label: String,
    },
    SkipWsprCycle {
        intent_id: String,
        reason: Option<String>,
    },
    ConfirmAntenna {
        slot_id: String,
        antenna_label: String,
        note: Option<String>,
    },
    ConfirmSignal {
        slot_id: String,
        frequency_hz: Option<u64>,
        mode: Option<SignalModeV3>,
        power_watts: Option<f32>,
        transmitted_callsign: Option<String>,
        cadence_followed: Option<bool>,
        note: Option<String>,
    },
    MarkMissed {
        slot_id: String,
        reason: Option<String>,
    },
    MarkBad {
        slot_id: String,
        reason: String,
    },
    AddNote {
        slot_id: Option<String>,
        note: String,
    },
    RetractEvent {
        target_event_id: String,
        reason: String,
    },
    ReplaceEvent {
        target_event_id: String,
        slot_id: Option<String>,
        replacement: CorrectableAction,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(super) enum CorrectableAction {
    ConfirmAntenna {
        antenna_label: String,
        note: Option<String>,
    },
    ConfirmSignal {
        frequency_hz: Option<u64>,
        mode: Option<SignalModeV3>,
        power_watts: Option<f32>,
        transmitted_callsign: Option<String>,
        cadence_followed: Option<bool>,
        note: Option<String>,
    },
    MarkMissed {
        reason: Option<String>,
    },
    MarkBad {
        reason: String,
    },
    AddNote {
        note: String,
    },
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn required_text(value: String, field: &str) -> Result<String, SessionErrorPayload> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Complete the required conductor action field.",
            format!("{field} must not be empty"),
        ))
    } else {
        Ok(trimmed.to_string())
    }
}

fn correction_payload(
    action: CorrectableAction,
) -> Result<CorrectableOperatorEventPayloadV2, SessionErrorPayload> {
    Ok(match action {
        CorrectableAction::ConfirmAntenna {
            antenna_label,
            note,
        } => CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label: required_text(antenna_label, "antennaLabel")?,
            note: optional_text(note),
        },
        CorrectableAction::ConfirmSignal { .. } => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "Signal-state confirmation requires a schema-v3 session.",
                "schema-v2 events cannot represent actual signal state",
            ));
        }
        CorrectableAction::MarkMissed { reason } => CorrectableOperatorEventPayloadV2::SlotMissed {
            reason: optional_text(reason),
        },
        CorrectableAction::MarkBad { reason } => CorrectableOperatorEventPayloadV2::SlotBad {
            reason: required_text(reason, "reason")?,
        },
        CorrectableAction::AddNote { note } => CorrectableOperatorEventPayloadV2::NoteAdded {
            note: required_text(note, "note")?,
        },
    })
}

fn action_payload(
    action: ConductorAction,
    occurred_at: DateTime<Utc>,
) -> Result<(Option<String>, OperatorEventPayloadV2), SessionErrorPayload> {
    Ok(match action {
        ConductorAction::Start { note } => (
            None,
            OperatorEventPayloadV2::SessionStarted {
                note: optional_text(note),
            },
        ),
        ConductorAction::Interrupt { reason } => (
            None,
            OperatorEventPayloadV2::SessionInterrupted {
                reason: optional_text(reason),
            },
        ),
        ConductorAction::Resume { note } => (
            None,
            OperatorEventPayloadV2::SessionResumed {
                note: optional_text(note),
            },
        ),
        ConductorAction::End { reason } => (
            None,
            OperatorEventPayloadV2::SessionEnded {
                reason: optional_text(reason),
            },
        ),
        ConductorAction::Abandon { reason } => (
            None,
            OperatorEventPayloadV2::SessionAbandoned {
                reason: optional_text(reason),
            },
        ),
        ConductorAction::ArmWsprCycle { .. } | ConductorAction::SkipWsprCycle { .. } => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "Operator-paced WSPR actions require a current session.",
                "schema-v2 sessions do not support durable cycle intentions",
            ));
        }
        ConductorAction::ConfirmAntenna {
            slot_id,
            antenna_label,
            note,
        } => (
            Some(required_text(slot_id, "slotId")?),
            OperatorEventPayloadV2::AntennaStateConfirmed {
                antenna_label: required_text(antenna_label, "antennaLabel")?,
                note: optional_text(note),
            },
        ),
        ConductorAction::ConfirmSignal { .. } => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "Signal-state confirmation requires a schema-v3 session.",
                "schema-v2 events cannot represent actual signal state",
            ));
        }
        ConductorAction::MarkMissed { slot_id, reason } => (
            Some(required_text(slot_id, "slotId")?),
            OperatorEventPayloadV2::SlotMissed {
                reason: optional_text(reason),
            },
        ),
        ConductorAction::MarkBad { slot_id, reason } => (
            Some(required_text(slot_id, "slotId")?),
            OperatorEventPayloadV2::SlotBad {
                reason: required_text(reason, "reason")?,
            },
        ),
        ConductorAction::AddNote { slot_id, note } => (
            optional_text(slot_id),
            OperatorEventPayloadV2::NoteAdded {
                note: required_text(note, "note")?,
            },
        ),
        ConductorAction::RetractEvent {
            target_event_id,
            reason,
        } => (
            None,
            OperatorEventPayloadV2::EventCorrected {
                target_event_id: required_text(target_event_id, "targetEventId")?,
                correction: EventCorrectionActionV2::Retract,
                reason: required_text(reason, "reason")?,
            },
        ),
        ConductorAction::ReplaceEvent {
            target_event_id,
            slot_id,
            replacement,
            reason,
        } => (
            None,
            OperatorEventPayloadV2::EventCorrected {
                target_event_id: required_text(target_event_id, "targetEventId")?,
                correction: EventCorrectionActionV2::Replace {
                    replacement: ReplacementOperatorEventV2 {
                        occurred_at,
                        time_basis: EventTimeBasisV2::ObservedNow,
                        uncertainty_seconds: None,
                        slot_id: optional_text(slot_id),
                        payload: correction_payload(replacement)?,
                    },
                },
                reason: required_text(reason, "reason")?,
            },
        ),
    })
}

pub(super) fn event_for_action(
    session_id: &str,
    pending: &PendingAction,
    action: ConductorAction,
) -> Result<OperatorEventV2, SessionErrorPayload> {
    let occurred_at = pending.occurred_at.ok_or_else(|| {
        SessionErrorPayload::report_pipeline("conductor action time was not initialized")
    })?;
    let (slot_id, payload) = action_payload(action, occurred_at)?;
    Ok(OperatorEventV2 {
        meta: RecordMetaV2 {
            schema_version: SCHEMA_VERSION_V2,
            session_id: session_id.to_string(),
            recorded_at: occurred_at,
            provenance: Provenance::from_legacy(RecordSource::Operator, env!("CARGO_PKG_VERSION")),
            mutation: MutationMember {
                mutation_id: pending.token.clone(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        event_id: format!("event-for-{}", pending.token),
        occurred_at,
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id,
        payload,
    })
}

fn signal_confirmation(
    frequency_hz: Option<u64>,
    mode: Option<SignalModeV3>,
    power_watts: Option<f32>,
    transmitted_callsign: Option<String>,
    cadence_followed: Option<bool>,
    note: Option<String>,
) -> SignalStateConfirmationV3 {
    SignalStateConfirmationV3 {
        frequency_hz,
        mode,
        power_watts,
        transmitted_callsign: optional_text(transmitted_callsign),
        cadence_followed,
        note: optional_text(note),
    }
}

fn correction_payload_v3(
    action: CorrectableAction,
) -> Result<CorrectableOperatorEventPayloadV3, SessionErrorPayload> {
    Ok(match action {
        CorrectableAction::ConfirmAntenna {
            antenna_label,
            note,
        } => CorrectableOperatorEventPayloadV3::AntennaStateConfirmed {
            antenna_label: required_text(antenna_label, "antennaLabel")?,
            note: optional_text(note),
        },
        CorrectableAction::ConfirmSignal {
            frequency_hz,
            mode,
            power_watts,
            transmitted_callsign,
            cadence_followed,
            note,
        } => CorrectableOperatorEventPayloadV3::SignalStateConfirmed {
            confirmation: signal_confirmation(
                frequency_hz,
                mode,
                power_watts,
                transmitted_callsign,
                cadence_followed,
                note,
            ),
        },
        CorrectableAction::MarkMissed { reason } => CorrectableOperatorEventPayloadV3::SlotMissed {
            reason: optional_text(reason),
        },
        CorrectableAction::MarkBad { reason } => CorrectableOperatorEventPayloadV3::SlotBad {
            reason: required_text(reason, "reason")?,
        },
        CorrectableAction::AddNote { note } => CorrectableOperatorEventPayloadV3::NoteAdded {
            note: required_text(note, "note")?,
        },
    })
}

fn action_payload_v3(
    action: ConductorAction,
    occurred_at: DateTime<Utc>,
    schema_version: u16,
) -> Result<(Option<String>, OperatorEventPayloadV3), SessionErrorPayload> {
    Ok(match action {
        ConductorAction::Start { note } => (
            None,
            OperatorEventPayloadV3::SessionStarted {
                note: optional_text(note),
            },
        ),
        ConductorAction::Interrupt { reason } => (
            None,
            OperatorEventPayloadV3::SessionInterrupted {
                reason: optional_text(reason),
            },
        ),
        ConductorAction::Resume { note } => (
            None,
            OperatorEventPayloadV3::SessionResumed {
                note: optional_text(note),
            },
        ),
        ConductorAction::End { reason } => (
            None,
            OperatorEventPayloadV3::SessionEnded {
                reason: optional_text(reason),
            },
        ),
        ConductorAction::Abandon { reason } => (
            None,
            OperatorEventPayloadV3::SessionAbandoned {
                reason: optional_text(reason),
            },
        ),
        ConductorAction::ArmWsprCycle {
            intent_id,
            antenna_label,
        } => {
            let cycle = next_wspr_cycle_after_ready(occurred_at, Duration::seconds(1)).map_err(
                |error| {
                    SessionErrorPayload::new(
                        SessionErrorKind::Validation,
                        "The next WSPR cycle could not be calculated.",
                        error.to_string(),
                    )
                },
            )?;
            (
                Some(required_text(intent_id, "intentId")?),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label: required_text(antenna_label, "antennaLabel")?,
                    cycle_starts_at: cycle.starts_at,
                    readiness: (schema_version >= SCHEMA_VERSION_V5)
                        .then_some(WsprReadinessBasisV5::OperatorConfirmed),
                },
            )
        }
        ConductorAction::SkipWsprCycle { intent_id, reason } => (
            Some(required_text(intent_id, "intentId")?),
            OperatorEventPayloadV3::SlotMissed {
                reason: optional_text(reason),
            },
        ),
        ConductorAction::ConfirmAntenna {
            slot_id,
            antenna_label,
            note,
        } => (
            Some(required_text(slot_id, "slotId")?),
            OperatorEventPayloadV3::AntennaStateConfirmed {
                antenna_label: required_text(antenna_label, "antennaLabel")?,
                note: optional_text(note),
            },
        ),
        ConductorAction::ConfirmSignal {
            slot_id,
            frequency_hz,
            mode,
            power_watts,
            transmitted_callsign,
            cadence_followed,
            note,
        } => (
            Some(required_text(slot_id, "slotId")?),
            OperatorEventPayloadV3::SignalStateConfirmed {
                confirmation: signal_confirmation(
                    frequency_hz,
                    mode,
                    power_watts,
                    transmitted_callsign,
                    cadence_followed,
                    note,
                ),
            },
        ),
        ConductorAction::MarkMissed { slot_id, reason } => (
            Some(required_text(slot_id, "slotId")?),
            OperatorEventPayloadV3::SlotMissed {
                reason: optional_text(reason),
            },
        ),
        ConductorAction::MarkBad { slot_id, reason } => (
            Some(required_text(slot_id, "slotId")?),
            OperatorEventPayloadV3::SlotBad {
                reason: required_text(reason, "reason")?,
            },
        ),
        ConductorAction::AddNote { slot_id, note } => (
            optional_text(slot_id),
            OperatorEventPayloadV3::NoteAdded {
                note: required_text(note, "note")?,
            },
        ),
        ConductorAction::RetractEvent {
            target_event_id,
            reason,
        } => (
            None,
            OperatorEventPayloadV3::EventCorrected {
                target_event_id: required_text(target_event_id, "targetEventId")?,
                correction: EventCorrectionActionV3::Retract,
                reason: required_text(reason, "reason")?,
            },
        ),
        ConductorAction::ReplaceEvent {
            target_event_id,
            slot_id,
            replacement,
            reason,
        } => (
            None,
            OperatorEventPayloadV3::EventCorrected {
                target_event_id: required_text(target_event_id, "targetEventId")?,
                correction: EventCorrectionActionV3::Replace {
                    replacement: ReplacementOperatorEventV3 {
                        occurred_at,
                        time_basis: EventTimeBasisV2::ObservedNow,
                        uncertainty_seconds: None,
                        slot_id: optional_text(slot_id),
                        payload: correction_payload_v3(replacement)?,
                    },
                },
                reason: required_text(reason, "reason")?,
            },
        ),
    })
}

pub(super) fn event_for_action_v3(
    session_id: &str,
    schema_version: u16,
    pending: &PendingAction,
    action: ConductorAction,
) -> Result<OperatorEventV3, SessionErrorPayload> {
    let occurred_at = pending.occurred_at.ok_or_else(|| {
        SessionErrorPayload::report_pipeline("conductor action time was not initialized")
    })?;
    let (slot_id, payload) = action_payload_v3(action, occurred_at, schema_version)?;
    Ok(OperatorEventV3 {
        meta: RecordMetaV3 {
            schema_version,
            session_id: session_id.to_string(),
            recorded_at: occurred_at,
            provenance: Provenance::from_legacy(RecordSource::Operator, env!("CARGO_PKG_VERSION")),
            mutation: MutationMember {
                mutation_id: pending.token.clone(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        event_id: format!("event-for-{}", pending.token),
        occurred_at,
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id,
        payload,
    })
}
