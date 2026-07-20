use std::{sync::atomic::Ordering, thread, time::Duration as StdDuration};

use antennabench_core::{
    v2::SessionLifecycleV2,
    v3::project_wspr_run_v3,
    v5::{AntennaControlInvocationPolicyV5, AntennaControlPolicyV5, AntennaControlRoleV5},
    v6::{DiagnosticOperationV6, DiagnosticPhaseV6, DiagnosticTargetV6, EvidenceEffectV6},
};
use antennabench_storage::{BundleStore, LiveAntennaControlMutationV5, SystemLivePersistenceHooks};
use chrono::Utc;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::{
    policy::{
        command_verified_event, next_intent, AntennaControllerState, AutomationStatus,
        ControllerAttemptSummary, RuntimeAssociation,
    },
    process::{attempt_diagnostic, execute_profile_attempt, invocation_context, rig_record},
    profiles::{read_catalog, resolved_app_data_dir, ControllerProfile},
};
use crate::{
    conductor::{
        live_error_payload,
        transition::{persist_continued_readiness, ContinuationOutcome},
    },
    open_session::{
        active_session_source, with_waiting_foreground_operation, ActiveSessionState,
        SessionErrorKind, SessionErrorPayload,
    },
};

const WAIT_POLL: StdDuration = StdDuration::from_millis(100);

/// Starts at most one Rust-owned automatic worker for the active session.
///
/// Calling this after every successful conductor mutation is intentional: it
/// lets explicit Start/Resume grant authority, and lets an operator ready or
/// skip action advance a review-required automatic session without any
/// browser timer issuing a process command.
pub(crate) fn schedule_automatic_coordinator(app: AppHandle) {
    let active_state = app.state::<ActiveSessionState>();
    let Ok((source, _)) = active_session_source(active_state.inner()) else {
        return;
    };
    let Ok(bundle) = BundleStore::new(&source).read_v3_checkpointed() else {
        return;
    };
    if bundle.session_state.lifecycle != SessionLifecycleV2::Running
        || !matches!(
            bundle.schedule.antenna_control,
            Some(AntennaControlPolicyV5::CommandControlled {
                invocation: AntennaControlInvocationPolicyV5::Automatic,
                ..
            })
        )
    {
        return;
    }
    if crate::conductor::transition::transition_plan(&bundle, next_intent(&bundle)).can_continue() {
        // The conductor's no-op coordinator owns unchanged-state continuation;
        // no controller worker is needed for this transition.
        return;
    }

    let controller_state = app.state::<AntennaControllerState>();
    let generation = {
        let Ok(mut runtime) = controller_state.runtime.lock() else {
            return;
        };
        if runtime.worker_running {
            return;
        }
        let Some(attached) = runtime.attached.as_ref().filter(|attached| {
            attached.source == source
                && attached.session_id == bundle.manifest.session_id
                && attached.armed
        }) else {
            runtime.automation_status = AutomationStatus::Blocked;
            return;
        };
        let generation = attached.generation;
        runtime.worker_running = true;
        runtime.automation_status = AutomationStatus::Waiting;
        generation
    };

    thread::spawn(move || run_worker(app, source, generation));
}

