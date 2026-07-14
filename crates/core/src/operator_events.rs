use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::{
    CorrectableOperatorEventPayloadV2, EventCorrectionActionV2, EventTimeBasisV2, OperatorEvent,
    OperatorEventPayloadV2, OperatorEventType, OperatorEventV2, RecordMeta, RecordSource,
    ReplacementOperatorEventV2, SessionLifecycleV2, SCHEMA_VERSION_V2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorEventDiagnosticCodeV2 {
    DuplicateEventId,
    InvalidLifecycleTransition,
    InvalidCorrectionTarget,
    InvalidCorrectionReason,
    InvalidEventShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorEventDiagnosticV2 {
    pub event_id: String,
    pub related_event_id: Option<String>,
    pub code: OperatorEventDiagnosticCodeV2,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveOperatorEventV2 {
    pub session_id: String,
    pub source_event_id: String,
    pub effective_through_event_id: String,
    pub recorded_at: DateTime<Utc>,
    pub occurred_at: DateTime<Utc>,
    pub time_basis: EventTimeBasisV2,
    pub uncertainty_seconds: Option<u32>,
    pub slot_id: Option<String>,
    pub payload: CorrectableOperatorEventPayloadV2,
}

impl EffectiveOperatorEventV2 {
    pub(crate) fn project_legacy(self) -> Option<OperatorEvent> {
        let (event_type, note, actual_antenna_label) = match self.payload {
            CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
                antenna_label,
                note,
            } => (OperatorEventType::Switched, note, Some(antenna_label)),
            CorrectableOperatorEventPayloadV2::SlotMissed { reason } => {
                (OperatorEventType::MissedSlot, reason, None)
            }
            CorrectableOperatorEventPayloadV2::SlotBad { reason } => (
                OperatorEventType::BadSlot,
                (!reason.is_empty()).then_some(reason),
                None,
            ),
            CorrectableOperatorEventPayloadV2::NoteAdded { note } => (
                OperatorEventType::NoteAdded,
                (!note.is_empty()).then_some(note),
                None,
            ),
        };
        Some(OperatorEvent {
            meta: RecordMeta {
                schema_version: SCHEMA_VERSION_V2,
                session_id: self.session_id,
                timestamp: self.occurred_at,
                source: RecordSource::Operator,
            },
            event_id: self.source_event_id,
            slot_id: self.slot_id,
            event_type,
            note,
            actual_antenna_label,
        })
    }
}

impl OperatorEventV2 {
    pub(crate) fn project_legacy_lifecycle(&self) -> Option<OperatorEvent> {
        let (event_type, note) = match &self.payload {
            OperatorEventPayloadV2::SessionStarted { note } => {
                (OperatorEventType::SessionStarted, note.clone())
            }
            OperatorEventPayloadV2::SessionEnded { reason } => {
                (OperatorEventType::SessionEnded, reason.clone())
            }
            _ => return None,
        };
        Some(OperatorEvent {
            meta: RecordMeta {
                schema_version: SCHEMA_VERSION_V2,
                session_id: self.meta.session_id.clone(),
                timestamp: self.occurred_at,
                source: self.meta.provenance.legacy_source(),
            },
            event_id: self.event_id.clone(),
            slot_id: self.slot_id.clone(),
            event_type,
            note,
            actual_antenna_label: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperatorEventReductionV2 {
    pub lifecycle: SessionLifecycleV2,
    pub effective_events: Vec<EffectiveOperatorEventV2>,
    pub diagnostics: Vec<OperatorEventDiagnosticV2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LifecycleTransitionErrorV2 {
    #[error("expected checkpoint revision {expected}, but current revision is {actual}")]
    StaleRevision { expected: u64, actual: u64 },
    #[error("event is not a lifecycle transition")]
    NotLifecycle,
    #[error("invalid lifecycle transition from {from:?}")]
    InvalidTransition { from: SessionLifecycleV2 },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OperatorEventAppendErrorV2 {
    #[error("expected checkpoint revision {expected}, but current revision is {actual}")]
    StaleRevision { expected: u64, actual: u64 },
    #[error("operator event is invalid: {message}")]
    InvalidEvent { message: String },
}

pub fn validate_operator_event_append_v2(
    initial_lifecycle: SessionLifecycleV2,
    current_revision: u64,
    expected_revision: u64,
    existing: &[OperatorEventV2],
    proposed: &OperatorEventV2,
) -> Result<OperatorEventReductionV2, OperatorEventAppendErrorV2> {
    if current_revision != expected_revision {
        return Err(OperatorEventAppendErrorV2::StaleRevision {
            expected: expected_revision,
            actual: current_revision,
        });
    }
    if existing
        .first()
        .is_some_and(|event| event.meta.session_id != proposed.meta.session_id)
    {
        return Err(OperatorEventAppendErrorV2::InvalidEvent {
            message: "event belongs to a different session".into(),
        });
    }
    let baseline = reduce_operator_events_v2(initial_lifecycle, existing);
    let mut with_proposed = existing.to_vec();
    with_proposed.push(proposed.clone());
    let reduction = reduce_operator_events_v2(initial_lifecycle, &with_proposed);
    if reduction.diagnostics.len() != baseline.diagnostics.len() {
        let message = reduction
            .diagnostics
            .last()
            .map(|diagnostic| diagnostic.message.clone())
            .unwrap_or_else(|| "event did not produce a valid effective view".into());
        return Err(OperatorEventAppendErrorV2::InvalidEvent { message });
    }
    Ok(reduction)
}

pub fn validate_lifecycle_transition_v2(
    current: SessionLifecycleV2,
    current_revision: u64,
    expected_revision: u64,
    payload: &OperatorEventPayloadV2,
) -> Result<SessionLifecycleV2, LifecycleTransitionErrorV2> {
    if current_revision != expected_revision {
        return Err(LifecycleTransitionErrorV2::StaleRevision {
            expected: expected_revision,
            actual: current_revision,
        });
    }
    apply_lifecycle_transition(current, payload)
}

fn apply_lifecycle_transition(
    current: SessionLifecycleV2,
    payload: &OperatorEventPayloadV2,
) -> Result<SessionLifecycleV2, LifecycleTransitionErrorV2> {
    use OperatorEventPayloadV2 as Payload;
    use SessionLifecycleV2 as State;
    match (current, payload) {
        (State::Ready, Payload::SessionStarted { .. }) => Ok(State::Running),
        (
            State::Running,
            Payload::SessionInterrupted { .. } | Payload::InterruptionDetected { .. },
        ) => Ok(State::Interrupted),
        (State::Interrupted, Payload::SessionResumed { .. }) => Ok(State::Running),
        (State::Running | State::Interrupted, Payload::SessionEnded { .. }) => Ok(State::Ended),
        (
            State::Draft | State::Ready | State::Running | State::Interrupted,
            Payload::SessionAbandoned { .. },
        ) => Ok(State::Abandoned),
        (_, payload) if !is_lifecycle_payload(payload) => {
            Err(LifecycleTransitionErrorV2::NotLifecycle)
        }
        (from, _) => Err(LifecycleTransitionErrorV2::InvalidTransition { from }),
    }
}

pub fn reduce_operator_events_v2(
    initial_lifecycle: SessionLifecycleV2,
    events: &[OperatorEventV2],
) -> OperatorEventReductionV2 {
    let mut lifecycle = initial_lifecycle;
    let mut diagnostics = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut originals = HashMap::<String, usize>::new();
    let mut effective = Vec::<Option<EffectiveOperatorEventV2>>::new();

    for event in events {
        if !seen_ids.insert(event.event_id.as_str()) {
            diagnostics.push(diagnostic(
                event,
                None,
                OperatorEventDiagnosticCodeV2::DuplicateEventId,
                "event ID is duplicated",
            ));
            continue;
        }

        if let Some(message) = event_shape_error(event) {
            diagnostics.push(diagnostic(
                event,
                None,
                OperatorEventDiagnosticCodeV2::InvalidEventShape,
                message,
            ));
            continue;
        }

        if is_lifecycle_payload(&event.payload) {
            match apply_lifecycle_transition(lifecycle, &event.payload) {
                Ok(next) => lifecycle = next,
                Err(error) => diagnostics.push(diagnostic(
                    event,
                    None,
                    OperatorEventDiagnosticCodeV2::InvalidLifecycleTransition,
                    error.to_string(),
                )),
            }
            continue;
        }

        if matches!(
            lifecycle,
            SessionLifecycleV2::Ended | SessionLifecycleV2::Abandoned
        ) {
            diagnostics.push(diagnostic(
                event,
                None,
                OperatorEventDiagnosticCodeV2::InvalidLifecycleTransition,
                "operator evidence cannot be appended after a terminal lifecycle event",
            ));
            continue;
        }

        match &event.payload {
            OperatorEventPayloadV2::EventCorrected {
                target_event_id,
                correction,
                reason,
            } => {
                if reason.trim().is_empty() {
                    diagnostics.push(diagnostic(
                        event,
                        Some(target_event_id),
                        OperatorEventDiagnosticCodeV2::InvalidCorrectionReason,
                        "correction reason must not be empty",
                    ));
                    continue;
                }
                let Some(&target_index) = originals.get(target_event_id) else {
                    diagnostics.push(diagnostic(
                        event,
                        Some(target_event_id),
                        OperatorEventDiagnosticCodeV2::InvalidCorrectionTarget,
                        "correction target must be an earlier correctable event",
                    ));
                    continue;
                };
                match correction {
                    EventCorrectionActionV2::Retract => effective[target_index] = None,
                    EventCorrectionActionV2::Replace { replacement } => {
                        effective[target_index] = Some(effective_from_replacement(
                            target_event_id,
                            event,
                            replacement,
                        ));
                    }
                }
            }
            payload => {
                let Some(payload) = correctable_payload(payload) else {
                    diagnostics.push(diagnostic(
                        event,
                        None,
                        OperatorEventDiagnosticCodeV2::InvalidEventShape,
                        "event payload is neither lifecycle evidence nor correctable operator evidence",
                    ));
                    continue;
                };
                let index = effective.len();
                originals.insert(event.event_id.clone(), index);
                effective.push(Some(EffectiveOperatorEventV2 {
                    session_id: event.meta.session_id.clone(),
                    source_event_id: event.event_id.clone(),
                    effective_through_event_id: event.event_id.clone(),
                    recorded_at: event.meta.recorded_at,
                    occurred_at: event.occurred_at,
                    time_basis: event.time_basis,
                    uncertainty_seconds: event.uncertainty_seconds,
                    slot_id: event.slot_id.clone(),
                    payload,
                }));
            }
        }
    }

    OperatorEventReductionV2 {
        lifecycle,
        effective_events: effective.into_iter().flatten().collect(),
        diagnostics,
    }
}

fn is_lifecycle_payload(payload: &OperatorEventPayloadV2) -> bool {
    matches!(
        payload,
        OperatorEventPayloadV2::SessionStarted { .. }
            | OperatorEventPayloadV2::SessionInterrupted { .. }
            | OperatorEventPayloadV2::InterruptionDetected { .. }
            | OperatorEventPayloadV2::SessionResumed { .. }
            | OperatorEventPayloadV2::SessionEnded { .. }
            | OperatorEventPayloadV2::SessionAbandoned { .. }
    )
}

fn event_shape_error(event: &OperatorEventV2) -> Option<String> {
    let slot_required = |kind: &str| {
        event
            .slot_id
            .is_none()
            .then(|| format!("{kind} requires a planned slot reference"))
    };
    match &event.payload {
        payload if is_lifecycle_payload(payload) && event.slot_id.is_some() => {
            Some("lifecycle events must not reference a planned slot".into())
        }
        OperatorEventPayloadV2::AntennaStateConfirmed { antenna_label, .. } => {
            slot_required("antenna confirmation").or_else(|| {
                (antenna_label.trim().is_empty() || antenna_label.trim() != antenna_label)
                    .then(|| "actual antenna label must be nonempty and trimmed".into())
            })
        }
        OperatorEventPayloadV2::SlotMissed { .. } => slot_required("missed-slot event"),
        OperatorEventPayloadV2::SlotBad { reason } => {
            slot_required("bad-slot event").or_else(|| {
                reason
                    .trim()
                    .is_empty()
                    .then(|| "bad-slot reason must not be empty".into())
            })
        }
        OperatorEventPayloadV2::NoteAdded { note } => note
            .trim()
            .is_empty()
            .then(|| "operator note must not be empty".into()),
        OperatorEventPayloadV2::EventCorrected {
            target_event_id,
            correction,
            reason,
        } => {
            if event.slot_id.is_some() {
                return Some(
                    "correction events carry replacement slot state in the correction payload"
                        .into(),
                );
            }
            if target_event_id.trim().is_empty() {
                return Some("correction target event ID must not be empty".into());
            }
            if reason.trim().is_empty() {
                return Some("correction reason must not be empty".into());
            }
            match correction {
                EventCorrectionActionV2::Retract => None,
                EventCorrectionActionV2::Replace { replacement } => {
                    replacement_shape_error(replacement)
                }
            }
        }
        _ => None,
    }
}

fn replacement_shape_error(replacement: &ReplacementOperatorEventV2) -> Option<String> {
    match &replacement.payload {
        CorrectableOperatorEventPayloadV2::AntennaStateConfirmed { antenna_label, .. } => {
            replacement
                .slot_id
                .is_none()
                .then(|| {
                    "replacement antenna confirmation requires a planned slot reference".into()
                })
                .or_else(|| {
                    (antenna_label.trim().is_empty() || antenna_label.trim() != antenna_label).then(
                        || "replacement actual antenna label must be nonempty and trimmed".into(),
                    )
                })
        }
        CorrectableOperatorEventPayloadV2::SlotMissed { .. } => replacement
            .slot_id
            .is_none()
            .then(|| "replacement missed-slot event requires a planned slot reference".into()),
        CorrectableOperatorEventPayloadV2::SlotBad { reason } => replacement
            .slot_id
            .is_none()
            .then(|| "replacement bad-slot event requires a planned slot reference".into())
            .or_else(|| {
                reason
                    .trim()
                    .is_empty()
                    .then(|| "replacement bad-slot reason must not be empty".into())
            }),
        CorrectableOperatorEventPayloadV2::NoteAdded { note } => note
            .trim()
            .is_empty()
            .then(|| "replacement operator note must not be empty".into()),
    }
}

fn correctable_payload(
    payload: &OperatorEventPayloadV2,
) -> Option<CorrectableOperatorEventPayloadV2> {
    match payload {
        OperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label,
            note,
        } => Some(CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label: antenna_label.clone(),
            note: note.clone(),
        }),
        OperatorEventPayloadV2::SlotMissed { reason } => {
            Some(CorrectableOperatorEventPayloadV2::SlotMissed {
                reason: reason.clone(),
            })
        }
        OperatorEventPayloadV2::SlotBad { reason } => {
            Some(CorrectableOperatorEventPayloadV2::SlotBad {
                reason: reason.clone(),
            })
        }
        OperatorEventPayloadV2::NoteAdded { note } => {
            Some(CorrectableOperatorEventPayloadV2::NoteAdded { note: note.clone() })
        }
        _ => None,
    }
}

fn effective_from_replacement(
    target_event_id: &str,
    correction: &OperatorEventV2,
    replacement: &ReplacementOperatorEventV2,
) -> EffectiveOperatorEventV2 {
    EffectiveOperatorEventV2 {
        session_id: correction.meta.session_id.clone(),
        source_event_id: target_event_id.to_string(),
        effective_through_event_id: correction.event_id.clone(),
        recorded_at: correction.meta.recorded_at,
        occurred_at: replacement.occurred_at,
        time_basis: replacement.time_basis,
        uncertainty_seconds: replacement.uncertainty_seconds,
        slot_id: replacement.slot_id.clone(),
        payload: replacement.payload.clone(),
    }
}

fn diagnostic(
    event: &OperatorEventV2,
    related_event_id: Option<&str>,
    code: OperatorEventDiagnosticCodeV2,
    message: impl Into<String>,
) -> OperatorEventDiagnosticV2 {
    OperatorEventDiagnosticV2 {
        event_id: event.event_id.clone(),
        related_event_id: related_event_id.map(str::to_string),
        code,
        message: message.into(),
    }
}
