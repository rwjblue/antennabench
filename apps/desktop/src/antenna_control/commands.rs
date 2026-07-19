use super::*;
use super::{policy::*, process::*, profiles::*};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveControllerView {
    policy: &'static str,
    invocation: Option<AntennaControlInvocationPolicyV5>,
    manual_review_required: Option<bool>,
    automation_status: &'static str,
    attached: bool,
    armed: bool,
    profile_id: Option<String>,
    profile_revision: Option<String>,
    profile_name: Option<String>,
    targets: BTreeMap<String, String>,
    stale_profile: bool,
    last_attempt: Option<ControllerAttemptSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachControllerRequest {
    profile_id: String,
    profile_revision: String,
    targets: Vec<ControllerTargetDraft>,
    armed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunControllerRequest {
    action_token: String,
    expected_revision: u64,
    intent_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunControllerOutcome {
    pub(super) revision: u64,
    pub(super) intent_id: String,
    pub(super) switch_disposition: AntennaControlDispositionV5,
    pub(super) verification_disposition: Option<AntennaControlDispositionV5>,
    pub(super) verification_ran: bool,
    pub(super) manual_ready_required: bool,
    pub(super) detail: String,
    pub(super) diagnostic: String,
}

#[tauri::command]
pub(crate) fn antenna_controller_profiles(
    app: AppHandle,
) -> Result<ControllerCatalogView, SessionErrorPayload> {
    let catalog = read_catalog(&resolved_app_data_dir(&app)?)?;
    let view = ControllerCatalogView {
        input_style: if cfg!(windows) {
            "structured"
        } else {
            "one_line"
        },
        profiles: catalog.profiles,
    };
    check_ipc_payload(&view, CONTROLLER_IPC_BYTES, "antenna_controller_profiles")?;
    Ok(view)
}

#[tauri::command]
pub(crate) fn save_antenna_controller_profile(
    app: AppHandle,
    controller_state: State<'_, AntennaControllerState>,
    draft: ControllerProfileDraft,
) -> Result<ControllerProfile, SessionErrorPayload> {
    let app_data_dir = resolved_app_data_dir(&app)?;
    let mut catalog = read_catalog(&app_data_dir)?;
    let existing = draft.profile_id.as_deref().and_then(|profile_id| {
        catalog
            .profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    });
    if draft.profile_id.is_some() && existing.is_none() {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The saved antenna-controller profile changed. Refresh before saving.",
            "the requested local profile identity no longer exists",
        ));
    }
    let profile = normalize_profile(&draft, existing, |prefix| {
        format!("{prefix}-{}", Uuid::new_v4())
    })
    .map_err(|detail| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The local antenna-controller profile is not valid.",
            detail,
        )
    })?;
    let changed = existing.is_none_or(|existing| existing.revision != profile.revision);
    catalog
        .profiles
        .retain(|candidate| candidate.profile_id != profile.profile_id);
    catalog.profiles.push(profile.clone());
    catalog.profiles.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.profile_id.cmp(&right.profile_id))
    });
    write_catalog(&app_data_dir, &catalog)?;
    if changed {
        let should_revoke = controller_state.runtime.lock().ok().is_some_and(|runtime| {
            runtime
                .attached
                .as_ref()
                .is_some_and(|attached| attached.profile_id == profile.profile_id)
        });
        if should_revoke {
            controller_state.revoke();
        }
    }
    Ok(profile)
}

