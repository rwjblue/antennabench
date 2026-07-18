//! Conductor read-model, diagnostics, recovery, and evidence presentation projection.

use std::collections::BTreeMap;

use antennabench_core::{
    v2::{
        reduce_operator_events_v2, BundleV2Contents, CorrectableOperatorEventPayloadV2,
        SessionLifecycleV2,
    },
    v3::{
        project_wspr_run_v3, reduce_operator_events_v3, validate_signal_state_confirmation_v3,
        BundleV3Contents, CorrectableOperatorEventPayloadV3, OperatorEventPayloadV3,
    },
    SCHEMA_VERSION_V4,
};
use antennabench_storage::{RecoveryDispositionV2, RecoveryReportV2};
use chrono::{DateTime, Duration, Utc};

use super::timing::{slot_evidence, slot_evidence_v3, timing_projection, timing_projection_v3};
use super::{
    ConductorDiagnostic, ConductorEventView, ConductorIntentView, ConductorPlannedSignalView,
    ConductorRecoveryView, ConductorSlotView, ConductorView, SignalEvidenceStatus,
    SlotEvidenceStatus,
};

pub(super) fn recovery_view(report: &RecoveryReportV2) -> ConductorRecoveryView {
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

pub(super) fn build_view(
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
                direction: None,
                actual_antenna,
                evidence_status,
                planned_signal: None,
                actual_signal: None,
                signal_status: SignalEvidenceStatus::NotPlanned,
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
        wsjtx_required: false,
        seconds_to_transition,
        antennas: bundle
            .antennas
            .antennas
            .iter()
            .map(|antenna| antenna.label.clone())
            .collect(),
        current_slot: current_index.map(|index| slots[index].clone()),
        next_slot: next_index.map(|index| slots[index].clone()),
        next_intent: None,
        antenna_in_use: current_index.and_then(|index| slots[index].actual_antenna.clone()),
        slots,
        effective_events,
        diagnostics,
        recovery,
    }
}

pub(super) fn requires_wsjtx_receiver(bundle: &BundleV3Contents) -> bool {
    bundle.manifest.schema_version >= SCHEMA_VERSION_V4
        && !bundle.session_state.wspr_live_acquisition_enabled
        && bundle.schedule.signal_plans.is_empty()
        && matches!(
            bundle.schedule.mode,
            antennabench_core::ExperimentMode::WholeStationAb
                | antennabench_core::ExperimentMode::RxFocused
                | antennabench_core::ExperimentMode::SingleAntennaProfiling
        )
}