fn run_worker(app: AppHandle, source: std::path::PathBuf, generation: u64) {
    loop {
        if cancelled(&app, generation) {
            return;
        }
        let bundle = match BundleStore::new(&source).read_v3_checkpointed() {
            Ok(bundle) => bundle,
            Err(error) => {
                block(
                    &app,
                    generation,
                    "unknown",
                    "Automatic antenna control could not read the active checkpoint.",
                    &error.to_string(),
                );
                return;
            }
        };
        if bundle.session_state.lifecycle != SessionLifecycleV2::Running
            || !matches!(
                bundle.schedule.antenna_control,
                Some(AntennaControlPolicyV5::CommandControlled {
                    invocation: AntennaControlInvocationPolicyV5::Automatic,
                    ..
                })
            )
        {
            finish(&app, generation, AutomationStatus::Idle, None, false);
            return;
        }
        let manual_review_required = matches!(
            bundle.schedule.antenna_control,
            Some(AntennaControlPolicyV5::CommandControlled {
                manual_review_required: true,
                ..
            })
        );
        let Some(intent) = next_intent(&bundle).cloned() else {
            finish(&app, generation, AutomationStatus::Idle, None, false);
            return;
        };

        if let Some(summary) = committed_attempt(&bundle, &intent.intent_id) {
            let successful = summary.successful_switch
                && (manual_review_required || summary.successful_verification == Some(true));
            let transition = crate::conductor::transition::transition_plan(&bundle, Some(&intent));
            if successful && (manual_review_required || transition.operator_action_required) {
                finish(
                    &app,
                    generation,
                    AutomationStatus::AwaitingReview,
                    Some(summary),
                    false,
                );
            } else {
                finish(
                    &app,
                    generation,
                    AutomationStatus::Blocked,
                    Some(summary),
                    true,
                );
            }
            return;
        }

        let prior_end = project_wspr_run_v3(&bundle.schedule, &bundle.events)
            .cycles
            .last()
            .map(|cycle| cycle.window.transmission_ends_at);
        if let Some(deadline) = prior_end {
            set_status(&app, generation, AutomationStatus::Waiting);
            while Utc::now() < deadline {
                if cancelled(&app, generation) {
                    return;
                }
                thread::sleep(WAIT_POLL);
            }
        }

        // An unchanged trusted station state is a durable no-op transition.
        // Re-evaluate it after the protected transmission interval so an
        // automatic controller is never invoked merely to select the antenna
        // that is already continuously occupied.
        let active_state = app.state::<ActiveSessionState>();
        match persist_continued_readiness(
            active_state.inner(),
            &source,
            std::sync::Arc::new(SystemLivePersistenceHooks),
        ) {
            Ok(ContinuationOutcome::Continued) => continue,
            Ok(ContinuationOutcome::Waiting(_)) => continue,
            Ok(ContinuationOutcome::NotApplicable) => {
                if BundleStore::new(&source)
                    .read_v3_checkpointed()
                    .ok()
                    .and_then(|current| next_intent(&current).map(|value| value.intent_id.clone()))
                    .as_deref()
                    != Some(intent.intent_id.as_str())
                {
                    continue;
                }
            }
            Err(error) => {
                block(
                    &app,
                    generation,
                    &intent.intent_id,
                    "Automatic continued-readiness evidence could not be committed.",
                    &format!("{}: {}", error.message, error.detail),
                );
                return;
            }
        }

        let (association, profile) = match controller_context(&app, &source, generation) {
            Ok(context) => context,
            Err(error) => {
                block(
                    &app,
                    generation,
                    &intent.intent_id,
                    &error.message,
                    &error.detail,
                );
                return;
            }
        };
        let Some(target) = association.targets.get(&intent.antenna_label).cloned() else {
            block(
                &app,
                generation,
                &intent.intent_id,
                "The pending antenna has no local controller target.",
                "repair the local target mapping and attach the controller again",
            );
            return;
        };
        let context = match invocation_context(&bundle, &intent, target) {
            Ok(context) => context,
            Err(detail) => {
                block(
                    &app,
                    generation,
                    &intent.intent_id,
                    "The pending antenna intention cannot be expanded safely.",
                    &detail,
                );
                return;
            }
        };
        set_status(&app, generation, AutomationStatus::Running);
        let generation_state = app.state::<AntennaControllerState>().generation.clone();
        let cancellation = || generation_state.load(Ordering::SeqCst) != generation;
        let (switch, verification) = match execute_profile_attempt(&profile, context, &cancellation)
        {
            Ok(attempt) => attempt,
            Err(error) => {
                block(
                    &app,
                    generation,
                    &intent.intent_id,
                    "The controller command could not be expanded safely.",
                    &error.detail,
                );
                return;
            }
        };
        if cancelled(&app, generation) {
            return;
        }
        let switch_success = switch.disposition.is_exit_zero();
        let verification_success = verification
            .as_ref()
            .map(|invocation| invocation.disposition.is_exit_zero());
        let diagnostic = attempt_diagnostic(&switch, verification.as_ref());
        let switch_record_id = format!("rig-{}", Uuid::new_v4());
        let verification_record_id = verification
            .as_ref()
            .map(|_| format!("rig-{}", Uuid::new_v4()));
        let mut rig_records = vec![rig_record(
            &bundle,
            switch_record_id.clone(),
            switch.clone(),
        )];
        if let Some(invocation) = verification.clone() {
            rig_records.push(rig_record(
                &bundle,
                verification_record_id
                    .clone()
                    .expect("verification invocation has an identity"),
                invocation,
            ));
        }
        let mutation_id = format!("automatic-control-{}", Uuid::new_v4());
        let active_state = app.state::<ActiveSessionState>();
        let persist = persist_attempt(
            active_state.inner(),
            &source,
            &intent.intent_id,
            manual_review_required,
            switch_success,
            verification_success,
            switch_record_id,
            verification_record_id,
            verification
                .as_ref()
                .map(|invocation| invocation.completed_at),
            mutation_id,
            rig_records,
        );
        if let Err(error) = persist {
            block(
                &app,
                generation,
                &intent.intent_id,
                "Automatic antenna-control evidence could not be committed coherently.",
                &format!("{}: {}", error.message, error.detail),
            );
            return;
        }

        let detail = attempt_detail(switch_success, verification_success, manual_review_required);
        let summary = ControllerAttemptSummary {
            intent_id: intent.intent_id.clone(),
            successful_switch: switch_success,
            successful_verification: verification_success,
            detail: detail.into(),
            diagnostic,
        };
        if !switch_success || verification_success == Some(false) {
            finish(
                &app,
                generation,
                AutomationStatus::Blocked,
                Some(summary),
                true,
            );
            return;
        }
        if manual_review_required {
            finish(
                &app,
                generation,
                AutomationStatus::AwaitingReview,
                Some(summary),
                false,
            );
            return;
        }
        if verification_success != Some(true) {
            finish(
                &app,
                generation,
                AutomationStatus::Blocked,
                Some(summary),
                true,
            );
            return;
        }
        update_attempt(&app, generation, summary);
    }
}