#[tauri::command]
pub(crate) fn delete_antenna_controller_profile(
    app: AppHandle,
    controller_state: State<'_, AntennaControllerState>,
    profile_id: String,
) -> Result<(), SessionErrorPayload> {
    let app_data_dir = resolved_app_data_dir(&app)?;
    let mut catalog = read_catalog(&app_data_dir)?;
    if !remove_profile(&mut catalog, &profile_id) {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The saved antenna-controller profile no longer exists.",
            "refresh the local controller profile list before deleting",
        ));
    }
    write_catalog(&app_data_dir, &catalog)?;
    let should_revoke = controller_state.runtime.lock().ok().is_some_and(|runtime| {
        runtime
            .attached
            .as_ref()
            .is_some_and(|attached| attached.profile_id == profile_id)
    });
    if should_revoke {
        controller_state.revoke();
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn attach_active_session_antenna_controller(
    app: AppHandle,
    active_state: State<'_, ActiveSessionState>,
    controller_state: State<'_, AntennaControllerState>,
    request: AttachControllerRequest,
) -> Result<ActiveControllerView, SessionErrorPayload> {
    let (source, _) = active_session_source(active_state.inner())?;
    let bundle = BundleStore::new(&source)
        .read_v3_checkpointed()
        .map_err(live_error_payload)?;
    if bundle.manifest.schema_version < SCHEMA_VERSION_V5 {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "Antenna-controller profiles require a schema-v5 session.",
            format!("schema version {}", bundle.manifest.schema_version),
        ));
    }
    if !matches!(
        bundle.schedule.antenna_control.as_ref(),
        Some(AntennaControlPolicyV5::CommandControlled { .. })
    ) {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "This session was not planned for command-controlled antenna assistance.",
            "the portable antenna-control policy remains manual",
        ));
    }
    let catalog = read_catalog(&resolved_app_data_dir(&app)?)?;
    let profile = catalog
        .profiles
        .iter()
        .find(|profile| {
            profile.profile_id == request.profile_id && profile.revision == request.profile_revision
        })
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The selected controller profile changed. Review and attach it again.",
                "the requested local profile revision is unavailable",
            )
        })?;
    let targets = validate_targets(&bundle, &request.targets).map_err(|detail| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The antenna target mapping is not valid.",
            detail,
        )
    })?;
    let prepared = PreparedSetupController {
        profile: profile.clone(),
        targets: targets.clone(),
        arm_for_session: request.armed,
    };
    persist_profile_and_association(
        &resolved_app_data_dir(&app)?,
        &source,
        &bundle.manifest.session_id,
        &prepared,
    )?;
    controller_state.attach(
        source,
        bundle.manifest.session_id,
        profile,
        targets,
        request.armed,
    )?;
    active_controller_view_for(app, active_state, controller_state)
}

#[tauri::command]
pub(crate) fn active_session_antenna_controller(
    app: AppHandle,
    active_state: State<'_, ActiveSessionState>,
    controller_state: State<'_, AntennaControllerState>,
) -> Result<ActiveControllerView, SessionErrorPayload> {
    active_controller_view_for(app, active_state, controller_state)
}

pub(super) fn active_controller_view_for(
    app: AppHandle,
    active_state: State<'_, ActiveSessionState>,
    controller_state: State<'_, AntennaControllerState>,
) -> Result<ActiveControllerView, SessionErrorPayload> {
    let (source, _) = active_session_source(active_state.inner())?;
    let bundle = BundleStore::new(&source)
        .read_v3_checkpointed()
        .map_err(live_error_payload)?;
    let catalog = read_catalog(&resolved_app_data_dir(&app)?)?;
    let runtime = controller_state.runtime.lock().map_err(|_| {
        SessionErrorPayload::report_pipeline("antenna-controller state is unavailable")
    })?;
    let attached = runtime.attached.as_ref().filter(|attached| {
        attached.source == source && attached.session_id == bundle.manifest.session_id
    });
    let persisted = catalog.associations.iter().find(|association| {
        association.source == source.to_string_lossy()
            && association.session_id == bundle.manifest.session_id
    });
    let profile_id = attached
        .map(|attached| attached.profile_id.as_str())
        .or_else(|| persisted.map(|association| association.profile_id.as_str()));
    let profile_revision = attached
        .map(|attached| attached.profile_revision.as_str())
        .or_else(|| persisted.map(|association| association.profile_revision.as_str()));
    let profile = profile_id.and_then(|profile_id| {
        catalog
            .profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    });
    let stale_profile = profile_revision
        .is_some_and(|revision| profile.is_none_or(|profile| profile.revision != revision));
    let view = ActiveControllerView {
        policy: match bundle.schedule.antenna_control.as_ref() {
            Some(AntennaControlPolicyV5::CommandControlled { .. }) => "command_controlled",
            _ => "manual",
        },
        invocation: match bundle.schedule.antenna_control.as_ref() {
            Some(AntennaControlPolicyV5::CommandControlled { invocation, .. }) => Some(*invocation),
            _ => None,
        },
        manual_review_required: match bundle.schedule.antenna_control.as_ref() {
            Some(AntennaControlPolicyV5::CommandControlled {
                manual_review_required,
                ..
            }) => Some(*manual_review_required),
            _ => None,
        },
        automation_status: runtime.automation_status.as_str(),
        attached: attached.is_some(),
        armed: attached.is_some_and(|attached| attached.armed) && !stale_profile,
        profile_id: profile_id.map(str::to_string),
        profile_revision: profile_revision.map(str::to_string),
        profile_name: profile.map(|profile| profile.name.clone()),
        targets: attached.map_or_else(
            || persisted.map_or_else(BTreeMap::new, |association| association.targets.clone()),
            |attached| attached.targets.clone(),
        ),
        stale_profile,
        last_attempt: runtime.last_attempt.clone(),
    };
    check_ipc_payload(&view, CONTROLLER_IPC_BYTES, "active_antenna_controller")?;
    Ok(view)
}

