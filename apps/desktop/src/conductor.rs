use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use antennabench_core::{
    reduce_operator_events_v2, BundleV2Contents, CorrectableOperatorEventPayloadV2,
    EventCorrectionActionV2, EventTimeBasisV2, MutationMember, OperatorEventPayloadV2,
    OperatorEventV2, Provenance, RecordMetaV2, RecordSource, ReplacementOperatorEventV2,
    SessionLifecycleV2, SCHEMA_VERSION_V2, V2_BUNDLE_SUFFIX,
};
use antennabench_storage::{
    BundleStore, LiveMutationMemberV2, LiveMutationV2, LivePersistenceError, LivePersistenceHooks,
    RecoveryDispositionV2, RecoveryReportV2, SystemLivePersistenceHooks,
};
use antennabench_wsjtx::WSPR_LIVE_INGESTION_GRACE_SECONDS;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::open_session::{
    active_session_source, check_ipc_payload, storage_error_payload, with_foreground_operation,
    ActiveSessionState, SessionErrorKind, SessionErrorPayload,
};
use crate::wsjtx_session::WsjtxSessionState;

const CONDUCTOR_VIEW_IPC_BYTES: u64 = 512 * 1024;
const MAX_PENDING_ACTION_TOKENS: usize = 32;

#[derive(Default)]
pub(crate) struct ConductorSessionState(Mutex<ConductorRuntime>);

#[derive(Default)]
struct ConductorRuntime {
    initialized_source: Option<PathBuf>,
    pending_actions: VecDeque<PendingAction>,
}