pub(super) fn build_view_v3(
    bundle_name: String,
    bundle: &BundleV3Contents,
    now: DateTime<Utc>,
    action_token: String,
    recovery: Option<ConductorRecoveryView>,
) -> ConductorView {
    let reduction = reduce_operator_events_v3(SessionLifecycleV2::Ready, &bundle.events);
    let run_projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    let mut evidence = BTreeMap::<String, Vec<&CorrectableOperatorEventPayloadV3>>::new();
    let mut effective_events = Vec::new();
    for event in &reduction.effective_events {
        if let Some(slot_id) = &event.slot_id {
            evidence
                .entry(slot_id.clone())
                .or_default()
                .push(&event.payload);
        }
        let (kind, summary) = event_summary_v3(&event.payload);
        effective_events.push(ConductorEventView {
            source_event_id: event.source_event_id.clone(),
            effective_through_event_id: event.effective_through_event_id.clone(),
            occurred_at: event.occurred_at,
            slot_id: event.slot_id.clone(),
            kind,
            summary,
        });
    }
    for event in &bundle.events {
        let (kind, summary) = match &event.payload {
            OperatorEventPayloadV3::AntennaSwitchStarted { note } => (
                "antenna_switch_started",
                note.clone()
                    .unwrap_or_else(|| "Antenna switching started; occupancy is unknown.".into()),
            ),
            OperatorEventPayloadV3::WsprCycleArmed {
                antenna_label,
                cycle_starts_at,
                readiness,
            } => {
                let basis = match readiness {
                    Some(antennabench_core::v5::WsprReadinessBasisV5::CommandVerified {
                        ..
                    }) => "Command verification",
                    _ => "Operator confirmation",
                };
                (
                    "wspr_cycle_armed",
                    format!(
                        "{basis} made {antenna_label} ready; WSPR cycle armed for {cycle_starts_at}."
                    ),
                )
            }
            _ => continue,
        };
        effective_events.push(ConductorEventView {
            source_event_id: event.event_id.clone(),
            effective_through_event_id: event.event_id.clone(),
            occurred_at: event.occurred_at,
            slot_id: event.slot_id.clone(),
            kind,
            summary,
        });
    }
    effective_events.sort_by_key(|event| event.occurred_at);

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
    diagnostics.extend(
        run_projection
            .diagnostics
            .iter()
            .map(|diagnostic| ConductorDiagnostic {
                code: diagnostic.code.into(),
                message: diagnostic.message.clone(),
                slot_id: None,
                event_id: Some(diagnostic.event_id.clone()),
            }),
    );

    let projected_schedule = bundle.clone().into_current().bundle.schedule;
    let slots = projected_schedule
        .slots
        .iter()
        .map(|slot| {
            let intent = bundle
                .schedule
                .wspr_cycle_intents
                .iter()
                .find(|intent| intent.intent_id == slot.slot_id);
            let armed_cycle = run_projection
                .cycles
                .iter()
                .find(|cycle| cycle.intent_id == slot.slot_id);
            let facts = evidence.get(&slot.slot_id).map(Vec::as_slice).unwrap_or(&[]);
            let (mut evidence_status, mut actual_antenna) = slot_evidence_v3(facts);
            if evidence_status == SlotEvidenceStatus::Unknown {
                if let Some(cycle) = armed_cycle {
                    evidence_status = SlotEvidenceStatus::Confirmed;
                    actual_antenna = Some(cycle.antenna_label.clone());
                }
            }
            if evidence_status == SlotEvidenceStatus::Conflicting {
                diagnostics.push(ConductorDiagnostic {
                    code: "conductor.slot.conflicting_evidence".into(),
                    message: "Competing effective operator facts keep this slot conservatively unresolved."
                        .into(),
                    slot_id: Some(slot.slot_id.clone()),
                    event_id: None,
                });
            }

            let confirmations = facts
                .iter()
                .filter_map(|fact| match fact {
                    CorrectableOperatorEventPayloadV3::SignalStateConfirmed { confirmation } => {
                        Some(confirmation)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let allocation = intent
                .and_then(|intent| intent.signal.as_ref())
                .or_else(|| {
                    bundle
                        .schedule
                        .slots
                        .iter()
                        .find(|legacy| legacy.slot_id == slot.slot_id)
                        .and_then(|legacy| legacy.signal.as_ref())
                });
            let (actual_signal, signal_status) = match confirmations.as_slice() {
                [] if allocation.is_some() => (None, SignalEvidenceStatus::Missing),
                [] => (None, SignalEvidenceStatus::NotPlanned),
                [confirmation] => {
                    let signal_diagnostics = validate_signal_state_confirmation_v3(
                        &bundle.schedule,
                        Some(&slot.slot_id),
                        confirmation,
                    );
                    let status = if signal_diagnostics.is_empty() {
                        SignalEvidenceStatus::Confirmed
                    } else {
                        SignalEvidenceStatus::Deviated
                    };
                    diagnostics.extend(signal_diagnostics.into_iter().map(|diagnostic| {
                        ConductorDiagnostic {
                            code: diagnostic.code.into(),
                            message: diagnostic.message,
                            slot_id: Some(slot.slot_id.clone()),
                            event_id: None,
                        }
                    }));
                    (Some((*confirmation).clone()), status)
                }
                _ => {
                    diagnostics.push(ConductorDiagnostic {
                        code: "conductor.signal.conflicting_evidence".into(),
                        message: "Competing signal confirmations keep the actual transmitted state unresolved."
                            .into(),
                        slot_id: Some(slot.slot_id.clone()),
                        event_id: None,
                    });
                    (None, SignalEvidenceStatus::Conflicting)
                }
            };
            let planned_signal = allocation.and_then(|allocation| {
                bundle
                    .schedule
                    .signal_plans
                    .iter()
                    .find(|plan| plan.signal_plan_id == allocation.signal_plan_id)
                    .map(|plan| ConductorPlannedSignalView {
                        mode: plan.mode,
                        frequency_hz: allocation.frequency_hz,
                        planned_power_watts: plan.planned_power_watts,
                        transmitted_callsign: plan.transmitted_callsign.clone(),
                        message: plan.cadence.message.clone(),
                        repetition_count: plan.cadence.repetition_count,
                        key_speed_wpm: plan.cadence.key_speed_wpm,
                        transmit_seconds: plan.cadence.transmit_seconds,
                        interval_seconds: plan.cadence.interval_seconds,
                    })
            });

            ConductorSlotView {
                slot_id: slot.slot_id.clone(),
                sequence_number: slot.sequence_number,
                starts_at: armed_cycle.map_or(slot.starts_at, |cycle| cycle.window.starts_at),
                usable_at: armed_cycle.map_or(
                    slot.starts_at + Duration::seconds(i64::from(slot.guard_seconds)),
                    |cycle| cycle.window.starts_at,
                ),
                ends_at: armed_cycle.map_or(
                    slot.starts_at + Duration::seconds(i64::from(slot.duration_seconds)),
                    |cycle| cycle.window.transmission_ends_at,
                ),
                band: serde_json::to_value(slot.band)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
                    .unwrap_or_else(|| format!("{:?}", slot.band)),
                planned_antenna: intent
                    .map_or_else(|| slot.antenna_label.clone(), |intent| intent.antenna_label.clone()),
                direction: intent.and_then(|intent| intent.direction),
                actual_antenna,
                evidence_status,
                planned_signal,
                actual_signal,
                signal_status,
            }
        })
        .collect::<Vec<_>>();

    let next_intent = bundle.schedule.wspr_cycle_intents.iter().find(|intent| {
        !run_projection
            .cycles
            .iter()
            .any(|cycle| cycle.intent_id == intent.intent_id)
            && !run_projection
                .skipped_intent_ids
                .iter()
                .any(|intent_id| intent_id == &intent.intent_id)
    });
    let switching = bundle
        .events
        .iter()
        .rev()
        .find(|event| {
            matches!(
                event.payload,
                OperatorEventPayloadV3::AntennaSwitchStarted { .. }
                    | OperatorEventPayloadV3::WsprCycleArmed { .. }
            )
        })
        .is_some_and(|event| {
            matches!(
                event.payload,
                OperatorEventPayloadV3::AntennaSwitchStarted { .. }
            )
        });
    let (phase, guidance, seconds_to_transition, current_index, next_index) = timing_projection_v3(
        reduction.lifecycle,
        bundle.session_state.wspr_live_acquisition_enabled,
        &slots,
        next_intent,
        switching,
        now,
    );
    let antenna_in_use = run_projection
        .occupancies
        .iter()
        .rev()
        .find(|interval| interval.ends_at.is_none())
        .map(|interval| interval.antenna_label.clone());
    ConductorView {
        bundle_name,
        session_id: bundle.manifest.session_id.clone(),
        revision: bundle.session_state.revision,
        lifecycle: reduction.lifecycle,
        now,
        action_token,
        phase,
        guidance,
        wsjtx_required: requires_wsjtx_receiver(bundle),
        seconds_to_transition,
        antennas: bundle
            .antennas
            .antennas
            .iter()
            .map(|antenna| antenna.label.clone())
            .collect(),
        current_slot: current_index.map(|index| slots[index].clone()),
        next_slot: next_index.map(|index| slots[index].clone()),
        next_intent: next_intent.map(|intent| ConductorIntentView {
            intent_id: intent.intent_id.clone(),
            sequence_number: intent.sequence_number,
            band: serde_json::to_value(intent.band)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_else(|| format!("{:?}", intent.band)),
            antenna_label: intent.antenna_label.clone(),
            direction: intent.direction,
        }),
        antenna_in_use,
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

fn event_summary_v3(payload: &CorrectableOperatorEventPayloadV3) -> (&'static str, String) {
    match payload {
        CorrectableOperatorEventPayloadV3::AntennaStateConfirmed {
            antenna_label,
            note,
        } => (
            "antenna_state_confirmed",
            note.as_ref().map_or_else(
                || format!("Actual antenna confirmed as {antenna_label}."),
                |note| format!("Actual antenna confirmed as {antenna_label}: {note}"),
            ),
        ),
        CorrectableOperatorEventPayloadV3::SignalStateConfirmed { confirmation } => (
            "signal_state_confirmed",
            confirmation.note.clone().unwrap_or_else(|| {
                "Actual transmitted signal state confirmed by the operator.".into()
            }),
        ),
        CorrectableOperatorEventPayloadV3::SlotMissed { reason } => (
            "slot_missed",
            reason
                .as_ref()
                .map_or_else(|| "Slot marked missed.".into(), Clone::clone),
        ),
        CorrectableOperatorEventPayloadV3::SlotBad { reason } => ("slot_bad", reason.clone()),
        CorrectableOperatorEventPayloadV3::NoteAdded { note } => ("note_added", note.clone()),
    }
}