#[allow(clippy::too_many_arguments)]
fn persist_attempt(
    active_state: &ActiveSessionState,
    source: &std::path::Path,
    intent_id: &str,
    manual_review_required: bool,
    switch_success: bool,
    verification_success: Option<bool>,
    switch_record_id: String,
    verification_record_id: Option<String>,
    ready_at: Option<chrono::DateTime<Utc>>,
    mutation_id: String,
    rig_records: Vec<antennabench_core::v3::RigRecordV3>,
) -> Result<(), SessionErrorPayload> {
    with_waiting_foreground_operation(active_state, || {
        let (active_source, _) = active_session_source(active_state)?;
        if active_source != source {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The active session changed while antenna control was running.",
                "automatic process evidence was not applied to a replacement session",
            ));
        }
        let store = BundleStore::new(source);
        let current = store.read_v3_checkpointed().map_err(live_error_payload)?;
        if current.session_state.lifecycle != SessionLifecycleV2::Running {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The session stopped while antenna control was running.",
                "automatic authority was revoked before evidence could be committed",
            ));
        }
        let current_intent = next_intent(&current);
        let run = project_wspr_run_v3(&current.schedule, &current.events);
        let initial_intent = run.cycles.is_empty()
            && run.skipped_intent_ids.is_empty()
            && current_intent.is_some_and(|intent| {
                current
                    .schedule
                    .wspr_cycle_intents
                    .first()
                    .is_some_and(|first| first.intent_id == intent.intent_id)
            });
        let transition_can_arm = current_intent.is_some_and(|intent| {
            crate::conductor::transition::transition_plan(&current, Some(intent))
                .automatic_verification_can_arm()
        });
        let armed_event = if !manual_review_required
            && switch_success
            && verification_success == Some(true)
            && current_intent.is_some_and(|intent| intent.intent_id == intent_id)
            && (initial_intent || transition_can_arm)
        {
            let intent = current_intent.expect("matching current intention exists");
            Some(command_verified_event(
                &current,
                intent,
                switch_record_id,
                verification_record_id.expect("successful verification has a record identity"),
                ready_at.expect("successful verification has a completion time"),
                format!("event-{}", Uuid::new_v4()),
            )?)
        } else {
            None
        };
        let mutation = LiveAntennaControlMutationV5 {
            expected_revision: current.session_state.revision,
            mutation_id: mutation_id.clone(),
            rig_records,
            armed_event,
        };
        let append = {
            let mut writer = crate::build_context::open_v3_writer_with_hooks(
                &store,
                std::sync::Arc::new(SystemLivePersistenceHooks),
            )
            .map_err(live_error_payload)?;
            writer.append_antenna_control(mutation)
        };
        if let Err(error) = append {
            let committed = store.read_v3_checkpointed().ok().is_some_and(|bundle| {
                bundle.session_state.last_committed_mutation_id.as_deref()
                    == Some(mutation_id.as_str())
            });
            if !committed {
                return Err(live_error_payload(error));
            }
        }
        let failure = if !switch_success {
            Some((
                DiagnosticOperationV6::AntennaControllerSwitch,
                "controller.switch_failed",
            ))
        } else if verification_success == Some(false) {
            Some((
                DiagnosticOperationV6::AntennaControllerVerify,
                "controller.verification_failed",
            ))
        } else {
            None
        };
        if let Some((operation, code)) = failure {
            let payload = SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "Antenna-controller evidence was retained, but readiness was not granted.",
                "the typed controller invocation remains in the session evidence",
            )
            .with_operation(code, "acquire");
            let _ = crate::operation_diagnostics::persist_failure(
                source,
                operation,
                DiagnosticPhaseV6::Acquire,
                code,
                EvidenceEffectV6::PrimaryEvidenceCommitted,
                vec![
                    DiagnosticTargetV6::Intent {
                        id: intent_id.into(),
                    },
                    DiagnosticTargetV6::Mutation {
                        id: mutation_id.clone(),
                    },
                ],
                payload,
            );
        }
        Ok(())
    })
}