#[derive(Debug, Clone)]
struct PendingAction {
    token: String,
    session_id: String,
    expected_revision: u64,
    occurred_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConductorView {
    bundle_name: String,
    session_id: String,
    revision: u64,
    lifecycle: SessionLifecycleV2,
    now: DateTime<Utc>,
    action_token: String,
    phase: ConductorPhase,
    guidance: String,
    seconds_to_transition: Option<i64>,
    antennas: Vec<String>,
    current_slot: Option<ConductorSlotView>,
    next_slot: Option<ConductorSlotView>,
    slots: Vec<ConductorSlotView>,
    effective_events: Vec<ConductorEventView>,
    diagnostics: Vec<ConductorDiagnostic>,
    recovery: Option<ConductorRecoveryView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ConductorPhase {
    Ready,
    AwaitingSlot,
    Guard,
    Active,
    BetweenSlots,
    Finalizing,
    Complete,
    Interrupted,
    Ended,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorSlotView {
    slot_id: String,
    sequence_number: u32,
    starts_at: DateTime<Utc>,
    usable_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    band: String,
    planned_antenna: String,
    actual_antenna: Option<String>,
    evidence_status: SlotEvidenceStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SlotEvidenceStatus {
    Unknown,
    Confirmed,
    Missed,
    Bad,
    Conflicting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorEventView {
    source_event_id: String,
    effective_through_event_id: String,
    occurred_at: DateTime<Utc>,
    slot_id: Option<String>,
    kind: &'static str,
    summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorDiagnostic {
    code: String,
    message: String,
    slot_id: Option<String>,
    event_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorRecoveryView {
    disposition: &'static str,
    starting_revision: u64,
    final_revision: u64,
    artifact_count: usize,
    interruption_recorded: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConductorMutationRequest {
    action_token: String,
    expected_revision: u64,
    action: ConductorAction,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum ConductorAction {
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
    ConfirmAntenna {
        slot_id: String,
        antenna_label: String,
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
enum CorrectableAction {
    ConfirmAntenna {
        antenna_label: String,
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

impl ConductorRuntime {
    fn is_initialized(&self, source: &Path) -> bool {
        self.initialized_source.as_deref() == Some(source)
    }

    fn mark_initialized(&mut self, source: PathBuf) {
        if self.initialized_source.as_ref() != Some(&source) {
            self.pending_actions.clear();
        }
        self.initialized_source = Some(source);
    }

    fn register_action(&mut self, action: PendingAction) {
        self.pending_actions.push_back(action);
        while self.pending_actions.len() > MAX_PENDING_ACTION_TOKENS {
            self.pending_actions.pop_front();
        }
    }

    fn resolve_action(&mut self, token: &str, now: DateTime<Utc>) -> Option<PendingAction> {
        self.pending_actions
            .iter_mut()
            .find(|pending| pending.token == token)
            .map(|pending| {
                pending.occurred_at.get_or_insert(now);
                pending.clone()
            })
    }
}

fn ensure_v2_source(source: &Path) -> Result<(), SessionErrorPayload> {
    let valid = source
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(V2_BUNDLE_SUFFIX));
    if valid {
        Ok(())
    } else {
        Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "The live conductor requires a schema-v2 session bundle.",
            "schema-v1 bundles remain read-only and must be explicitly upgraded",
        ))
    }
}

pub(crate) fn live_error_payload(error: LivePersistenceError) -> SessionErrorPayload {
    match error {
        LivePersistenceError::Store(error) => storage_error_payload(error),
        LivePersistenceError::WriterBusy => SessionErrorPayload::new(
            SessionErrorKind::Busy,
            "Another local operation is updating this session.",
            "schema-v2 writer lock is busy",
        ),
        LivePersistenceError::StaleRevision { expected, actual } => SessionErrorPayload::new(
            SessionErrorKind::StaleRevision,
            "The session changed. Refresh the conductor before retrying.",
            format!("expected checkpoint revision {expected}, actual revision {actual}"),
        ),
        LivePersistenceError::MutationConflict { mutation_id } => SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "That action token was already used for different evidence.",
            format!("conflicting mutation ID {mutation_id}"),
        ),
        LivePersistenceError::Capability { message } => SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "This filesystem cannot safely conduct a live session.",
            message,
        ),
        error @ LivePersistenceError::RecoveryRequired { .. } => SessionErrorPayload::new(
            SessionErrorKind::Verification,
            "The session must be recovered before it can be changed.",
            error.to_string(),
        ),
        error @ (LivePersistenceError::InvalidMutation { .. }
        | LivePersistenceError::PlanFrozen { .. }) => SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The requested conductor action is not valid for this session.",
            error.to_string(),
        ),
        error @ LivePersistenceError::ExternalModification { .. } => SessionErrorPayload::new(
            SessionErrorKind::Verification,
            "The session changed outside AntennaBench and live mutation was stopped.",
            error.to_string(),
        ),
        error @ LivePersistenceError::Io { .. } => SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The conductor could not durably update the session.",
            error.to_string(),
        ),
        error @ LivePersistenceError::CheckpointVerification { .. } => SessionErrorPayload::new(
            SessionErrorKind::Verification,
            "The conductor could not verify a coherent checkpoint.",
            error.to_string(),
        ),
    }
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

fn event_for_action(
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
        },
        event_id: format!("event-for-{}", pending.token),
        occurred_at,
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id,
        payload,
    })
}

fn recovery_view(report: &RecoveryReportV2) -> ConductorRecoveryView {
    ConductorRecoveryView {
        disposition: match report.disposition {
            RecoveryDispositionV2::Clean => "clean",
            RecoveryDispositionV2::RolledForward => "rolled_forward",
            RecoveryDispositionV2::RolledBack => "rolled_back",
            RecoveryDispositionV2::IdempotentTailRemoved => "idempotent_tail_removed",
        },
        starting_revision: report.starting_revision,
        final_revision: report.final_revision,
        artifact_count: report.artifacts.len(),
        interruption_recorded: report.interruption.is_some(),
    }
}

fn build_view(
    bundle_name: String,
    bundle: &BundleV2Contents,
    now: DateTime<Utc>,
    action_token: String,
    recovery: Option<ConductorRecoveryView>,
) -> ConductorView {
    let reduction = reduce_operator_events_v2(SessionLifecycleV2::Ready, &bundle.events);
    let mut evidence = BTreeMap::<String, Vec<&CorrectableOperatorEventPayloadV2>>::new();
    let mut effective_events = Vec::new();
    for event in &reduction.effective_events {
        if let Some(slot_id) = &event.slot_id {
            evidence
                .entry(slot_id.clone())
                .or_default()
                .push(&event.payload);
        }
        let (kind, summary) = event_summary(&event.payload);
        effective_events.push(ConductorEventView {
            source_event_id: event.source_event_id.clone(),
            effective_through_event_id: event.effective_through_event_id.clone(),
            occurred_at: event.occurred_at,
            slot_id: event.slot_id.clone(),
            kind,
            summary,
        });
    }

    let mut diagnostics = reduction
        .diagnostics
        .into_iter()
        .map(|diagnostic| ConductorDiagnostic {
            code: format!("operator_event.{:?}", diagnostic.code).to_lowercase(),
            message: diagnostic.message,
            slot_id: None,
            event_id: Some(diagnostic.event_id),
        })
        .collect::<Vec<_>>();

    let slots = bundle
        .schedule
        .slots
        .iter()
        .map(|slot| {
            let facts = evidence.get(&slot.slot_id).map(Vec::as_slice).unwrap_or(&[]);
            let (evidence_status, actual_antenna) = slot_evidence(facts);
            if evidence_status == SlotEvidenceStatus::Conflicting {
                diagnostics.push(ConductorDiagnostic {
                    code: "conductor.slot.conflicting_evidence".into(),
                    message: "Competing effective operator facts keep this slot conservatively unresolved."
                        .into(),
                    slot_id: Some(slot.slot_id.clone()),
                    event_id: None,
                });
            }
            ConductorSlotView {
                slot_id: slot.slot_id.clone(),
                sequence_number: slot.sequence_number,
                starts_at: slot.starts_at,
                usable_at: slot.starts_at + Duration::seconds(i64::from(slot.guard_seconds)),
                ends_at: slot.starts_at + Duration::seconds(i64::from(slot.duration_seconds)),
                band: serde_json::to_value(slot.band)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
                    .unwrap_or_else(|| format!("{:?}", slot.band)),
                planned_antenna: slot.antenna_label.clone(),
                actual_antenna,
                evidence_status,
            }
        })
        .collect::<Vec<_>>();

    let (phase, guidance, seconds_to_transition, current_index, next_index) = timing_projection(
        bundle.session_state.lifecycle,
        bundle.session_state.wspr_live_acquisition_enabled,
        &slots,
        now,
    );
    ConductorView {
        bundle_name,
        session_id: bundle.manifest.session_id.clone(),
        revision: bundle.session_state.revision,
        lifecycle: bundle.session_state.lifecycle,
        now,
        action_token,
        phase,
        guidance,
        seconds_to_transition,
        antennas: bundle
            .antennas
            .antennas
            .iter()
            .map(|antenna| antenna.label.clone())
            .collect(),
        current_slot: current_index.map(|index| slots[index].clone()),
        next_slot: next_index.map(|index| slots[index].clone()),
        slots,
        effective_events,
        diagnostics,
        recovery,
    }
}

fn event_summary(payload: &CorrectableOperatorEventPayloadV2) -> (&'static str, String) {
    match payload {
        CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label,
            note,
        } => (
            "antenna_state_confirmed",
            note.as_ref().map_or_else(
                || format!("Actual antenna confirmed as {antenna_label}."),
                |note| format!("Actual antenna confirmed as {antenna_label}: {note}"),
            ),
        ),
        CorrectableOperatorEventPayloadV2::SlotMissed { reason } => (
            "slot_missed",
            reason
                .as_ref()
                .map_or_else(|| "Slot marked missed.".into(), |reason| reason.clone()),
        ),
        CorrectableOperatorEventPayloadV2::SlotBad { reason } => ("slot_bad", reason.clone()),
        CorrectableOperatorEventPayloadV2::NoteAdded { note } => ("note_added", note.clone()),
    }
}

fn slot_evidence(
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

fn timing_projection(
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

fn register_view_action(
    state: &ConductorSessionState,
    session_id: &str,
    revision: u64,
    token: String,
) -> Result<(), SessionErrorPayload> {
    state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("conductor state is unavailable"))?
        .register_action(PendingAction {
            token,
            session_id: session_id.to_string(),
            expected_revision: revision,
            occurred_at: None,
        });
    Ok(())
}

fn read_conductor_with_hooks(
    active_state: &ActiveSessionState,
    conductor_state: &ConductorSessionState,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<ConductorView, SessionErrorPayload> {
    with_foreground_operation(active_state, || {
        let (source, bundle_name) = active_session_source(active_state)?;
        ensure_v2_source(&source)?;
        let store = BundleStore::new(&source);
        let initialized = conductor_state
            .0
            .lock()
            .map_err(|_| SessionErrorPayload::report_pipeline("conductor state is unavailable"))?
            .is_initialized(&source);
        let recovery = if initialized {
            None
        } else {
            let report = store
                .recover_v2_with_hooks(hooks.clone())
                .map_err(live_error_payload)?;
            conductor_state
                .0
                .lock()
                .map_err(|_| {
                    SessionErrorPayload::report_pipeline("conductor state is unavailable")
                })?
                .mark_initialized(source.clone());
            Some(recovery_view(&report))
        };
        let bundle = store.read_v2_checkpointed().map_err(live_error_payload)?;
        let now = hooks.now();
        let action_token = hooks.new_id("mutation");
        register_view_action(
            conductor_state,
            &bundle.manifest.session_id,
            bundle.session_state.revision,
            action_token.clone(),
        )?;
        let view = build_view(bundle_name, &bundle, now, action_token, recovery);
        check_ipc_payload(&view, CONDUCTOR_VIEW_IPC_BYTES, "conductor_view")?;
        Ok(view)
    })
}

fn mutate_conductor_with_hooks(
    active_state: &ActiveSessionState,
    conductor_state: &ConductorSessionState,
    request: ConductorMutationRequest,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<ConductorView, SessionErrorPayload> {
    with_foreground_operation(active_state, || {
        let (source, bundle_name) = active_session_source(active_state)?;
        ensure_v2_source(&source)?;
        let store = BundleStore::new(&source);
        let snapshot = store.read_v2_checkpointed().map_err(live_error_payload)?;
        let pending = conductor_state
            .0
            .lock()
            .map_err(|_| SessionErrorPayload::report_pipeline("conductor state is unavailable"))?
            .resolve_action(&request.action_token, hooks.now())
            .ok_or_else(|| {
                SessionErrorPayload::new(
                    SessionErrorKind::StaleRevision,
                    "Refresh the conductor before submitting this action.",
                    "the Rust-issued action token is missing or expired",
                )
            })?;
        if pending.session_id != snapshot.manifest.session_id
            || pending.expected_revision != request.expected_revision
        {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::StaleRevision,
                "Refresh the conductor before submitting this action.",
                "the action token does not match this session revision",
            ));
        }
        let event = event_for_action(&snapshot.manifest.session_id, &pending, request.action)?;
        let mutation = LiveMutationV2 {
            expected_revision: request.expected_revision,
            mutation_id: pending.token.clone(),
            members: vec![LiveMutationMemberV2::Event(event)],
        };
        let append_result = {
            let mut writer = store
                .open_v2_writer_with_hooks(hooks.clone())
                .map_err(live_error_payload)?;
            writer.append(mutation)
        };
        if let Err(error) = append_result {
            let committed = store.read_v2_checkpointed().ok().is_some_and(|bundle| {
                bundle.session_state.last_committed_mutation_id.as_deref()
                    == Some(pending.token.as_str())
            });
            if !committed {
                return Err(live_error_payload(error));
            }
        }
        let bundle = store.read_v2_checkpointed().map_err(live_error_payload)?;
        let now = hooks.now();
        let action_token = hooks.new_id("mutation");
        register_view_action(
            conductor_state,
            &bundle.manifest.session_id,
            bundle.session_state.revision,
            action_token.clone(),
        )?;
        let view = build_view(bundle_name, &bundle, now, action_token, None);
        check_ipc_payload(&view, CONDUCTOR_VIEW_IPC_BYTES, "conductor_view")?;
        Ok(view)
    })
}

#[tauri::command]
pub(crate) fn active_session_conductor(
    active_state: State<'_, ActiveSessionState>,
    conductor_state: State<'_, ConductorSessionState>,
) -> Result<ConductorView, SessionErrorPayload> {
    read_conductor_with_hooks(
        active_state.inner(),
        conductor_state.inner(),
        Arc::new(SystemLivePersistenceHooks),
    )
}

#[tauri::command]
pub(crate) fn mutate_active_session_conductor(
    request: ConductorMutationRequest,
    active_state: State<'_, ActiveSessionState>,
    conductor_state: State<'_, ConductorSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
) -> Result<ConductorView, SessionErrorPayload> {
    let view = mutate_conductor_with_hooks(
        active_state.inner(),
        conductor_state.inner(),
        request,
        Arc::new(SystemLivePersistenceHooks),
    )?;
    if view.lifecycle != SessionLifecycleV2::Running {
        let (source, _) = active_session_source(active_state.inner())?;
        wsjtx_state.stop_for_source(
            &source,
            "WSJT-X reception stopped because the durable session is not running.",
        );
    }
    Ok(view)
}

#[cfg(test)]
mod tests {
    use std::{
        fs, io,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    };

    use antennabench_core::{
        reduce_operator_events_v2, AdapterInput, Band, CorrectableOperatorEventPayloadV2,
        PlannedSlot, SessionLifecycleV2, V2_BUNDLE_SUFFIX,
    };
    use antennabench_storage::{
        BundleStore, LiveMutationMemberV2, LiveMutationV2, LivePersistenceHooks,
        LivePersistencePoint, LiveStreamV2, RecoveryDispositionV2,
    };
    use chrono::{DateTime, TimeZone, Utc};
    use tempfile::TempDir;

    use super::{
        mutate_conductor_with_hooks, read_conductor_with_hooks, ConductorAction,
        ConductorMutationRequest, ConductorPhase, ConductorSessionState, CorrectableAction,
        SlotEvidenceStatus,
    };
    use crate::{
        open_session::{
            activate_created_bundle, e2e_report_snapshot, export_e2e_snapshots, ActiveSessionState,
            SessionErrorKind,
        },
        setup::create_e2e_session,
        wsjtx_session::inject_e2e_wsjtx_sequence,
    };

    #[derive(Debug)]
    struct TestHooks {
        now: Mutex<DateTime<Utc>>,
        next_id: Mutex<u64>,
        fail_once: Mutex<Option<LivePersistencePoint>>,
    }

    impl TestHooks {
        fn new(now: DateTime<Utc>) -> Self {
            Self {
                now: Mutex::new(now),
                next_id: Mutex::new(1),
                fail_once: Mutex::new(None),
            }
        }

        fn set_now(&self, now: DateTime<Utc>) {
            *self.now.lock().unwrap() = now;
        }

        fn fail_once_at(&self, point: LivePersistencePoint) {
            *self.fail_once.lock().unwrap() = Some(point);
        }
    }

    impl LivePersistenceHooks for TestHooks {
        fn now(&self) -> DateTime<Utc> {
            *self.now.lock().unwrap()
        }

        fn new_id(&self, kind: &str) -> String {
            let mut next = self.next_id.lock().unwrap();
            let value = format!("{kind}-{next:04}");
            *next += 1;
            value
        }

        fn check(&self, point: LivePersistencePoint) -> io::Result<()> {
            let mut fail = self.fail_once.lock().unwrap();
            if fail.as_ref() == Some(&point) {
                *fail = None;
                Err(io::Error::other("injected lost acknowledgement"))
            } else {
                Ok(())
            }
        }
    }

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle")
    }

    fn ready_store(temp: &TempDir, start: DateTime<Utc>) -> BundleStore {
        let upgraded = BundleStore::new(fixture_root())
            .upgrade_v1_to_v2(temp.path().join(format!("upgraded{V2_BUNDLE_SUFFIX}")))
            .unwrap();
        let mut bundle = upgraded.read_v2().unwrap();
        bundle.events.clear();
        bundle.adapter_records.clear();
        bundle.observations.clear();
        bundle.rig.clear();
        bundle.propagation.clear();
        bundle.session_state.lifecycle = SessionLifecycleV2::Ready;
        bundle.session_state.revision = 0;
        bundle.session_state.last_committed_mutation_id = None;
        let labels = bundle
            .antennas
            .antennas
            .iter()
            .map(|antenna| antenna.label.clone())
            .collect::<Vec<_>>();
        assert!(labels.len() >= 2);
        bundle.schedule.slots = vec![
            PlannedSlot {
                slot_id: "slot-1".into(),
                sequence_number: 1,
                starts_at: start,
                duration_seconds: 120,
                guard_seconds: 10,
                band: Band::M20,
                antenna_label: labels[0].clone(),
            },
            PlannedSlot {
                slot_id: "slot-2".into(),
                sequence_number: 2,
                starts_at: start + chrono::Duration::seconds(120),
                duration_seconds: 120,
                guard_seconds: 10,
                band: Band::M20,
                antenna_label: labels[1].clone(),
            },
        ];
        BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();
        let store = BundleStore::new(temp.path().join(format!("live{V2_BUNDLE_SUFFIX}")));
        store.write_v2(&bundle).unwrap();
        store
    }

    fn activate(store: &BundleStore) -> ActiveSessionState {
        let state = ActiveSessionState::default();
        activate_created_bundle(&state, store.root().to_path_buf()).unwrap();
        state
    }

    fn request(view: &super::ConductorView, action: ConductorAction) -> ConductorMutationRequest {
        ConductorMutationRequest {
            action_token: view.action_token.clone(),
            expected_revision: view.revision,
            action,
        }
    }

    struct FailureArtifacts {
        source: PathBuf,
        seed: &'static str,
    }

    impl Drop for FailureArtifacts {
        fn drop(&mut self) {
            if !std::thread::panicking() {
                return;
            }
            let destination = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/desktop-e2e-failures")
                .join(self.seed);
            let _ = fs::remove_dir_all(&destination);
            let _ = copy_tree(&self.source, &destination);
            eprintln!(
                "desktop-e2e failure-artifacts={} seed={}",
                destination.display(),
                self.seed
            );
        }
    }

    fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let target = destination.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_tree(&entry.path(), &target)?;
            } else {
                fs::copy(entry.path(), target)?;
            }
        }
        Ok(())
    }

