use super::*;
use super::{
    commands::RunControllerOutcome, process::attempt_diagnostic, profiles::ControllerProfile,
};

#[derive(Default)]
pub(crate) struct AntennaControllerState {
    pub(super) runtime: Mutex<ControllerRuntime>,
    pub(super) generation: Arc<AtomicU64>,
    abort_coordination: Mutex<AbortCoordination>,
    abort_settled: Condvar,
}

#[derive(Default)]
struct AbortCoordination {
    next_attempt_id: u64,
    active: Option<InFlightAttempt>,
    settlement: Option<AttemptSettlement>,
}

struct InFlightAttempt {
    id: u64,
    source: PathBuf,
    session_id: String,
    mutation_id: String,
    abort_requested: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AttemptCommit {
    pub(crate) expected_revision: u64,
    pub(crate) committed_revision: u64,
    pub(crate) mutation_id: String,
}

#[derive(Clone)]
enum AttemptSettlement {
    Committed(AttemptCommit),
    Failed(SessionErrorPayload),
}

#[derive(Default)]
pub(super) struct ControllerRuntime {
    pub(super) attached: Option<RuntimeAssociation>,
    pub(super) last_attempt: Option<ControllerAttemptSummary>,
    pub(super) worker_running: bool,
    pub(super) automation_status: AutomationStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum AutomationStatus {
    #[default]
    Idle,
    Waiting,
    Running,
    AwaitingReview,
    Blocked,
}

impl AutomationStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::AwaitingReview => "awaiting_review",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeAssociation {
    pub(super) source: PathBuf,
    pub(super) session_id: String,
    pub(super) profile_id: String,
    pub(super) profile_revision: String,
    pub(super) targets: BTreeMap<String, String>,
    pub(super) armed: bool,
    pub(super) generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ControllerAttemptSummary {
    pub(super) intent_id: String,
    pub(super) successful_switch: bool,
    pub(super) successful_verification: Option<bool>,
    pub(super) detail: String,
    pub(super) diagnostic: String,
}
impl AntennaControllerState {
    pub(super) fn begin_attempt(
        &self,
        source: PathBuf,
        session_id: String,
        mutation_id: String,
    ) -> Result<u64, SessionErrorPayload> {
        let mut coordination = self.abort_coordination.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline(
                "antenna-controller abort coordination is unavailable",
            )
        })?;
        if coordination.active.is_some() {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Busy,
                "Another antenna-controller attempt is already running.",
                "only one Rust-owned controller process may be active",
            ));
        }
        coordination.next_attempt_id = coordination.next_attempt_id.wrapping_add(1);
        let id = coordination.next_attempt_id;
        coordination.settlement = None;
        coordination.active = Some(InFlightAttempt {
            id,
            source,
            session_id,
            mutation_id,
            abort_requested: false,
        });
        Ok(id)
    }

    pub(super) fn abort_requested(&self, attempt_id: u64) -> bool {
        self.abort_coordination
            .lock()
            .ok()
            .is_some_and(|coordination| {
                coordination
                    .active
                    .as_ref()
                    .is_some_and(|active| active.id == attempt_id && active.abort_requested)
            })
    }

    pub(super) fn settle_attempt(
        &self,
        attempt_id: u64,
        result: Result<AttemptCommit, SessionErrorPayload>,
    ) {
        if let Ok(mut coordination) = self.abort_coordination.lock() {
            let Some(active) = coordination.active.as_ref() else {
                return;
            };
            if active.id != attempt_id {
                return;
            }
            let settlement = match result {
                Ok(commit) if commit.mutation_id == active.mutation_id => {
                    AttemptSettlement::Committed(commit)
                }
                Ok(_) => AttemptSettlement::Failed(SessionErrorPayload::report_pipeline(
                    "antenna-controller persistence settlement did not match the active attempt",
                )),
                Err(payload) => AttemptSettlement::Failed(payload),
            };
            coordination.active = None;
            coordination.settlement = Some(settlement);
            self.abort_settled.notify_all();
        }
    }

    pub(crate) fn cancel_and_wait_for_abort(
        &self,
        source: &Path,
        session_id: &str,
    ) -> Result<Option<AttemptCommit>, SessionErrorPayload> {
        let mut coordination = self.abort_coordination.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline(
                "antenna-controller abort coordination is unavailable",
            )
        })?;
        let Some(active) = coordination
            .active
            .as_mut()
            .filter(|active| active.source == source && active.session_id == session_id)
        else {
            return Ok(None);
        };
        active.abort_requested = true;
        let attempt_id = active.id;
        self.generation.fetch_add(1, Ordering::SeqCst);
        while coordination
            .active
            .as_ref()
            .is_some_and(|active| active.id == attempt_id)
        {
            coordination = self.abort_settled.wait(coordination).map_err(|_| {
                SessionErrorPayload::report_pipeline(
                    "antenna-controller abort coordination is unavailable",
                )
            })?;
        }
        match coordination.settlement.take() {
            Some(AttemptSettlement::Committed(commit)) => Ok(Some(commit)),
            Some(AttemptSettlement::Failed(payload)) => Err(payload),
            None => Err(SessionErrorPayload::report_pipeline(
                "antenna-controller attempt ended without a persistence settlement",
            )),
        }
    }

    pub(crate) fn revoke(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.attached = None;
            runtime.last_attempt = None;
            runtime.worker_running = false;
            runtime.automation_status = AutomationStatus::Idle;
        }
    }

    pub(super) fn attach(
        &self,
        source: PathBuf,
        session_id: String,
        profile: &ControllerProfile,
        targets: BTreeMap<String, String>,
        armed: bool,
    ) -> Result<(), SessionErrorPayload> {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut runtime = self.runtime.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("antenna-controller state is unavailable")
        })?;
        let preserve_last_attempt = runtime.attached.as_ref().is_some_and(|attached| {
            attached.source == source
                && attached.session_id == session_id
                && attached.profile_id == profile.profile_id
                && attached.profile_revision == profile.revision
                && attached.targets == targets
        });
        runtime.attached = Some(RuntimeAssociation {
            source,
            session_id,
            profile_id: profile.profile_id.clone(),
            profile_revision: profile.revision.clone(),
            targets,
            armed,
            generation,
        });
        if !preserve_last_attempt {
            runtime.last_attempt = None;
        }
        runtime.worker_running = false;
        runtime.automation_status = AutomationStatus::Idle;
        Ok(())
    }
}