fn controller_context(
    app: &AppHandle,
    source: &std::path::Path,
    generation: u64,
) -> Result<(RuntimeAssociation, ControllerProfile), SessionErrorPayload> {
    let controller_state = app.state::<AntennaControllerState>();
    let association = controller_state
        .runtime
        .lock()
        .map_err(|_| {
            SessionErrorPayload::report_pipeline("antenna-controller state is unavailable")
        })?
        .attached
        .as_ref()
        .filter(|attached| {
            attached.source == source && attached.armed && attached.generation == generation
        })
        .cloned()
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "Attach and arm the local controller before automatic switching.",
                "automatic authority is not active for this session",
            )
        })?;
    let catalog = read_catalog(&resolved_app_data_dir(app)?)?;
    let profile = catalog
        .profiles
        .into_iter()
        .find(|profile| {
            profile.profile_id == association.profile_id
                && profile.revision == association.profile_revision
        })
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The attached controller changed. Review and attach it again.",
                "the pinned local controller revision is stale",
            )
        })?;
    Ok((association, profile))
}

fn committed_attempt(
    bundle: &antennabench_core::v3::BundleV3Contents,
    intent_id: &str,
) -> Option<ControllerAttemptSummary> {
    let switch_record = bundle.rig.iter().rev().find(|record| {
        record.antenna_control.as_ref().is_some_and(|invocation| {
            invocation.role == AntennaControlRoleV5::Switch
                && invocation.context.intent_id == intent_id
        })
    })?;
    let mutation_id = &switch_record.meta.mutation.mutation_id;
    let switch = switch_record.antenna_control.as_ref()?;
    let verification = bundle.rig.iter().find_map(|record| {
        (record.meta.mutation.mutation_id == *mutation_id)
            .then_some(record.antenna_control.as_ref())
            .flatten()
            .filter(|invocation| invocation.role == AntennaControlRoleV5::Verification)
    });
    let switch_success = switch.disposition.is_exit_zero();
    let verification_success = verification.map(|item| item.disposition.is_exit_zero());
    Some(ControllerAttemptSummary {
        intent_id: intent_id.into(),
        successful_switch: switch_success,
        successful_verification: verification_success,
        detail: attempt_detail(switch_success, verification_success, true).into(),
        diagnostic: attempt_diagnostic(switch, verification),
    })
}