    #[test]
    fn deterministic_clock_projects_schedule_boundaries_without_persisting_timer_state() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let active = activate(&store);
        let state = ConductorSessionState::default();
        let hooks = Arc::new(TestHooks::new(start - chrono::Duration::seconds(30)));

        let ready = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        assert_eq!(ready.phase, ConductorPhase::Ready);
        let awaiting = mutate_conductor_with_hooks(
            &active,
            &state,
            request(&ready, ConductorAction::Start { note: None }),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(awaiting.phase, ConductorPhase::AwaitingSlot);
        assert_eq!(awaiting.seconds_to_transition, Some(30));

        hooks.set_now(start);
        let guard = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        assert_eq!(guard.phase, ConductorPhase::Guard);
        assert_eq!(guard.seconds_to_transition, Some(10));

        hooks.set_now(start + chrono::Duration::seconds(10));
        let active_slot = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        assert_eq!(active_slot.phase, ConductorPhase::Active);
        assert_eq!(active_slot.current_slot.unwrap().slot_id, "slot-1");

        hooks.set_now(start + chrono::Duration::seconds(240));
        let complete = read_conductor_with_hooks(&active, &state, hooks).unwrap();
        assert_eq!(complete.phase, ConductorPhase::Complete);
        assert_eq!(complete.revision, awaiting.revision);
    }