pub(super) fn next_intent(bundle: &BundleV3Contents) -> Option<&WsprCycleIntentV3> {
    let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    bundle.schedule.wspr_cycle_intents.iter().find(|intent| {
        !projection
            .cycles
            .iter()
            .any(|cycle| cycle.intent_id == intent.intent_id)
            && !projection
                .skipped_intent_ids
                .iter()
                .any(|intent_id| intent_id == &intent.intent_id)
    })
}

pub(super) fn ensure_prior_transmission_complete(
    bundle: &BundleV3Contents,
    now: DateTime<Utc>,
) -> Result<(), SessionErrorPayload> {
    let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    if projection
        .cycles
        .last()
        .is_some_and(|cycle| cycle.window.transmission_ends_at > now)
    {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "Wait for the current WSPR transmission interval to finish before switching.",
            "the antenna controller cannot run during the committed transmission interval",
        ));
    }
    Ok(())
}

pub(super) fn committed_outcome(
    bundle: &BundleV3Contents,
    mutation_id: &str,
    intent_id: &str,
) -> Option<RunControllerOutcome> {
    let invocations = bundle
        .rig
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation_id)
        .filter_map(|record| record.antenna_control.as_ref())
        .collect::<Vec<_>>();
    let switch = invocations
        .iter()
        .find(|invocation| invocation.role == AntennaControlRoleV5::Switch)?;
    if switch.context.intent_id != intent_id {
        return None;
    }
    let verification = invocations
        .iter()
        .find(|invocation| invocation.role == AntennaControlRoleV5::Verification);
    let switch_success = switch.disposition.is_exit_zero();
    let verification_success = verification.map(|invocation| invocation.disposition.is_exit_zero());
    let manual_ready_required = matches!(
        bundle.schedule.antenna_control,
        Some(AntennaControlPolicyV5::CommandControlled {
            manual_review_required: true,
            ..
        })
    );
    let detail = if !switch_success {
        "Switch did not exit successfully. No verification ran; manual operation remains available."
    } else if verification_success == Some(false) {
        "Switch exited successfully, but verification did not. Confirm hardware manually or retry explicitly."
    } else if verification_success == Some(true) && !manual_ready_required {
        "Switch and verification exited successfully. Command verification armed the next eligible WSPR cycle."
    } else if verification_success == Some(true) {
        "Switch and verification exited successfully. Operator readiness is still required."
    } else {
        "Switch exited successfully. No verification command is configured; operator readiness is required."
    };
    let diagnostic = attempt_diagnostic(switch, verification.copied());
    Some(RunControllerOutcome {
        revision: bundle.session_state.revision,
        intent_id: intent_id.into(),
        switch_disposition: switch.disposition.clone(),
        verification_disposition: verification.map(|invocation| invocation.disposition.clone()),
        verification_ran: verification.is_some(),
        manual_ready_required,
        detail: detail.into(),
        diagnostic,
    })
}

pub(super) fn command_verified_event(
    bundle: &BundleV3Contents,
    intent: &WsprCycleIntentV3,
    switch_record_id: String,
    verification_record_id: String,
    ready_at: DateTime<Utc>,
    event_id: String,
) -> Result<OperatorEventV3, SessionErrorPayload> {
    let cycle = next_wspr_cycle_after_ready(ready_at, Duration::seconds(1)).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The next WSPR cycle could not be calculated after verification.",
            error.to_string(),
        )
    })?;
    Ok(OperatorEventV3 {
        meta: RecordMetaV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: bundle.manifest.session_id.clone(),
            recorded_at: ready_at,
            provenance: Provenance::from_legacy(
                RecordSource::RigAdapter,
                "local-direct-process-v1",
            ),
            mutation: MutationMember {
                mutation_id: "pending".into(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        event_id,
        occurred_at: ready_at,
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: Some(intent.intent_id.clone()),
        payload: OperatorEventPayloadV3::WsprCycleArmed {
            antenna_label: intent.antenna_label.clone(),
            cycle_starts_at: cycle.starts_at,
            readiness: Some(WsprReadinessBasisV5::CommandVerified {
                switch_record_id,
                verification_record_id,
            }),
        },
    })
}