fn attempt_detail(
    switch_success: bool,
    verification_success: Option<bool>,
    manual_review_required: bool,
) -> &'static str {
    if !switch_success {
        "Switch did not exit successfully. No verification ran; automation is blocked without retry."
    } else if verification_success == Some(false) {
        "Switch exited successfully, but verification did not. Automation is blocked without retry."
    } else if verification_success == Some(true) && !manual_review_required {
        "Switch and verification exited successfully. Command verification armed the next eligible WSPR cycle."
    } else if verification_success == Some(true) {
        "Switch and verification exited successfully. Awaiting the named operator ready action."
    } else {
        "Switch exited successfully. No verification command is configured; awaiting operator review."
    }
}

fn cancelled(app: &AppHandle, generation: u64) -> bool {
    app.state::<AntennaControllerState>()
        .generation
        .load(Ordering::SeqCst)
        != generation
}

fn set_status(app: &AppHandle, generation: u64, status: AutomationStatus) {
    let controller_state = app.state::<AntennaControllerState>();
    if let Ok(mut runtime) = controller_state.runtime.lock() {
        if runtime
            .attached
            .as_ref()
            .is_some_and(|attached| attached.generation == generation)
        {
            runtime.automation_status = status;
        }
    };
}

fn update_attempt(app: &AppHandle, generation: u64, summary: ControllerAttemptSummary) {
    let controller_state = app.state::<AntennaControllerState>();
    if let Ok(mut runtime) = controller_state.runtime.lock() {
        if runtime
            .attached
            .as_ref()
            .is_some_and(|attached| attached.generation == generation)
        {
            runtime.last_attempt = Some(summary);
            runtime.automation_status = AutomationStatus::Waiting;
        }
    };
}

fn finish(
    app: &AppHandle,
    generation: u64,
    status: AutomationStatus,
    summary: Option<ControllerAttemptSummary>,
    disarm: bool,
) {
    let controller_state = app.state::<AntennaControllerState>();
    if let Ok(mut runtime) = controller_state.runtime.lock() {
        let current = runtime
            .attached
            .as_ref()
            .is_some_and(|attached| attached.generation == generation);
        if current {
            if let Some(summary) = summary {
                runtime.last_attempt = Some(summary);
            }
            if disarm {
                if let Some(attached) = runtime.attached.as_mut() {
                    attached.armed = false;
                }
                controller_state.generation.fetch_add(1, Ordering::SeqCst);
            }
            runtime.worker_running = false;
            runtime.automation_status = status;
        }
    };
}