    #[test]
    fn confirmed_final_slot_projects_the_ingestion_grace_as_finalizing() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let mut bundle = store.read_v2().unwrap();
        bundle.session_state.wspr_live_acquisition_enabled = true;
        fs::write(
            store.root().join("session-state.json"),
            serde_json::to_vec_pretty(&bundle.session_state).unwrap(),
        )
        .unwrap();
        let active = activate(&store);
        let state = ConductorSessionState::default();
        let hooks = Arc::new(TestHooks::new(start - chrono::Duration::seconds(30)));
        let ready = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        mutate_conductor_with_hooks(
            &active,
            &state,
            request(&ready, ConductorAction::Start { note: None }),
            hooks.clone(),
        )
        .unwrap();

        hooks.set_now(start + chrono::Duration::seconds(120));
        let second_slot = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        let antenna_label = second_slot
            .current_slot
            .as_ref()
            .unwrap()
            .planned_antenna
            .clone();
        mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &second_slot,
                ConductorAction::ConfirmAntenna {
                    slot_id: "slot-2".into(),
                    antenna_label,
                    note: None,
                },
            ),
            hooks.clone(),
        )
        .unwrap();

        hooks.set_now(start + chrono::Duration::seconds(240));
        let waiting = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        assert_eq!(waiting.phase, ConductorPhase::Finalizing);
        assert_eq!(waiting.seconds_to_transition, Some(300));

        hooks.set_now(start + chrono::Duration::seconds(540));
        let due = read_conductor_with_hooks(&active, &state, hooks).unwrap();
        assert_eq!(due.phase, ConductorPhase::Finalizing);
        assert_eq!(due.seconds_to_transition, Some(0));
    }

    #[test]
    fn desktop_e2e_manual_conductor_records_actual_state_corrections_and_lifecycle() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let labels = store
            .read_v2()
            .unwrap()
            .antennas
            .antennas
            .iter()
            .map(|antenna| antenna.label.clone())
            .collect::<Vec<_>>();
        let active = activate(&store);
        let state = ConductorSessionState::default();
        let hooks = Arc::new(TestHooks::new(start + chrono::Duration::seconds(20)));

        let ready = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        let running = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &ready,
                ConductorAction::Start {
                    note: Some("manual no-rig run".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let confirmed = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &running,
                ConductorAction::ConfirmAntenna {
                    slot_id: "slot-1".into(),
                    antenna_label: labels[1].clone(),
                    note: Some("operator checked the switch".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(
            confirmed.current_slot.as_ref().unwrap().actual_antenna,
            Some(labels[1].clone())
        );
        assert_ne!(
            confirmed
                .current_slot
                .as_ref()
                .unwrap()
                .planned_antenna
                .as_str(),
            confirmed
                .current_slot
                .as_ref()
                .unwrap()
                .actual_antenna
                .as_deref()
                .unwrap()
        );
        let original = confirmed.effective_events[0].source_event_id.clone();

        let replaced = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &confirmed,
                ConductorAction::ReplaceEvent {
                    target_event_id: original.clone(),
                    slot_id: Some("slot-1".into()),
                    replacement: CorrectableAction::ConfirmAntenna {
                        antenna_label: labels[0].clone(),
                        note: Some("corrected after inspection".into()),
                    },
                    reason: "wrong antenna selected in the first entry".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(
            replaced.current_slot.as_ref().unwrap().actual_antenna,
            Some(labels[0].clone())
        );
        assert_ne!(
            replaced.effective_events[0].source_event_id,
            replaced.effective_events[0].effective_through_event_id
        );

        let retracted = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &replaced,
                ConductorAction::RetractEvent {
                    target_event_id: original,
                    reason: "confirmation could not be verified".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(
            retracted.current_slot.as_ref().unwrap().evidence_status,
            SlotEvidenceStatus::Unknown
        );

        let interrupted = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &retracted,
                ConductorAction::Interrupt {
                    reason: Some("operator pause".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(interrupted.lifecycle, SessionLifecycleV2::Interrupted);
        let resumed = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &interrupted,
                ConductorAction::Resume {
                    note: Some("manual checks complete".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let ended = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &resumed,
                ConductorAction::End {
                    reason: Some("planned run complete".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(ended.lifecycle, SessionLifecycleV2::Ended);

        let terminal = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &ended,
                ConductorAction::AddNote {
                    slot_id: None,
                    note: "too late".into(),
                },
            ),
            hooks,
        )
        .unwrap_err();
        assert_eq!(terminal.kind, SessionErrorKind::Validation);
        println!(
            "desktop-e2e result=manual-conductor revision={} lifecycle={:?}",
            ended.revision, ended.lifecycle
        );
    }

    #[test]
    fn desktop_e2e_complete_local_workflow_is_coherent_recoverable_and_exportable() {
        const SEED: &str = "complete-workflow-v1";
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("scenario-seed.txt"), SEED).unwrap();
        let _failure_artifacts = FailureArtifacts {
            source: temp.path().to_path_buf(),
            seed: SEED,
        };
        let active = ActiveSessionState::default();
        let created = create_e2e_session(temp.path(), &active);
        assert_eq!(created.slot_ids.len(), 4);
        assert_eq!(created.antenna_labels, ["Vertical", "Dipole"]);
        let store = BundleStore::new(&created.path);
        let initial = store.read_v2_checkpointed().unwrap();
        assert_eq!(initial.manifest.session_id, created.session_id);
        assert_eq!(initial.session_state.revision, 0);
        assert_eq!(initial.session_state.lifecycle, SessionLifecycleV2::Ready);

        let at = Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 20).unwrap();
        let hooks = Arc::new(TestHooks::new(at));
        let conductor = ConductorSessionState::default();
        let ready = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        let start_request = request(
            &ready,
            ConductorAction::Start {
                note: Some("deterministic complete workflow".into()),
            },
        );
        hooks.fail_once_at(LivePersistencePoint::BeforeAcknowledge);
        let started =
            mutate_conductor_with_hooks(&active, &conductor, start_request.clone(), hooks.clone())
                .unwrap();
        let retried =
            mutate_conductor_with_hooks(&active, &conductor, start_request, hooks.clone()).unwrap();
        assert_eq!(started.revision, 1);
        assert_eq!(retried.revision, 1);
        assert_eq!(store.read_v2_checkpointed().unwrap().events.len(), 1);

        let confirmed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &retried,
                ConductorAction::ConfirmAntenna {
                    slot_id: created.slot_ids[0].clone(),
                    antenna_label: created.antenna_labels[1].clone(),
                    note: Some("operator verified the actual switch".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let original_confirmation = confirmed
            .effective_events
            .iter()
            .find(|event| event.kind == "antenna_state_confirmed")
            .unwrap()
            .source_event_id
            .clone();
        let missed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &confirmed,
                ConductorAction::MarkMissed {
                    slot_id: created.slot_ids[1].clone(),
                    reason: Some("operator was unavailable".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let bad = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &missed,
                ConductorAction::MarkBad {
                    slot_id: created.slot_ids[2].clone(),
                    reason: "feedline connection was suspect".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let noted = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &bad,
                ConductorAction::AddNote {
                    slot_id: Some(created.slot_ids[0].clone()),
                    note: "manual evidence remains available without WSJT-X".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let corrected = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &noted,
                ConductorAction::ReplaceEvent {
                    target_event_id: original_confirmation,
                    slot_id: Some(created.slot_ids[0].clone()),
                    replacement: CorrectableAction::ConfirmAntenna {
                        antenna_label: created.antenna_labels[0].clone(),
                        note: Some("corrected after inspecting the switch".into()),
                    },
                    reason: "first actual-state entry selected the wrong label".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let interrupted = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &corrected,
                ConductorAction::Interrupt {
                    reason: Some("planned operator pause".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(interrupted.lifecycle, SessionLifecycleV2::Interrupted);
        let resumed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &interrupted,
                ConductorAction::Resume {
                    note: Some("operator returned".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(resumed.lifecycle, SessionLifecycleV2::Running);

        let wsjtx_at = Utc.with_ymd_and_hms(2026, 7, 15, 20, 1, 50).unwrap();
        let intake = inject_e2e_wsjtx_sequence(&created.path, wsjtx_at);
        assert_eq!(intake.adapter_records, 4);
        assert_eq!(intake.observations, 1);
        assert_eq!(intake.gaps, 1);
        assert!(intake.revision > resumed.revision);

        hooks.set_now(Utc.with_ymd_and_hms(2026, 7, 15, 20, 2, 30).unwrap());
        let current = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        hooks.fail_once_at(LivePersistencePoint::MidStreamWrite(LiveStreamV2::Events));
        let crash = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &current,
                ConductorAction::AddNote {
                    slot_id: None,
                    note: "torn crash mutation must not become evidence".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap_err();
        assert_eq!(crash.kind, SessionErrorKind::Filesystem);
        assert_eq!(
            store.read_v2_checkpointed().unwrap().session_state.revision,
            intake.revision
        );

        let crash_recovery = store.recover_v2_with_hooks(hooks.clone()).unwrap();
        assert_eq!(crash_recovery.disposition, RecoveryDispositionV2::Clean);
        assert_eq!(crash_recovery.starting_revision, intake.revision);
        assert_eq!(crash_recovery.recovered_revision, intake.revision);
        assert_eq!(crash_recovery.final_revision, intake.revision + 1);
        assert!(crash_recovery.interruption.is_some());

        let reopened_active = ActiveSessionState::default();
        activate_created_bundle(&reopened_active, created.path.clone()).unwrap();
        let recovered_conductor = ConductorSessionState::default();
        let recovered =
            read_conductor_with_hooks(&reopened_active, &recovered_conductor, hooks.clone())
                .unwrap();
        assert_eq!(recovered.lifecycle, SessionLifecycleV2::Interrupted);
        let recovery = recovered
            .recovery
            .as_ref()
            .expect("process recovery details");
        assert_eq!(recovery.disposition, "clean");
        assert_eq!(recovery.starting_revision, intake.revision + 1);
        assert_eq!(recovery.final_revision, intake.revision + 1);
        assert!(!recovery.interruption_recorded);
        let resumed = mutate_conductor_with_hooks(
            &reopened_active,
            &recovered_conductor,
            request(
                &recovered,
                ConductorAction::Resume {
                    note: Some("resumed after deterministic crash recovery".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let ended = mutate_conductor_with_hooks(
            &reopened_active,
            &recovered_conductor,
            request(
                &resumed,
                ConductorAction::End {
                    reason: Some("complete workflow finished".into()),
                },
            ),
            hooks,
        )
        .unwrap();
        assert_eq!(ended.lifecycle, SessionLifecycleV2::Ended);

        let final_bundle = store.read_v2_checkpointed().unwrap();
        assert_eq!(
            final_bundle.session_state.lifecycle,
            SessionLifecycleV2::Ended
        );
        assert_eq!(final_bundle.session_state.revision, ended.revision);
        assert_eq!(final_bundle.adapter_records.len(), 4);
        assert_eq!(final_bundle.observations.len(), 1);
        assert!(final_bundle.adapter_records.iter().take(3).all(|record| {
            matches!(
                &record.input,
                AdapterInput::Inline {
                    data,
                    encoding: Some(encoding),
                    ..
                } if !data.is_empty() && encoding == "hex"
            )
        }));
        assert!(!final_bundle.events.iter().any(|event| {
            matches!(
                &event.payload,
                antennabench_core::OperatorEventPayloadV2::NoteAdded { note }
                    if note.contains("torn crash mutation")
            )
        }));
        let reduction = reduce_operator_events_v2(SessionLifecycleV2::Ready, &final_bundle.events);
        assert_eq!(reduction.lifecycle, SessionLifecycleV2::Ended);
        assert!(reduction.diagnostics.is_empty());
        assert!(reduction.effective_events.iter().any(|event| {
            event.payload
                == CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
                    antenna_label: created.antenna_labels[0].clone(),
                    note: Some("corrected after inspecting the switch".into()),
                }
        }));

        let exported = export_e2e_snapshots(&reopened_active, temp.path());
        assert_eq!(exported.revision, ended.revision);
        assert!(exported.presentation_id > 0);
        assert!(exported.report_path.exists());
        assert!(exported.report_html.contains(&format!(
            "<dt>Checkpoint revision</dt><dd>{}</dd>",
            ended.revision
        )));
        assert!(exported.report_html.contains("Ended / final"));
        assert!(exported
            .report_html
            .contains("Incomplete or unknown: 1 explicit acquisition gap(s)"));
        assert!(exported
            .report_html
            .contains("Lifecycle and interruption history"));

        let exported_store = BundleStore::new(&exported.bundle_path);
        let reopened_bundle = exported_store.read_v2_checkpointed().unwrap();
        assert_eq!(reopened_bundle, final_bundle);
        let final_active = ActiveSessionState::default();
        activate_created_bundle(&final_active, exported.bundle_path.clone()).unwrap();
        let (report_revision, presentation_id, reopened_html) = e2e_report_snapshot(&final_active);
        assert_eq!(report_revision, ended.revision);
        assert!(presentation_id > 0);
        assert_eq!(reopened_html, exported.report_html);
        fs::write(
            temp.path().join("scenario-result.txt"),
            format!(
                "seed={SEED}\nrevision={}\nevents={}\nadapter_records={}\nobservations={}\n",
                ended.revision,
                final_bundle.events.len(),
                final_bundle.adapter_records.len(),
                final_bundle.observations.len()
            ),
        )
        .unwrap();
        eprintln!(
            "desktop-e2e result=complete-workflow seed={SEED} revision={} report={} bundle={}",
            ended.revision,
            exported.report_path.display(),
            exported.bundle_path.display()
        );
    }

    #[test]
    fn idempotent_retry_lost_ack_stale_revision_and_recovery_are_explicit() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let active = activate(&store);
        let state = ConductorSessionState::default();
        let hooks = Arc::new(TestHooks::new(start));
        let first = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        let competing = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        let start_request = request(&first, ConductorAction::Start { note: None });
        hooks.fail_once_at(LivePersistencePoint::BeforeAcknowledge);
        let started =
            mutate_conductor_with_hooks(&active, &state, start_request.clone(), hooks.clone())
                .unwrap();
        assert_eq!(started.revision, 1);

        let retried =
            mutate_conductor_with_hooks(&active, &state, start_request, hooks.clone()).unwrap();
        assert_eq!(retried.revision, 1);
        assert_eq!(store.read_v2().unwrap().events.len(), 1);

        let stale = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &competing,
                ConductorAction::AddNote {
                    slot_id: None,
                    note: "stale action".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap_err();
        assert_eq!(stale.kind, SessionErrorKind::StaleRevision);

        let restarted_state = ConductorSessionState::default();
        let recovered = read_conductor_with_hooks(&active, &restarted_state, hooks).unwrap();
        assert_eq!(recovered.lifecycle, SessionLifecycleV2::Interrupted);
        assert!(recovered.recovery.unwrap().interruption_recorded);
        assert_eq!(store.read_v2().unwrap().events.len(), 2);
    }

    #[test]
    fn conflicting_effective_slot_facts_are_conservative() {
        let confirmed =
            antennabench_core::CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
                antenna_label: "A".into(),
                note: None,
            };
        let missed =
            antennabench_core::CorrectableOperatorEventPayloadV2::SlotMissed { reason: None };

        assert_eq!(
            super::slot_evidence(&[&confirmed, &missed]),
            (SlotEvidenceStatus::Conflicting, None)
        );
    }

    #[test]
    fn writer_primitives_remain_the_only_durable_event_path() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let mut writer = store.open_v2_writer().unwrap();
        let revision = writer.checkpoint().revision;
        let mutation_id = "mutation-direct-proof".to_string();
        let pending = super::PendingAction {
            token: mutation_id.clone(),
            session_id: store.read_v2().unwrap().manifest.session_id.clone(),
            expected_revision: revision,
            occurred_at: Some(start),
        };
        let event = super::event_for_action(
            &pending.session_id,
            &pending,
            ConductorAction::Start { note: None },
        )
        .unwrap();
        writer
            .append(LiveMutationV2 {
                expected_revision: revision,
                mutation_id,
                members: vec![LiveMutationMemberV2::Event(event)],
            })
            .unwrap();
        assert_eq!(writer.checkpoint().lifecycle, SessionLifecycleV2::Running);
    }
}