#[tauri::command]
pub(crate) fn run_active_session_antenna_controller(
    active_state: State<'_, ActiveSessionState>,
    conductor_state: State<'_, ConductorSessionState>,
    controller_state: State<'_, AntennaControllerState>,
    app: AppHandle,
    request: RunControllerRequest,
) -> Result<RunControllerOutcome, SessionErrorPayload> {
    if controller_state
        .runtime
        .lock()
        .map_err(|_| {
            SessionErrorPayload::report_pipeline("antenna-controller state is unavailable")
        })?
        .worker_running
    {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Busy,
            "Automatic antenna control is already preparing this intention.",
            "wait for the current Rust-owned controller process to finish",
        ));
    }
    let coordinator_app = app.clone();
    let outcome = with_foreground_operation(active_state.inner(), || {
        let (source, _) = active_session_source(active_state.inner())?;
        let store = BundleStore::new(&source);
        let bundle = store.read_v3_checkpointed().map_err(live_error_payload)?;
        if bundle.manifest.schema_version < SCHEMA_VERSION_V5
            || bundle.session_state.lifecycle != SessionLifecycleV2::Running
        {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "Antenna control is available only for an active schema-v5 session.",
                "the active session is not a running schema-v5 session",
            ));
        }
        let manual_review_required = match bundle.schedule.antenna_control.as_ref() {
            Some(AntennaControlPolicyV5::CommandControlled {
                manual_review_required,
                ..
            }) => *manual_review_required,
            _ => {
                return Err(SessionErrorPayload::new(
                    SessionErrorKind::Conflict,
                    "This session was not planned for command-controlled antenna assistance.",
                    "the portable antenna-control policy is manual",
                ));
            }
        };
        if let Some(outcome) = committed_outcome(&bundle, &request.action_token, &request.intent_id)
        {
            return Ok(outcome);
        }
        if request.expected_revision != bundle.session_state.revision {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::StaleRevision,
                "The session changed. Refresh Active Run before switching.",
                format!(
                    "expected checkpoint revision {}, actual revision {}",
                    request.expected_revision, bundle.session_state.revision
                ),
            ));
        }
        ensure_prior_transmission_complete(&bundle, Utc::now())?;
        let intent = next_intent(&bundle).ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "There is no pending antenna intention to switch.",
                "all cycle intentions are armed or skipped",
            )
        })?;
        if intent.intent_id != request.intent_id {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The pending antenna intention changed. Refresh before switching.",
                format!(
                    "requested intention {:?}, current intention {:?}",
                    request.intent_id, intent.intent_id
                ),
            ));
        }
        let (association, generation) = {
            let runtime = controller_state.runtime.lock().map_err(|_| {
                SessionErrorPayload::report_pipeline("antenna-controller state is unavailable")
            })?;
            let association = runtime
                .attached
                .as_ref()
                .filter(|attached| {
                    attached.source == source
                        && attached.session_id == bundle.manifest.session_id
                        && attached.armed
                })
                .cloned()
                .ok_or_else(|| {
                    SessionErrorPayload::new(
                        SessionErrorKind::Conflict,
                        "Attach and arm a local controller before switching.",
                        "no armed local controller is associated with this active session",
                    )
                })?;
            let generation = association.generation;
            (association, generation)
        };
        let catalog = read_catalog(&resolved_app_data_dir(&app)?)?;
        let profile = catalog
            .profiles
            .iter()
            .find(|profile| {
                profile.profile_id == association.profile_id
                    && profile.revision == association.profile_revision
            })
            .cloned()
            .ok_or_else(|| {
                controller_state.revoke();
                SessionErrorPayload::new(
                    SessionErrorKind::Conflict,
                    "The attached controller changed. Review and arm it again.",
                    "the pinned local controller revision is stale",
                )
            })?;
        let target = association
            .targets
            .get(&intent.antenna_label)
            .cloned()
            .ok_or_else(|| {
                SessionErrorPayload::new(
                    SessionErrorKind::Validation,
                    "The pending antenna has no local controller target.",
                    format!("missing target for {:?}", intent.antenna_label),
                )
            })?;
        let context = invocation_context(&bundle, intent, target).map_err(|detail| {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The pending antenna intention cannot be expanded safely.",
                detail,
            )
        })?;
        resolve_command(&profile.switch_command, &context).map_err(|detail| {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The switch command could not be expanded.",
                detail,
            )
        })?;
        if let Some(verification) = &profile.verification_command {
            resolve_command(verification, &context).map_err(|detail| {
                SessionErrorPayload::new(
                    SessionErrorKind::Validation,
                    "The verification command could not be expanded.",
                    detail,
                )
            })?;
        }
        conductor_state.authorize_controller_action(
            &request.action_token,
            &bundle.manifest.session_id,
            request.expected_revision,
            Utc::now(),
        )?;
        let generation_state = controller_state.generation.clone();
        let cancelled = || generation_state.load(Ordering::SeqCst) != generation;
        let (switch, verification) = execute_profile_attempt(&profile, context, &cancelled)
            .map_err(|error| {
                let message = if error.role == AntennaControlRoleV5::Switch {
                    "The switch command could not be expanded."
                } else {
                    "The verification command could not be expanded."
                };
                SessionErrorPayload::new(SessionErrorKind::Validation, message, error.detail)
            })?;
        let switch_success = switch.disposition.is_exit_zero();
        let verification_success = verification
            .as_ref()
            .map(|invocation| invocation.disposition.is_exit_zero());
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
                    .expect("verification identity exists with invocation"),
                invocation,
            ));
        }
        let armed_event =
            if !manual_review_required && switch_success && verification_success == Some(true) {
                Some(command_verified_event(
                    &bundle,
                    intent,
                    switch_record_id,
                    verification_record_id
                        .clone()
                        .expect("successful verification has a record identity"),
                    verification
                        .as_ref()
                        .expect("successful verification has an invocation")
                        .completed_at,
                    format!("event-{}", Uuid::new_v4()),
                )?)
            } else {
                None
            };
        let receipt = {
            let mut writer = crate::build_context::open_v3_writer_with_hooks(
                &store,
                Arc::new(SystemLivePersistenceHooks),
            )
            .map_err(live_error_payload)?;
            writer
                .append_antenna_control(LiveAntennaControlMutationV5 {
                    expected_revision: request.expected_revision,
                    mutation_id: request.action_token.clone(),
                    rig_records,
                    armed_event,
                })
                .map_err(live_error_payload)?
        };
        let detail = if !switch_success {
            "Switch did not exit successfully. No verification ran; manual operation remains available."
        } else if verification_success == Some(false) {
            "Switch exited successfully, but verification did not. Confirm hardware manually or retry explicitly."
        } else if verification_success == Some(true) && !manual_review_required {
            "Switch and verification exited successfully. Command verification armed the next eligible WSPR cycle."
        } else if verification_success == Some(true) {
            "Switch and verification exited successfully. Operator readiness is still required."
        } else {
            "Switch exited successfully. No verification command is configured; operator readiness is required."
        }
        .to_string();
        let diagnostic = attempt_diagnostic(&switch, verification.as_ref());
        if let Ok(mut runtime) = controller_state.runtime.lock() {
            runtime.last_attempt = Some(ControllerAttemptSummary {
                intent_id: request.intent_id.clone(),
                successful_switch: switch_success,
                successful_verification: verification_success,
                detail: detail.clone(),
                diagnostic: diagnostic.clone(),
            });
        }
        Ok(RunControllerOutcome {
            revision: receipt.revision,
            intent_id: request.intent_id,
            switch_disposition: switch.disposition,
            verification_disposition: verification.map(|invocation| invocation.disposition),
            verification_ran: verification_success.is_some(),
            manual_ready_required: manual_review_required,
            detail,
            diagnostic,
        })
    })?;
    check_ipc_payload(
        &outcome,
        CONTROLLER_IPC_BYTES,
        "run_active_session_antenna_controller",
    )?;
    schedule_automatic_coordinator(coordinator_app);
    Ok(outcome)
}