fn block(app: &AppHandle, generation: u64, intent_id: &str, message: &str, diagnostic: &str) {
    finish(
        app,
        generation,
        AutomationStatus::Blocked,
        Some(ControllerAttemptSummary {
            intent_id: intent_id.into(),
            successful_switch: false,
            successful_verification: None,
            detail: message.into(),
            diagnostic: diagnostic.into(),
        }),
        true,
    );
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        sync::{atomic::AtomicUsize, mpsc},
        thread,
        time::Duration,
    };

    use antennabench_core::{
        v2::{EventTimeBasisV2, MutationMember, Provenance},
        v3::{OperatorEventPayloadV3, OperatorEventV3, RecordMetaV3},
        v5::{
            AntennaControlCommandV5, AntennaControlDispositionV5, AntennaControlInvocationV5,
            AntennaControlOutputEncodingV5, AntennaControlOutputV5, AntennaControlRoleV5,
            WsprReadinessBasisV5,
        },
        RecordSource,
    };
    use antennabench_storage::{BundleStore, LiveEventMutationV3};
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::{
        open_session::ActiveSessionState,
        setup::{create_e2e_controller_session, create_e2e_session},
    };

    fn invocation(
        bundle: &antennabench_core::v3::BundleV3Contents,
        intent: &antennabench_core::v3::WsprCycleIntentV3,
        role: AntennaControlRoleV5,
        completed_at: chrono::DateTime<Utc>,
        disposition: AntennaControlDispositionV5,
    ) -> AntennaControlInvocationV5 {
        AntennaControlInvocationV5 {
            role,
            controller_profile_name: "Fake controller".into(),
            controller_profile_revision: "revision-1".into(),
            command: AntennaControlCommandV5 {
                program_template: "controller".into(),
                argument_templates: vec!["{target}".into()],
                resolved_program: "controller".into(),
                resolved_arguments: vec!["relay-a".into()],
            },
            context: super::invocation_context(bundle, intent, "relay-a".into()).unwrap(),
            started_at: completed_at - chrono::Duration::milliseconds(25),
            completed_at,
            elapsed_milliseconds: 25,
            disposition,
            stdout: AntennaControlOutputV5 {
                encoding: AntennaControlOutputEncodingV5::Utf8,
                data: "ok".into(),
                truncated: false,
            },
            stderr: AntennaControlOutputV5 {
                encoding: AntennaControlOutputEncodingV5::Utf8,
                data: String::new(),
                truncated: false,
            },
        }
    }

    fn bundle() -> (tempfile::TempDir, antennabench_core::v3::BundleV3Contents) {
        let root = tempfile::tempdir().unwrap();
        let active = ActiveSessionState::default();
        let created = create_e2e_session(root.path(), &active);
        let bundle = BundleStore::new(created.path)
            .read_v3_checkpointed()
            .unwrap();
        (root, bundle)
    }

    fn running_bundle(
        root: &std::path::Path,
        active: &ActiveSessionState,
        occurred_at: chrono::DateTime<Utc>,
    ) -> (PathBuf, antennabench_core::v3::BundleV3Contents) {
        let created = create_e2e_controller_session(root, active);
        let store = BundleStore::new(&created.path);
        let mut writer = store.open_v3_writer().unwrap();
        let snapshot = writer.snapshot().clone();
        let mutation_id = "automatic-controller-start".to_string();
        writer
            .append_event(LiveEventMutationV3 {
                expected_revision: writer.checkpoint().revision,
                mutation_id: mutation_id.clone(),
                event: OperatorEventV3 {
                    meta: RecordMetaV3 {
                        schema_version: snapshot.manifest.schema_version,
                        session_id: snapshot.manifest.session_id,
                        recorded_at: occurred_at,
                        provenance: Provenance::from_legacy(
                            RecordSource::Operator,
                            env!("CARGO_PKG_VERSION"),
                        ),
                        mutation: MutationMember {
                            mutation_id,
                            member_index: 0,
                            member_count: 1,
                        },
                        runtime_context_id: None,
                    },
                    event_id: "automatic-controller-start-event".into(),
                    occurred_at,
                    time_basis: EventTimeBasisV2::ObservedNow,
                    uncertainty_seconds: None,
                    slot_id: None,
                    payload: OperatorEventPayloadV3::SessionStarted { note: None },
                },
            })
            .unwrap();
        drop(writer);
        let bundle = store.read_v3_checkpointed().unwrap();
        (created.path, bundle)
    }

    #[test]
    fn exact_reattach_preserves_the_attempt_needed_for_explicit_retry() {
        let state = AntennaControllerState::default();
        let profile = ControllerProfile {
            profile_id: "profile-1".into(),
            revision: "revision-1".into(),
            name: "Bench switch".into(),
            switch_command: super::super::profiles::ControllerCommandTemplate {
                program_template: "switch".into(),
                argument_templates: vec!["{target}".into()],
            },
            verification_command: None,
            timeout_seconds: 10,
        };
        let source = PathBuf::from("/tmp/session.antennabench");
        let targets = BTreeMap::from([("antenna-a".into(), "relay-a".into())]);
        state
            .attach(
                source.clone(),
                "session-1".into(),
                &profile,
                targets.clone(),
                false,
            )
            .unwrap();
        state.runtime.lock().unwrap().last_attempt = Some(ControllerAttemptSummary {
            intent_id: "intent-1".into(),
            successful_switch: false,
            successful_verification: None,
            detail: "blocked".into(),
            diagnostic: "exit 1".into(),
        });

        state
            .attach(source.clone(), "session-1".into(), &profile, targets, true)
            .unwrap();
        assert!(state.runtime.lock().unwrap().last_attempt.is_some());

        state
            .attach(
                source,
                "session-1".into(),
                &profile,
                BTreeMap::from([("antenna-a".into(), "relay-b".into())]),
                true,
            )
            .unwrap();
        assert!(state.runtime.lock().unwrap().last_attempt.is_none());
    }

    #[test]
    fn committed_success_pair_is_awaiting_review_instead_of_runnable_again() {
        let (_root, mut bundle) = bundle();
        let intent = bundle.schedule.wspr_cycle_intents[0].clone();
        let completed_at = Utc.with_ymd_and_hms(2026, 7, 18, 1, 0, 1).unwrap();
        let mut switch = super::rig_record(
            &bundle,
            "switch-record".into(),
            invocation(
                &bundle,
                &intent,
                AntennaControlRoleV5::Switch,
                completed_at,
                AntennaControlDispositionV5::Exit { code: 0 },
            ),
        );
        let mut verification = super::rig_record(
            &bundle,
            "verification-record".into(),
            invocation(
                &bundle,
                &intent,
                AntennaControlRoleV5::Verification,
                completed_at + chrono::Duration::milliseconds(25),
                AntennaControlDispositionV5::Exit { code: 0 },
            ),
        );
        switch.meta.mutation.mutation_id = "attempt-1".into();
        verification.meta.mutation.mutation_id = "attempt-1".into();
        bundle.rig.extend([switch, verification]);

        let summary = committed_attempt(&bundle, &intent.intent_id).unwrap();
        assert!(summary.successful_switch);
        assert_eq!(summary.successful_verification, Some(true));
        assert!(summary.detail.contains("Awaiting"));
    }

    #[test]
    fn captured_attempt_waits_for_admission_and_commits_once_without_rerunning() {
        let root = tempfile::tempdir().unwrap();
        let active = ActiveSessionState::default();
        let completed_at = Utc.with_ymd_and_hms(2026, 7, 18, 1, 0, 1).unwrap();
        let (path, bundle) = running_bundle(root.path(), &active, completed_at);
        let intent = bundle.schedule.wspr_cycle_intents[0].clone();
        let switch_record_id = "contended-switch-record".to_string();
        let verification_record_id = "contended-verification-record".to_string();
        let mutation_id = "contended-controller-attempt".to_string();
        let switch = rig_record(
            &bundle,
            switch_record_id.clone(),
            invocation(
                &bundle,
                &intent,
                AntennaControlRoleV5::Switch,
                completed_at,
                AntennaControlDispositionV5::Exit { code: 0 },
            ),
        );
        let verification = rig_record(
            &bundle,
            verification_record_id.clone(),
            invocation(
                &bundle,
                &intent,
                AntennaControlRoleV5::Verification,
                completed_at + chrono::Duration::milliseconds(25),
                AntennaControlDispositionV5::Exit { code: 0 },
            ),
        );
        let rig_records = vec![switch, verification];
        let process_invocations = AtomicUsize::new(0);

        thread::scope(|scope| {
            let (permit_held_tx, permit_held_rx) = mpsc::channel();
            let (release_permit_tx, release_permit_rx) = mpsc::channel();
            let active_for_holder = &active;
            scope.spawn(move || {
                crate::open_session::with_foreground_operation(active_for_holder, || {
                    permit_held_tx.send(()).unwrap();
                    release_permit_rx.recv().unwrap();
                    Ok(())
                })
                .unwrap();
            });
            permit_held_rx.recv().unwrap();
            let (started_tx, started_rx) = mpsc::channel();
            let (done_tx, done_rx) = mpsc::channel();
            let active_for_persist = &active;
            let path_for_persist = &path;
            let process_invocations_for_persist = &process_invocations;
            let intent_id_for_persist = intent.intent_id.clone();
            let switch_record_id_for_persist = switch_record_id.clone();
            let verification_record_id_for_persist = verification_record_id.clone();
            let mutation_id_for_persist = mutation_id.clone();
            let rig_records_for_persist = rig_records.clone();
            scope.spawn(move || {
                process_invocations_for_persist.fetch_add(1, Ordering::SeqCst);
                started_tx.send(()).unwrap();
                let result = persist_attempt(
                    active_for_persist,
                    path_for_persist,
                    &intent_id_for_persist,
                    false,
                    true,
                    Some(true),
                    switch_record_id_for_persist,
                    Some(verification_record_id_for_persist),
                    Some(completed_at + chrono::Duration::milliseconds(25)),
                    mutation_id_for_persist,
                    rig_records_for_persist,
                );
                done_tx.send(result).unwrap();
            });
            started_rx.recv().unwrap();
            assert!(done_rx.recv_timeout(Duration::from_millis(25)).is_err());
            release_permit_tx.send(()).unwrap();
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap();
        });

        assert_eq!(process_invocations.load(Ordering::SeqCst), 1);
        persist_attempt(
            &active,
            &path,
            &intent.intent_id,
            false,
            true,
            Some(true),
            switch_record_id,
            Some(verification_record_id),
            Some(completed_at + chrono::Duration::milliseconds(25)),
            mutation_id.clone(),
            rig_records,
        )
        .unwrap();
        let committed = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(
            committed
                .rig
                .iter()
                .filter(|record| record.meta.mutation.mutation_id == mutation_id)
                .count(),
            2
        );
        assert_eq!(
            committed
                .events
                .iter()
                .filter(|event| event.meta.mutation.mutation_id == mutation_id)
                .count(),
            1
        );
    }

    #[test]
    fn command_verified_event_uses_verification_completion_for_next_boundary() {
        let (_root, bundle) = bundle();
        let intent = &bundle.schedule.wspr_cycle_intents[0];
        let ready_at = Utc.with_ymd_and_hms(2026, 7, 18, 1, 59, 59).unwrap();
        let event = super::command_verified_event(
            &bundle,
            intent,
            "switch-record".into(),
            "verification-record".into(),
            ready_at,
            "armed-event".into(),
        )
        .unwrap();
        let OperatorEventPayloadV3::WsprCycleArmed {
            cycle_starts_at,
            readiness,
            ..
        } = event.payload
        else {
            panic!("expected armed event");
        };
        assert!(cycle_starts_at > ready_at);
        assert_eq!(cycle_starts_at.timestamp() % 120, 1);
        assert_eq!(
            readiness,
            Some(WsprReadinessBasisV5::CommandVerified {
                switch_record_id: "switch-record".into(),
                verification_record_id: "verification-record".into(),
            })
        );
    }
}
