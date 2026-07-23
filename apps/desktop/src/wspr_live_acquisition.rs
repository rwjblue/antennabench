use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_core::{
    v2::{
        reduce_operator_events_v2, AdapterInput, AdapterRecordV2, BundleV2Contents,
        CorrectableOperatorEventPayloadV2, EventTimeBasisV2, MutationMember,
        OperatorEventPayloadV2, OperatorEventV2, Provenance, RecordMetaV2, SessionLifecycleV2,
    },
    v3::{
        project_wspr_run_v3, BundleV3Contents, OperatorEventPayloadV3, OperatorEventV3,
        RecordMetaV3,
    },
    v6::{
        DiagnosticCauseV6, DiagnosticDetailStateV6, DiagnosticDetailStatusV6,
        DiagnosticOperationV6, DiagnosticOutcomeV6, DiagnosticPhaseV6, DiagnosticRetryV6,
        DiagnosticSeverityV6, DiagnosticTargetV6, EvidenceEffectV6, OperationalDiagnosticV6,
        RetryDispositionV6, OPERATIONAL_DIAGNOSTIC_SCHEMA_V1,
    },
    PlannedSlot, RecordSource, SCHEMA_VERSION_V2, SCHEMA_VERSION_V3, SCHEMA_VERSION_V4,
    SCHEMA_VERSION_V5, SCHEMA_VERSION_V6,
};
use antennabench_storage::{
    BundleStore, LiveDiagnosticMutationV6, LiveEventMutationV3, LiveMutationMemberV2,
    LiveMutationV2, LivePersistenceError,
};
use antennabench_wsjtx::{
    latest_due_wspr_live_acquisition, plan_wspr_live_acquisitions_for_confirmed_slots,
    AdapterCancellationToken, ReqwestWsprLiveTransport, WsprLiveAcquirer,
    WsprLiveAcquisitionChannel, WsprLiveAcquisitionPlan, WsprLiveConfirmedCycle,
    WsprLiveHttpResponse, WsprLiveHttpTransport, WsprLiveImportConfig,
    WSPR_LIVE_MIN_REQUEST_INTERVAL_SECONDS, WSPR_LIVE_QUERY_ENDPOINT,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    open_session::{
        active_session_source, with_suspended_foreground_operation,
        with_waiting_foreground_operation, ActiveSessionState, OpenedSession, SessionErrorKind,
        SessionErrorPayload,
    },
    wsjtx_session::WsjtxSessionState,
    wspr_live_import::{
        commit_wspr_live_activity_response, commit_wspr_live_response,
        record_wspr_live_activity_failure, CommittedWsprLiveResponse,
    },
};

mod outcomes;

use outcomes::{
    automatic_capture_attempts, automatic_capture_counts, captured_outcome, failed_outcome,
    zero_evidence_outcome,
};

#[derive(Default)]
pub(crate) struct WsprLiveAcquisitionState(Mutex<WsprLiveAcquisitionRuntime>);

#[derive(Default)]
struct WsprLiveAcquisitionRuntime {
    source: Option<PathBuf>,
    last_request_started_at: Option<DateTime<Utc>>,
    failure: Option<WsprLiveFailure>,
}

#[derive(Debug, Clone)]
struct WsprLiveFailure {
    window_end: DateTime<Utc>,
    completed_slot_id: String,
    message: String,
    detail: String,
}

enum AcquisitionSnapshot {
    V2(BundleV2Contents),
    V3(BundleV3Contents),
}

impl AcquisitionSnapshot {
    fn revision(&self) -> u64 {
        match self {
            Self::V2(bundle) => bundle.session_state.revision,
            Self::V3(bundle) => bundle.session_state.revision,
        }
    }

    fn lifecycle(&self) -> SessionLifecycleV2 {
        match self {
            Self::V2(bundle) => bundle.session_state.lifecycle,
            Self::V3(bundle) => bundle.session_state.lifecycle,
        }
    }

    fn wspr_live_acquisition_enabled(&self) -> bool {
        match self {
            Self::V2(bundle) => bundle.session_state.wspr_live_acquisition_enabled,
            Self::V3(bundle) => bundle.session_state.wspr_live_acquisition_enabled,
        }
    }

    fn projected_slots(&self) -> Vec<PlannedSlot> {
        match self {
            Self::V2(bundle) => bundle.schedule.slots.clone(),
            Self::V3(bundle) => {
                let known_intents = bundle
                    .schedule
                    .wspr_cycle_intents
                    .iter()
                    .map(|intent| intent.intent_id.as_str())
                    .collect::<BTreeSet<_>>();
                let attributable = project_wspr_run_v3(&bundle.schedule, &bundle.events)
                    .cycles
                    .into_iter()
                    .filter(|cycle| {
                        cycle.occupancy_fully_covers_transmission
                            && known_intents.contains(cycle.intent_id.as_str())
                    })
                    .map(|cycle| cycle.intent_id)
                    .collect::<BTreeSet<_>>();
                bundle
                    .clone()
                    .into_current()
                    .bundle
                    .schedule
                    .slots
                    .into_iter()
                    .filter(|slot| attributable.contains(&slot.slot_id))
                    .collect()
            }
        }
    }

    fn final_completed_slot_id(&self) -> Option<String> {
        match self {
            Self::V2(bundle) => bundle
                .schedule
                .slots
                .last()
                .map(|slot| slot.slot_id.clone()),
            Self::V3(bundle) => {
                let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
                let consumed = projection.cycles.len() + projection.skipped_intent_ids.len();
                (consumed == bundle.schedule.wspr_cycle_intents.len())
                    .then(|| {
                        projection
                            .cycles
                            .iter()
                            .rev()
                            .find(|cycle| cycle.occupancy_fully_covers_transmission)
                            .map(|cycle| cycle.intent_id.clone())
                    })
                    .flatten()
            }
        }
    }

    fn confirmed_cycles(&self) -> Option<Vec<WsprLiveConfirmedCycle>> {
        match self {
            Self::V2(_) => None,
            Self::V3(bundle) if bundle.manifest.schema_version < SCHEMA_VERSION_V4 => None,
            Self::V3(bundle) => {
                let directions = bundle
                    .schedule
                    .wspr_cycle_intents
                    .iter()
                    .map(|intent| (intent.intent_id.as_str(), intent.direction))
                    .collect::<std::collections::BTreeMap<_, _>>();
                Some(
                    project_wspr_run_v3(&bundle.schedule, &bundle.events)
                        .cycles
                        .into_iter()
                        .filter(|cycle| cycle.occupancy_fully_covers_transmission)
                        .map(|cycle| WsprLiveConfirmedCycle {
                            slot_id: cycle.intent_id.clone(),
                            antenna_label: cycle.antenna_label,
                            starts_at: cycle.window.starts_at,
                            transmission_ends_at: cycle.window.transmission_ends_at,
                            band: cycle.band,
                            direction: directions.get(cycle.intent_id.as_str()).copied().flatten(),
                        })
                        .collect(),
                )
            }
        }
    }

    fn adapter_records(&self) -> &[AdapterRecordV2] {
        match self {
            Self::V2(bundle) => &bundle.adapter_records,
            Self::V3(bundle) => &bundle.adapter_records,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WsprLiveAcquisitionRequest {
    #[serde(default)]
    retry: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum WsprLiveAcquisitionOutcome {
    Disabled,
    Dormant {
        #[serde(rename = "capturedThrough")]
        captured_through: Option<DateTime<Utc>>,
    },
    Waiting {
        #[serde(rename = "completedSlotId")]
        completed_slot_id: String,
        #[serde(rename = "notBefore")]
        not_before: DateTime<Utc>,
        #[serde(rename = "capturedThrough")]
        captured_through: Option<DateTime<Utc>>,
    },
    UpToDate {
        #[serde(rename = "capturedThrough")]
        captured_through: DateTime<Utc>,
    },
    Completed {
        session: Box<OpenedSession>,
        revision: u64,
        #[serde(rename = "capturedThrough")]
        captured_through: DateTime<Utc>,
    },
    AwaitingAcknowledgement {
        session: Box<OpenedSession>,
        revision: u64,
        #[serde(rename = "completedSlotId")]
        completed_slot_id: String,
        #[serde(rename = "capturedThrough")]
        captured_through: DateTime<Utc>,
        #[serde(rename = "retryAvailable")]
        retry_available: bool,
        #[serde(rename = "successfulWindows")]
        successful_windows: usize,
        returned: usize,
        accepted: usize,
        filtered: usize,
        conflicted: usize,
        duplicated: usize,
        created: usize,
    },
    Failed {
        #[serde(rename = "completedSlotId")]
        completed_slot_id: String,
        #[serde(rename = "windowEnd")]
        window_end: DateTime<Utc>,
        message: String,
        detail: String,
    },
    Captured {
        session: Box<OpenedSession>,
        revision: u64,
        #[serde(rename = "completedSlotId")]
        completed_slot_id: String,
        #[serde(rename = "capturedThrough")]
        captured_through: DateTime<Utc>,
        #[serde(rename = "checkedAt")]
        checked_at: DateTime<Utc>,
        total: usize,
        accepted: usize,
        duplicate: usize,
        conflict: usize,
        #[serde(rename = "observationsCreated")]
        observations_created: usize,
    },
}

impl WsprLiveAcquisitionRuntime {
    fn reset_for_source(&mut self, source: &Path) {
        if self.source.as_deref() != Some(source) {
            *self = Self {
                source: Some(source.to_path_buf()),
                ..Self::default()
            };
        }
    }

    fn remember_failure(&mut self, plan: &WsprLiveAcquisitionPlan, error: &SessionErrorPayload) {
        self.failure = Some(WsprLiveFailure {
            window_end: plan.query.window_end,
            completed_slot_id: plan.completed_slot_id.clone(),
            message: error.message.clone(),
            detail: error.detail.clone(),
        });
    }
}

#[tauri::command]
pub(crate) fn advance_active_session_wspr_live(
    request: WsprLiveAcquisitionRequest,
    active_state: State<'_, ActiveSessionState>,
    acquisition_state: State<'_, WsprLiveAcquisitionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
) -> Result<WsprLiveAcquisitionOutcome, SessionErrorPayload> {
    let transport = ReqwestWsprLiveTransport::new().map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Resource,
            "The bounded WSPR.live client could not be initialized.",
            error.to_string(),
        )
    })?;
    let outcome = advance_with_transport(
        active_state.inner(),
        acquisition_state.inner(),
        request,
        Utc::now(),
        transport,
    )?;
    if matches!(
        &outcome,
        WsprLiveAcquisitionOutcome::Captured { session, .. }
            | WsprLiveAcquisitionOutcome::Completed { session, .. }
            if session.lifecycle == Some(SessionLifecycleV2::Ended)
    ) {
        let (source, _) = active_session_source(active_state.inner())?;
        wsjtx_state.stop_for_source(
            &source,
            "WSJT-X reception stopped after final WSPR.live acquisition completed the session.",
        );
    }
    Ok(outcome)
}

fn advance_with_transport<T: WsprLiveHttpTransport>(
    active_state: &ActiveSessionState,
    acquisition_state: &WsprLiveAcquisitionState,
    request: WsprLiveAcquisitionRequest,
    now: DateTime<Utc>,
    transport: T,
) -> Result<WsprLiveAcquisitionOutcome, SessionErrorPayload> {
    with_waiting_foreground_operation(active_state, || {
        let (source, _) = active_session_source(active_state)?;
        let store = BundleStore::new(&source);
        let schema_version = store
            .schema_version()
            .map_err(LivePersistenceError::from)
            .map_err(crate::conductor::live_error_payload)?;
        let snapshot = match schema_version {
            SCHEMA_VERSION_V2 => AcquisitionSnapshot::V2(
                store
                    .read_v2_checkpointed()
                    .map_err(crate::conductor::live_error_payload)?,
            ),
            SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6 => {
                AcquisitionSnapshot::V3(
                    store
                        .read_v3_checkpointed()
                        .map_err(crate::conductor::live_error_payload)?,
                )
            }
            actual => {
                return Err(SessionErrorPayload::new(
                    SessionErrorKind::Validation,
                    "This session format cannot acquire WSPR.live spots.",
                    format!("unsupported schema version {actual}"),
                ))
            }
        };
        if snapshot.lifecycle() != SessionLifecycleV2::Running {
            return Ok(WsprLiveAcquisitionOutcome::Dormant {
                captured_through: captured_through(&snapshot),
            });
        }
        if !snapshot.wspr_live_acquisition_enabled() {
            return Ok(WsprLiveAcquisitionOutcome::Disabled);
        }

        let plans = authorized_plans(&snapshot).map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The committed session cannot produce a WSPR.live acquisition plan.",
                error.to_string(),
            )
        })?;
        let captured_through = captured_through(&snapshot);
        let (last_request_started_at, prior_failure) = {
            let mut runtime = acquisition_state.0.lock().map_err(|_| {
                SessionErrorPayload::report_pipeline("WSPR.live acquisition state is unavailable")
            })?;
            runtime.reset_for_source(&source);
            (runtime.last_request_started_at, runtime.failure.clone())
        };
        let pending = plans
            .iter()
            .filter(|plan| captured_through.is_none_or(|end| plan.query.window_end > end))
            .cloned()
            .collect::<Vec<_>>();
        let mut pending = pending;
        let mut retry_final_plan = None;
        if pending.is_empty() && request.retry {
            if let Some(captured_through) = captured_through {
                if final_capture_is_complete(&snapshot, &plans, captured_through) {
                    if let Some(final_plan) = plans
                        .iter()
                        .rev()
                        .find(|plan| plan.query.window_end <= captured_through)
                    {
                        if automatic_capture_counts(&snapshot).created == 0
                            && automatic_capture_attempts(&snapshot, final_plan.query.window_end)
                                < 2
                        {
                            pending.push(final_plan.clone());
                            retry_final_plan = Some(final_plan.clone());
                        }
                    }
                }
            }
        }
        if pending.is_empty() {
            if let Some(captured_through) = captured_through {
                if final_capture_is_complete(&snapshot, &plans, captured_through) {
                    let final_plan = plans
                        .iter()
                        .rev()
                        .find(|plan| plan.query.window_end <= captured_through)
                        .expect("a complete final capture has an authorized plan");
                    if let Some(failure) = prior_failure.filter(|failure| {
                        failure.window_end == final_plan.query.window_end && !request.retry
                    }) {
                        return Ok(WsprLiveAcquisitionOutcome::Failed {
                            completed_slot_id: failure.completed_slot_id,
                            window_end: failure.window_end,
                            message: failure.message,
                            detail: failure.detail,
                        });
                    }
                    let counts = automatic_capture_counts(&snapshot);
                    if counts.created == 0 {
                        let revision = snapshot.revision();
                        let session = refresh_active_summary(active_state, &source, &snapshot)?;
                        return Ok(zero_evidence_outcome(
                            session,
                            revision,
                            final_plan,
                            captured_through,
                            counts,
                            automatic_capture_attempts(&snapshot, final_plan.query.window_end) < 2,
                        ));
                    }
                    return finish_final_capture(
                        active_state,
                        acquisition_state,
                        &source,
                        final_plan,
                        now,
                        captured_through,
                    );
                }
            }
            return Ok(captured_through.map_or(
                WsprLiveAcquisitionOutcome::Dormant {
                    captured_through: None,
                },
                |captured_through| WsprLiveAcquisitionOutcome::UpToDate { captured_through },
            ));
        }

        let due = retry_final_plan.or_else(|| {
            latest_due_wspr_live_acquisition(&pending, now, last_request_started_at).cloned()
        });
        let Some(plan) = due else {
            let next = pending
                .iter()
                .min_by_key(|plan| (plan.not_before, plan.segment_ended_at))
                .expect("pending acquisition is non-empty");
            let interval_deadline = last_request_started_at.and_then(|started| {
                started
                    .checked_add_signed(Duration::seconds(WSPR_LIVE_MIN_REQUEST_INTERVAL_SECONDS))
            });
            return Ok(WsprLiveAcquisitionOutcome::Waiting {
                completed_slot_id: next.completed_slot_id.clone(),
                not_before: interval_deadline
                    .map_or(next.not_before, |deadline| deadline.max(next.not_before)),
                captured_through,
            });
        };
        if let Some(failure) = prior_failure
            .filter(|failure| failure.window_end == plan.query.window_end && !request.retry)
        {
            return Ok(WsprLiveAcquisitionOutcome::Failed {
                completed_slot_id: failure.completed_slot_id,
                window_end: failure.window_end,
                message: failure.message,
                detail: failure.detail,
            });
        }

        {
            let mut runtime = acquisition_state.0.lock().map_err(|_| {
                SessionErrorPayload::report_pipeline("WSPR.live acquisition state is unavailable")
            })?;
            runtime.last_request_started_at = Some(now);
            runtime.failure = None;
        }
        let acquirer = WsprLiveAcquirer::new(transport);
        let requested_revision = snapshot.revision();
        let response = match with_suspended_foreground_operation(active_state, || {
            acquirer.acquire(&plan, &AdapterCancellationToken::default())
        })? {
            Ok(response) => response,
            Err(error) => {
                let payload = SessionErrorPayload::new(
                    SessionErrorKind::Resource,
                    "WSPR.live spots could not be fetched.",
                    error.to_string(),
                );
                acquisition_state
                    .0
                    .lock()
                    .map_err(|_| {
                        SessionErrorPayload::report_pipeline(
                            "WSPR.live acquisition state is unavailable",
                        )
                    })?
                    .remember_failure(&plan, &payload);
                return Ok(failed_outcome(&plan, payload));
            }
        };
        let mut snapshot = match revalidate_acquisition_response(
            active_state,
            &source,
            requested_revision,
            &plan,
        )? {
            RevalidatedAcquisition::Authorized { snapshot, .. } => *snapshot,
            RevalidatedAcquisition::Discarded { captured_through } => {
                return Ok(WsprLiveAcquisitionOutcome::Dormant { captured_through });
            }
        };
        let plans;
        let config = WsprLiveImportConfig {
            session_callsign: plan.query.session_callsign.clone(),
            window_start: plan.query.window_start,
            window_end: plan.query.window_end,
            selected_bands: snapshot
                .projected_slots()
                .iter()
                .filter(|slot| {
                    slot.starts_at
                        .checked_add_signed(Duration::seconds(i64::from(slot.duration_seconds)))
                        .is_some_and(|ends_at| ends_at <= plan.query.window_end)
                })
                .fold(Vec::new(), |mut bands, slot| {
                    if !bands.contains(&slot.band) {
                        bands.push(slot.band);
                    }
                    bands
                }),
            captured_at: response.received_at,
            source_locator: Some(WSPR_LIVE_QUERY_ENDPOINT.into()),
            confirmed_cycles: snapshot.confirmed_cycles(),
        };
        let mut committed = match commit_wspr_live_response(
            active_state,
            &source,
            &response.body,
            config.clone(),
            WsprLiveAcquisitionChannel::HttpsQuery,
        ) {
            Ok(committed) => committed,
            Err(error) => {
                let payload = SessionErrorPayload::new(
                    error.kind,
                    "The WSPR.live response could not be saved.",
                    error.detail,
                );
                acquisition_state
                    .0
                    .lock()
                    .map_err(|_| {
                        SessionErrorPayload::report_pipeline(
                            "WSPR.live acquisition state is unavailable",
                        )
                    })?
                    .remember_failure(&plan, &payload);
                return Ok(failed_outcome(&plan, payload));
            }
        };
        let cancellation = AdapterCancellationToken::default();
        let activity_response = with_suspended_foreground_operation(active_state, || {
            acquirer.acquire_activity_census(&plan, &cancellation)
        })?;
        match revalidate_acquisition_response(active_state, &source, snapshot.revision(), &plan)? {
            RevalidatedAcquisition::Authorized {
                snapshot: current_snapshot,
                plans: current_plans,
            } => {
                snapshot = *current_snapshot;
                plans = current_plans;
            }
            RevalidatedAcquisition::Discarded { .. } => {
                let refreshed = read_acquisition_snapshot(&source)?;
                committed.revision = refreshed.revision();
                committed.session = refresh_active_summary(active_state, &source, &refreshed)?;
                return Ok(captured_outcome(&plan, &response, committed));
            }
        }
        let mut activity_config = config.clone();
        activity_config.confirmed_cycles = snapshot.confirmed_cycles();
        match activity_response {
            Ok(activity_response) => {
                activity_config.captured_at = activity_response.received_at;
                match commit_wspr_live_activity_response(
                    active_state,
                    &source,
                    &activity_response.body,
                    &activity_config,
                ) {
                    Ok(activity) => {
                        committed.session = activity.session;
                        committed.revision = activity.revision;
                    }
                    Err(error) => {
                        if let Ok(activity) = record_wspr_live_activity_failure(
                            active_state,
                            &source,
                            &activity_config,
                            "wspr-live.activity-census-response-rejected",
                            &error.detail,
                        ) {
                            committed.session = activity.session;
                            committed.revision = activity.revision;
                        }
                    }
                }
            }
            Err(error) => {
                if let Ok(activity) = record_wspr_live_activity_failure(
                    active_state,
                    &source,
                    &activity_config,
                    "wspr-live.activity-census-query-failed",
                    &error.to_string(),
                ) {
                    committed.session = activity.session;
                    committed.revision = activity.revision;
                }
            }
        }
        acquisition_state
            .0
            .lock()
            .map_err(|_| {
                SessionErrorPayload::report_pipeline("WSPR.live acquisition state is unavailable")
            })?
            .failure = None;
        if final_capture_is_complete(&snapshot, &plans, plan.query.window_end) {
            let refreshed = read_acquisition_snapshot(&source)?;
            let counts = automatic_capture_counts(&refreshed);
            if counts.created == 0 {
                return Ok(zero_evidence_outcome(
                    committed.session,
                    committed.revision,
                    &plan,
                    plan.query.window_end,
                    counts,
                    automatic_capture_attempts(&refreshed, plan.query.window_end) < 2,
                ));
            }
            return match finish_final_capture(
                active_state,
                acquisition_state,
                &source,
                &plan,
                now,
                plan.query.window_end,
            )? {
                WsprLiveAcquisitionOutcome::Completed {
                    session,
                    revision,
                    captured_through,
                } => Ok(WsprLiveAcquisitionOutcome::Captured {
                    session,
                    revision,
                    completed_slot_id: plan.completed_slot_id,
                    captured_through,
                    checked_at: response.received_at,
                    total: committed.summary.total,
                    accepted: committed.summary.accepted,
                    duplicate: committed.summary.duplicate,
                    conflict: committed.summary.conflict,
                    observations_created: committed.summary.observations_created,
                }),
                failed @ WsprLiveAcquisitionOutcome::Failed { .. } => Ok(failed),
                _ => unreachable!("nonzero final capture returns completed or failed"),
            };
        }
        Ok(captured_outcome(&plan, &response, committed))
    })
}

enum RevalidatedAcquisition {
    Authorized {
        snapshot: Box<AcquisitionSnapshot>,
        plans: Vec<WsprLiveAcquisitionPlan>,
    },
    Discarded {
        captured_through: Option<DateTime<Utc>>,
    },
}

fn read_acquisition_snapshot(source: &Path) -> Result<AcquisitionSnapshot, SessionErrorPayload> {
    let store = BundleStore::new(source);
    match store
        .schema_version()
        .map_err(LivePersistenceError::from)
        .map_err(crate::conductor::live_error_payload)?
    {
        SCHEMA_VERSION_V2 => Ok(AcquisitionSnapshot::V2(
            store
                .read_v2_checkpointed()
                .map_err(crate::conductor::live_error_payload)?,
        )),
        SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6 => {
            Ok(AcquisitionSnapshot::V3(
                store
                    .read_v3_checkpointed()
                    .map_err(crate::conductor::live_error_payload)?,
            ))
        }
        actual => Err(SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "This session format cannot acquire WSPR.live spots.",
            format!("unsupported schema version {actual}"),
        )),
    }
}

fn revalidate_acquisition_response(
    active_state: &ActiveSessionState,
    source: &Path,
    requested_revision: u64,
    plan: &WsprLiveAcquisitionPlan,
) -> Result<RevalidatedAcquisition, SessionErrorPayload> {
    let (active_source, _) = active_session_source(active_state)?;
    if active_source != source {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The active session changed while WSPR.live was responding.",
            "the response was not applied to a replacement session",
        ));
    }
    let snapshot = read_acquisition_snapshot(source)?;
    let captured_through = captured_through(&snapshot);
    if snapshot.lifecycle() != SessionLifecycleV2::Running
        || !snapshot.wspr_live_acquisition_enabled()
    {
        return Ok(RevalidatedAcquisition::Discarded { captured_through });
    }
    if snapshot.revision() < requested_revision {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The session checkpoint regressed while WSPR.live was responding.",
            format!(
                "requested from revision {requested_revision}, current revision {}",
                snapshot.revision()
            ),
        ));
    }
    let plans = authorized_plans(&snapshot).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The committed session cannot produce a WSPR.live acquisition plan.",
            error.to_string(),
        )
    })?;
    let plan_is_still_authorized = plans.iter().any(|candidate| candidate == plan);
    if !plan_is_still_authorized {
        return Ok(RevalidatedAcquisition::Discarded { captured_through });
    }
    Ok(RevalidatedAcquisition::Authorized {
        snapshot: Box::new(snapshot),
        plans,
    })
}

fn refresh_active_summary(
    active_state: &ActiveSessionState,
    source: &Path,
    snapshot: &AcquisitionSnapshot,
) -> Result<OpenedSession, SessionErrorPayload> {
    match snapshot {
        AcquisitionSnapshot::V2(_) => {
            crate::open_session::reload_active_session(active_state, source)
        }
        AcquisitionSnapshot::V3(bundle) => {
            crate::open_session::update_active_session_live_projection(active_state, source, bundle)
        }
    }
}

fn finish_final_capture(
    active_state: &ActiveSessionState,
    acquisition_state: &WsprLiveAcquisitionState,
    source: &Path,
    plan: &WsprLiveAcquisitionPlan,
    occurred_at: DateTime<Utc>,
    captured_through: DateTime<Utc>,
) -> Result<WsprLiveAcquisitionOutcome, SessionErrorPayload> {
    match end_after_final_capture(active_state, source, occurred_at, captured_through) {
        Ok((session, revision)) => {
            acquisition_state
                .0
                .lock()
                .map_err(|_| {
                    SessionErrorPayload::report_pipeline(
                        "WSPR.live acquisition state is unavailable",
                    )
                })?
                .failure = None;
            Ok(WsprLiveAcquisitionOutcome::Completed {
                session: Box::new(session),
                revision,
                captured_through,
            })
        }
        Err(error) => {
            let payload = persist_finalization_failure(
                source,
                plan,
                captured_through,
                error.with_message(
                    "Public spots were saved, but the session could not end automatically.",
                ),
            );
            acquisition_state
                .0
                .lock()
                .map_err(|_| {
                    SessionErrorPayload::report_pipeline(
                        "WSPR.live acquisition state is unavailable",
                    )
                })?
                .remember_failure(plan, &payload);
            Ok(failed_outcome(plan, payload))
        }
    }
}

fn persist_finalization_failure(
    source: &Path,
    plan: &WsprLiveAcquisitionPlan,
    captured_through: DateTime<Utc>,
    payload: SessionErrorPayload,
) -> SessionErrorPayload {
    let store = BundleStore::new(source);
    let Ok(mut writer) = crate::build_context::open_v3_writer(&store) else {
        return payload.with_diagnostic_not_persisted("diagnostic.writer_unavailable");
    };
    if writer.snapshot().manifest.schema_version != SCHEMA_VERSION_V6 {
        return payload;
    }
    let attempt_id = format!(
        "wspr-finalize-{}-{}",
        plan.completed_slot_id,
        captured_through.timestamp()
    );
    if let Some(existing) = writer
        .snapshot()
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.attempt_id == attempt_id)
    {
        return payload.with_diagnostic_persisted(Some(existing.diagnostic_id.clone()));
    }
    let revision = writer.checkpoint().revision;
    let diagnostic = OperationalDiagnosticV6 {
        schema: OPERATIONAL_DIAGNOSTIC_SCHEMA_V1.into(),
        diagnostic_id: writer.allocate_id("diagnostic"),
        correlation_id: format!("wspr-window-{}", plan.query.window_end.timestamp()),
        attempt_id,
        mutation: MutationMember {
            mutation_id: "pending-diagnostic".into(),
            member_index: 0,
            member_count: 1,
        },
        runtime_context_id: String::new(),
        occurred_at: captured_through,
        operation: DiagnosticOperationV6::WsprLiveAcquisition,
        phase: DiagnosticPhaseV6::Finalize,
        code: "session.finalization_rejected".into(),
        summary: "WSPR.live evidence committed, but automatic session end failed.".into(),
        outcome: DiagnosticOutcomeV6::Partial,
        severity: DiagnosticSeverityV6::Error,
        revision_before: Some(revision),
        revision_after: Some(revision),
        diagnostic_revision: revision,
        evidence_effect: EvidenceEffectV6::PrimaryEvidenceCommitted,
        retry: DiagnosticRetryV6 {
            disposition: RetryDispositionV6::RequiresStateChange,
            guidance_code: "refresh_state_then_finalize".into(),
        },
        targets: vec![
            DiagnosticTargetV6::Source {
                id: "wspr-live".into(),
            },
            DiagnosticTargetV6::Slot {
                id: plan.completed_slot_id.clone(),
            },
            DiagnosticTargetV6::AcquisitionWindow {
                start: plan.query.window_start,
                end: plan.query.window_end,
            },
        ],
        causes: vec![DiagnosticCauseV6 {
            code: payload.operation.as_ref().map_or_else(
                || "session.finalization_rejected".into(),
                |detail| detail.code.clone(),
            ),
            phase: DiagnosticPhaseV6::Finalize,
            facts: Vec::new(),
        }],
        detail_status: DiagnosticDetailStatusV6 {
            state: DiagnosticDetailStateV6::Complete,
            omitted_fact_count: 0,
        },
    };
    let mutation_id = writer.allocate_id("diagnostic-mutation");
    let result = writer.append_diagnostic(LiveDiagnosticMutationV6 {
        expected_revision: revision,
        mutation_id,
        diagnostic,
    });
    match result {
        Ok(receipt) => payload.with_diagnostic_persisted(receipt.diagnostic_id),
        Err(_) => payload.with_diagnostic_not_persisted("diagnostic.persistence_failed"),
    }
}

fn final_capture_is_complete(
    bundle: &AcquisitionSnapshot,
    authorized_plans: &[WsprLiveAcquisitionPlan],
    captured_through: DateTime<Utc>,
) -> bool {
    let Some(final_completed_slot_id) = bundle.final_completed_slot_id() else {
        return false;
    };
    bundle.projected_slots().last().is_some_and(|final_slot| {
        final_slot.slot_id == final_completed_slot_id
            && authorized_plans
                .iter()
                .any(|plan| plan.completed_slot_id == final_slot.slot_id)
            && final_slot
                .starts_at
                .checked_add_signed(Duration::seconds(i64::from(final_slot.duration_seconds)))
                .is_some_and(|final_end| captured_through >= final_end)
    })
}

fn end_after_final_capture(
    active_state: &ActiveSessionState,
    source: &Path,
    occurred_at: DateTime<Utc>,
    captured_through: DateTime<Utc>,
) -> Result<(OpenedSession, u64), SessionErrorPayload> {
    let store = BundleStore::new(source);
    match store
        .schema_version()
        .map_err(LivePersistenceError::from)
        .map_err(crate::conductor::live_error_payload)?
    {
        SCHEMA_VERSION_V2 => {
            end_v2_after_final_capture(active_state, source, occurred_at, captured_through, &store)
        }
        SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6 => {
            end_v3_after_final_capture(active_state, source, occurred_at, captured_through, &store)
        }
        actual => Err(SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "This session format cannot be finalized automatically.",
            format!("unsupported schema version {actual}"),
        )),
    }
}

fn end_v2_after_final_capture(
    active_state: &ActiveSessionState,
    source: &Path,
    occurred_at: DateTime<Utc>,
    captured_through: DateTime<Utc>,
    store: &BundleStore,
) -> Result<(OpenedSession, u64), SessionErrorPayload> {
    let mut writer = store
        .open_v2_writer()
        .map_err(crate::conductor::live_error_payload)?;
    if writer.snapshot().session_state.lifecycle == SessionLifecycleV2::Ended {
        let revision = writer.checkpoint().revision;
        drop(writer);
        return Ok((
            crate::open_session::reload_active_session(active_state, source)?,
            revision,
        ));
    }
    if writer.snapshot().session_state.lifecycle != SessionLifecycleV2::Running {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The session changed before automatic finalization could commit.",
            format!(
                "current lifecycle: {:?}",
                writer.snapshot().session_state.lifecycle
            ),
        ));
    }
    let mutation_id = writer.allocate_id("mutation");
    let event_id = writer.allocate_id("event");
    let event = OperatorEventV2 {
        meta: RecordMetaV2 {
            schema_version: SCHEMA_VERSION_V2,
            session_id: writer.snapshot().manifest.session_id.clone(),
            recorded_at: occurred_at,
            provenance: Provenance::from_legacy(RecordSource::Derived, env!("CARGO_PKG_VERSION")),
            mutation: MutationMember {
                mutation_id: mutation_id.clone(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        event_id,
        occurred_at,
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: None,
        payload: OperatorEventPayloadV2::SessionEnded {
            reason: Some(format!(
                "Automatically ended after cumulative WSPR.live capture through {captured_through}."
            )),
        },
    };
    let receipt = writer
        .append(LiveMutationV2 {
            expected_revision: writer.checkpoint().revision,
            mutation_id,
            members: vec![LiveMutationMemberV2::Event(event)],
        })
        .map_err(crate::conductor::live_error_payload)?;
    drop(writer);
    Ok((
        crate::open_session::reload_active_session(active_state, source)?,
        receipt.revision,
    ))
}

fn end_v3_after_final_capture(
    active_state: &ActiveSessionState,
    source: &Path,
    occurred_at: DateTime<Utc>,
    captured_through: DateTime<Utc>,
    store: &BundleStore,
) -> Result<(OpenedSession, u64), SessionErrorPayload> {
    let mut writer = crate::build_context::open_v3_writer(store)
        .map_err(crate::conductor::live_error_payload)?;
    if writer.snapshot().session_state.lifecycle == SessionLifecycleV2::Ended {
        let revision = writer.checkpoint().revision;
        let projection = writer.snapshot().clone();
        drop(writer);
        return Ok((
            crate::open_session::update_active_session_live_projection(
                active_state,
                source,
                &projection,
            )?,
            revision,
        ));
    }
    if writer.snapshot().session_state.lifecycle != SessionLifecycleV2::Running {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The session changed before automatic finalization could commit.",
            format!(
                "current lifecycle: {:?}",
                writer.snapshot().session_state.lifecycle
            ),
        ));
    }
    let mutation_id = writer.allocate_id("mutation");
    let event = OperatorEventV3 {
        meta: RecordMetaV3 {
            schema_version: writer.snapshot().manifest.schema_version,
            session_id: writer.snapshot().manifest.session_id.clone(),
            recorded_at: occurred_at,
            provenance: Provenance::from_legacy(RecordSource::Derived, env!("CARGO_PKG_VERSION")),
            mutation: MutationMember {
                mutation_id: mutation_id.clone(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        event_id: writer.allocate_id("event"),
        occurred_at,
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: None,
        payload: OperatorEventPayloadV3::SessionEnded {
            reason: Some(format!(
                "Automatically ended after cumulative WSPR.live capture through {captured_through}."
            )),
        },
    };
    let receipt = writer
        .append_event(LiveEventMutationV3 {
            expected_revision: writer.checkpoint().revision,
            mutation_id,
            event,
        })
        .map_err(crate::conductor::live_error_payload)?;
    let projection = writer.snapshot().clone();
    drop(writer);
    Ok((
        crate::open_session::update_active_session_live_projection(
            active_state,
            source,
            &projection,
        )?,
        receipt.revision,
    ))
}

fn authorized_plans(
    bundle: &AcquisitionSnapshot,
) -> Result<Vec<WsprLiveAcquisitionPlan>, antennabench_wsjtx::WsprLiveImportError> {
    let (callsign, confirmed_slot_ids) = match bundle {
        AcquisitionSnapshot::V2(bundle) => (
            bundle.station.callsign.as_str(),
            reduce_operator_events_v2(SessionLifecycleV2::Ready, &bundle.events)
                .effective_events
                .into_iter()
                .filter_map(|event| {
                    matches!(
                        event.payload,
                        CorrectableOperatorEventPayloadV2::AntennaStateConfirmed { .. }
                    )
                    .then_some(event.slot_id)
                    .flatten()
                })
                .collect::<BTreeSet<_>>(),
        ),
        AcquisitionSnapshot::V3(bundle) => (
            bundle.station.callsign.as_str(),
            project_wspr_run_v3(&bundle.schedule, &bundle.events)
                .cycles
                .into_iter()
                .filter(|cycle| cycle.occupancy_fully_covers_transmission)
                .map(|cycle| cycle.intent_id)
                .collect::<BTreeSet<_>>(),
        ),
    };
    let slots = bundle.projected_slots();
    if slots.is_empty() {
        return Ok(Vec::new());
    }
    plan_wspr_live_acquisitions_for_confirmed_slots(callsign, &slots, &confirmed_slot_ids)
}

fn captured_through(bundle: &AcquisitionSnapshot) -> Option<DateTime<Utc>> {
    bundle
        .adapter_records()
        .iter()
        .filter(|record| record.record_type == "wspr_live_import_summary")
        .filter_map(|record| {
            let AdapterInput::Inline { data, .. } = &record.input else {
                return None;
            };
            let summary = serde_json::from_str::<serde_json::Value>(data).ok()?;
            let window_end = summary
                .get("window_end")?
                .as_str()?
                .parse::<DateTime<Utc>>()
                .ok()?;
            let is_automatic = summary
                .get("acquisition_channel")
                .and_then(serde_json::Value::as_str)
                == Some("https-query");
            let is_mature_manual_recovery = summary
                .get("captured_at")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| value.parse::<DateTime<Utc>>().ok())
                .and_then(|captured_at| {
                    window_end
                        .checked_add_signed(Duration::minutes(5))
                        .map(|mature_at| captured_at >= mature_at)
                })
                .unwrap_or(false);
            (is_automatic || is_mature_manual_recovery).then_some(window_end)
        })
        .max()
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, fs, path::Path};

    use antennabench_core::{
        v2::{EventTimeBasisV2, MutationMember, Provenance, SessionLifecycleV2},
        v3::{OperatorEventPayloadV3, OperatorEventV3, RecordMetaV3, WsprCycleDirection},
        v6::{
            DiagnosticFactValueV6, DiagnosticOperationV6, DiagnosticOutcomeV6, DiagnosticPhaseV6,
            DiagnosticTargetV6, EvidenceEffectV6, RetryDispositionV6,
        },
        RecordSource,
    };
    use antennabench_storage::{BundleStore, LiveEventMutationV3};
    use antennabench_wsjtx::{
        AdapterCancellationToken, WsprLiveAcquisitionChannel, WsprLiveAcquisitionError,
        WsprLiveHttpResponse, WsprLiveHttpTransport, WsprLiveImportConfig,
        WSPR_LIVE_ACTIVITY_COLUMNS, WSPR_LIVE_COLUMNS,
    };
    use chrono::{DateTime, Utc};
    use serde_json::json;
    use tempfile::TempDir;

    use super::{
        advance_with_transport, authorized_plans, commit_wspr_live_response, finish_final_capture,
        AcquisitionSnapshot, WsprLiveAcquisitionOutcome, WsprLiveAcquisitionRequest,
        WsprLiveAcquisitionState,
    };
    use crate::{
        open_session::{ActiveSessionState, SessionErrorKind, SessionErrorPayload},
        setup::create_e2e_session,
        wspr_live_import::persist_wspr_live_commit_failure,
    };

    struct FakeTransport {
        calls: Cell<usize>,
        response: Result<WsprLiveHttpResponse, WsprLiveAcquisitionError>,
    }

    struct CensusFailingTransport {
        calls: Cell<usize>,
        received_at: DateTime<Utc>,
    }

    struct SequencedEmptyTransport {
        calls: Cell<usize>,
        first_received_at: DateTime<Utc>,
    }

    struct MutatingTransport<F> {
        calls: Cell<usize>,
        received_at: DateTime<Utc>,
        mutate: F,
    }

    impl WsprLiveHttpTransport for &CensusFailingTransport {
        fn get(
            &self,
            url: &str,
            _body_limit: u64,
            _cancellation: &AdapterCancellationToken,
        ) -> Result<WsprLiveHttpResponse, WsprLiveAcquisitionError> {
            self.calls.set(self.calls.get() + 1);
            if url.contains("uniqExact") {
                Err(WsprLiveAcquisitionError::Transport(
                    "activity endpoint unavailable".into(),
                ))
            } else {
                Ok(empty_response(self.received_at))
            }
        }
    }

    impl WsprLiveHttpTransport for &FakeTransport {
        fn get(
            &self,
            url: &str,
            _body_limit: u64,
            _cancellation: &AdapterCancellationToken,
        ) -> Result<WsprLiveHttpResponse, WsprLiveAcquisitionError> {
            self.calls.set(self.calls.get() + 1);
            let response = self.response.clone();
            if url.contains("uniqExact") {
                response.map(|response| empty_activity_response(response.received_at))
            } else {
                response
            }
        }
    }

    impl WsprLiveHttpTransport for &SequencedEmptyTransport {
        fn get(
            &self,
            url: &str,
            _body_limit: u64,
            _cancellation: &AdapterCancellationToken,
        ) -> Result<WsprLiveHttpResponse, WsprLiveAcquisitionError> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            let received_at = self.first_received_at
                + chrono::Duration::seconds(i64::try_from(call / 2).unwrap());
            Ok(if url.contains("uniqExact") {
                empty_activity_response(received_at)
            } else {
                empty_response(received_at)
            })
        }
    }

    impl<F: Fn()> WsprLiveHttpTransport for &MutatingTransport<F> {
        fn get(
            &self,
            url: &str,
            _body_limit: u64,
            _cancellation: &AdapterCancellationToken,
        ) -> Result<WsprLiveHttpResponse, WsprLiveAcquisitionError> {
            let calls = self.calls.get();
            self.calls.set(calls + 1);
            if calls == 0 {
                (self.mutate)();
            }
            Ok(if url.contains("uniqExact") {
                empty_activity_response(self.received_at)
            } else {
                empty_response(self.received_at)
            })
        }
    }

    fn empty_response(received_at: DateTime<Utc>) -> WsprLiveHttpResponse {
        WsprLiveHttpResponse {
            received_at,
            status: 200,
            body: serde_json::to_vec(&json!({
                "meta": WSPR_LIVE_COLUMNS.map(|name| json!({
                    "name": name,
                    "type": "Synthetic",
                })),
                "data": [],
                "rows": 0,
            }))
            .unwrap(),
        }
    }

    fn empty_activity_response(received_at: DateTime<Utc>) -> WsprLiveHttpResponse {
        WsprLiveHttpResponse {
            received_at,
            status: 200,
            body: serde_json::to_vec(&json!({
                "meta": WSPR_LIVE_ACTIVITY_COLUMNS.map(|name| json!({
                    "name": name,
                    "type": "Synthetic",
                })),
                "data": [],
                "rows": 0,
            }))
            .unwrap(),
        }
    }

    fn confirmed_receive_response(received_at: DateTime<Utc>) -> WsprLiveHttpResponse {
        WsprLiveHttpResponse {
            received_at,
            status: 200,
            body: serde_json::to_vec(&json!({
                "meta": WSPR_LIVE_COLUMNS.map(|name| json!({
                    "name": name,
                    "type": "Synthetic",
                })),
                "data": [{
                    "id": "91001",
                    "time": "2026-07-15 20:00:00",
                    "band": 14,
                    "rx_sign": "N1RWJ",
                    "rx_loc": "FN42",
                    "tx_sign": "K1ABC",
                    "tx_loc": "EM12",
                    "distance": 2450,
                    "azimuth": 252,
                    "rx_azimuth": 65,
                    "frequency": "14095600",
                    "power": 37,
                    "snr": -18,
                    "drift": 1,
                    "version": "2.6.1",
                    "code": 1
                }],
                "rows": 1,
            }))
            .unwrap(),
        }
    }

    fn large_confirmed_receive_response(received_at: DateTime<Utc>) -> WsprLiveHttpResponse {
        let rows = (0..400)
            .map(|index| {
                json!({
                    "id": 92000 + index,
                    "time": "2026-07-15 20:00:00",
                    "band": 14,
                    "rx_sign": "N1RWJ",
                    "rx_loc": "FN42",
                    "tx_sign": format!("K1A{index:03}"),
                    "tx_loc": "EM12",
                    "distance": 2450,
                    "azimuth": 252,
                    "rx_azimuth": 65,
                    "frequency": "14095600",
                    "power": 37,
                    "snr": -18,
                    "drift": 1,
                    "version": "2.6.1",
                    "code": 1
                })
            })
            .collect::<Vec<_>>();
        WsprLiveHttpResponse {
            received_at,
            status: 200,
            body: serde_json::to_vec(&json!({
                "meta": WSPR_LIVE_COLUMNS.map(|name| json!({
                    "name": name,
                    "type": "Synthetic",
                })),
                "data": rows,
                "rows": 400,
            }))
            .unwrap(),
        }
    }

    fn running_confirmed_session(
        root: &Path,
        wspr_live_acquisition_enabled: bool,
    ) -> (ActiveSessionState, std::path::PathBuf, DateTime<Utc>) {
        let active = ActiveSessionState::default();
        let created = create_e2e_session(root, &active);
        let store = BundleStore::new(&created.path);
        let mut bundle = store.read_v3().unwrap();
        bundle.session_state.wspr_live_acquisition_enabled = wspr_live_acquisition_enabled;
        fs::write(
            store.root().join("session-state.json"),
            serde_json::to_vec_pretty(&bundle.session_state).unwrap(),
        )
        .unwrap();
        let mut writer = store.open_v3_writer().unwrap();
        let snapshot = writer.snapshot().clone();
        let first_cycle: DateTime<Utc> = "2026-07-15T20:00:01Z".parse().unwrap();
        let final_cycle = first_cycle
            + chrono::Duration::seconds(
                120 * i64::try_from(snapshot.schedule.wspr_cycle_intents.len() - 1).unwrap(),
            );
        let final_end = final_cycle + chrono::Duration::seconds(120);
        let mut actions = vec![(
            None,
            first_cycle - chrono::Duration::seconds(30),
            OperatorEventPayloadV3::SessionStarted { note: None },
        )];
        actions.extend(snapshot.schedule.wspr_cycle_intents.iter().enumerate().map(
            |(index, intent)| {
                let cycle_starts_at =
                    first_cycle + chrono::Duration::seconds(120 * i64::try_from(index).unwrap());
                (
                    Some(intent.intent_id.clone()),
                    cycle_starts_at - chrono::Duration::seconds(1),
                    OperatorEventPayloadV3::WsprCycleArmed {
                        antenna_label: intent.antenna_label.clone(),
                        cycle_starts_at,
                        readiness: Some(
                            antennabench_core::v5::WsprReadinessBasisV5::OperatorConfirmed,
                        ),
                    },
                )
            },
        ));
        for (index, (slot_id, occurred_at, payload)) in actions.into_iter().enumerate() {
            let mutation_id = format!("automatic-test-mutation-{index}");
            let event = OperatorEventV3 {
                meta: RecordMetaV3 {
                    schema_version: snapshot.manifest.schema_version,
                    session_id: snapshot.manifest.session_id.clone(),
                    recorded_at: occurred_at,
                    provenance: Provenance::from_legacy(
                        RecordSource::Operator,
                        env!("CARGO_PKG_VERSION"),
                    ),
                    mutation: MutationMember {
                        mutation_id: mutation_id.clone(),
                        member_index: 0,
                        member_count: 1,
                    },
                    runtime_context_id: None,
                },
                event_id: format!("automatic-test-event-{index}"),
                occurred_at,
                time_basis: EventTimeBasisV2::ObservedNow,
                uncertainty_seconds: None,
                slot_id,
                payload,
            };
            writer
                .append_event(LiveEventMutationV3 {
                    expected_revision: writer.checkpoint().revision,
                    mutation_id,
                    event,
                })
                .unwrap();
        }
        drop(writer);
        (
            active,
            created.path,
            final_end + chrono::Duration::minutes(5),
        )
    }

    fn commit_final_response(
        active: &ActiveSessionState,
        path: &Path,
        captured_at: DateTime<Utc>,
    ) -> super::WsprLiveAcquisitionPlan {
        let bundle = BundleStore::new(path).read_v3_checkpointed().unwrap();
        let snapshot = AcquisitionSnapshot::V3(bundle);
        let plan = authorized_plans(&snapshot).unwrap().pop().unwrap();
        let response = empty_response(captured_at);
        commit_wspr_live_response(
            active,
            path,
            &response.body,
            WsprLiveImportConfig {
                session_callsign: plan.query.session_callsign.clone(),
                window_start: plan.query.window_start,
                window_end: plan.query.window_end,
                selected_bands: snapshot.projected_slots().into_iter().fold(
                    Vec::new(),
                    |mut bands, slot| {
                        if !bands.contains(&slot.band) {
                            bands.push(slot.band);
                        }
                        bands
                    },
                ),
                captured_at,
                source_locator: None,
                confirmed_cycles: snapshot.confirmed_cycles(),
            },
            WsprLiveAcquisitionChannel::HttpsQuery,
        )
        .unwrap();
        plan
    }

    fn end_session_during_acquisition(path: &Path, occurred_at: DateTime<Utc>) {
        let store = BundleStore::new(path);
        let mut writer = store.open_v3_writer().unwrap();
        let snapshot = writer.snapshot().clone();
        let mutation_id = "operator-end-during-acquisition".to_string();
        let event = OperatorEventV3 {
            meta: RecordMetaV3 {
                schema_version: snapshot.manifest.schema_version,
                session_id: snapshot.manifest.session_id,
                recorded_at: occurred_at,
                provenance: Provenance::from_legacy(
                    RecordSource::Operator,
                    env!("CARGO_PKG_VERSION"),
                ),
                mutation: MutationMember {
                    mutation_id: mutation_id.clone(),
                    member_index: 0,
                    member_count: 1,
                },
                runtime_context_id: None,
            },
            event_id: "operator-end-event-during-acquisition".into(),
            occurred_at,
            time_basis: EventTimeBasisV2::ObservedNow,
            uncertainty_seconds: None,
            slot_id: None,
            payload: OperatorEventPayloadV3::SessionEnded {
                reason: Some("Operator ended the run while public spots were loading.".into()),
            },
        };
        writer
            .append_event(LiveEventMutationV3 {
                expected_revision: writer.checkpoint().revision,
                mutation_id,
                event,
            })
            .unwrap();
    }

    #[test]
    fn final_public_capture_includes_confirmed_receive_and_transmit_cycles() {
        let temp = TempDir::new().unwrap();
        let active = ActiveSessionState::default();
        let created = create_e2e_session(temp.path(), &active);
        let mut bundle = BundleStore::new(created.path)
            .read_v3_checkpointed()
            .unwrap();
        let first_cycle: DateTime<Utc> = "2026-07-15T20:00:01Z".parse().unwrap();
        let mut actions = vec![(
            None,
            first_cycle - chrono::Duration::seconds(30),
            OperatorEventPayloadV3::SessionStarted { note: None },
        )];
        actions.extend(
            bundle
                .schedule
                .wspr_cycle_intents
                .iter()
                .take(3)
                .enumerate()
                .map(|(index, intent)| {
                    let cycle_starts_at = first_cycle
                        + chrono::Duration::seconds(120 * i64::try_from(index).unwrap());
                    (
                        Some(intent.intent_id.clone()),
                        cycle_starts_at - chrono::Duration::seconds(1),
                        OperatorEventPayloadV3::WsprCycleArmed {
                            antenna_label: intent.antenna_label.clone(),
                            cycle_starts_at,
                            readiness: Some(
                                antennabench_core::v5::WsprReadinessBasisV5::OperatorConfirmed,
                            ),
                        },
                    )
                }),
        );
        actions.extend(
            bundle
                .schedule
                .wspr_cycle_intents
                .iter()
                .skip(3)
                .enumerate()
                .map(|(index, intent)| {
                    (
                        Some(intent.intent_id.clone()),
                        first_cycle
                            + chrono::Duration::seconds(
                                6 * 60 + 120 * i64::try_from(index).unwrap(),
                            ),
                        OperatorEventPayloadV3::SlotMissed {
                            reason: Some("operator ended after three cycles".into()),
                        },
                    )
                }),
        );
        bundle.events = actions
            .into_iter()
            .enumerate()
            .map(|(index, (slot_id, occurred_at, payload))| {
                let mutation_id = format!("skip-final-mutation-{index}");
                OperatorEventV3 {
                    meta: RecordMetaV3 {
                        schema_version: bundle.manifest.schema_version,
                        session_id: bundle.manifest.session_id.clone(),
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
                    event_id: format!("skip-final-event-{index}"),
                    occurred_at,
                    time_basis: EventTimeBasisV2::ObservedNow,
                    uncertainty_seconds: None,
                    slot_id,
                    payload,
                }
            })
            .collect();
        let snapshot = AcquisitionSnapshot::V3(bundle);

        assert_eq!(
            snapshot.final_completed_slot_id().as_deref(),
            Some(created.slot_ids[2].as_str())
        );
        assert_eq!(
            snapshot
                .projected_slots()
                .into_iter()
                .map(|slot| slot.slot_id)
                .collect::<Vec<_>>(),
            created.slot_ids[..3].to_vec()
        );
        let plans = authorized_plans(&snapshot).unwrap();
        assert_eq!(plans.last().unwrap().completed_slot_id, created.slot_ids[2]);
    }

    #[test]
    fn receive_only_transmit_only_and_single_antenna_snapshots_all_finalize() {
        let temp = TempDir::new().unwrap();
        let (_, path, _) = running_confirmed_session(temp.path(), true);
        let original = BundleStore::new(path).read_v3_checkpointed().unwrap();

        for (direction, single_antenna) in [
            (WsprCycleDirection::Receive, false),
            (WsprCycleDirection::Transmit, false),
            (WsprCycleDirection::Receive, true),
        ] {
            let mut bundle = original.clone();
            let first_antenna = bundle.schedule.wspr_cycle_intents[0].antenna_label.clone();
            bundle.schedule.wspr_cycle_intents.retain(|intent| {
                intent.direction == Some(direction)
                    && (!single_antenna || intent.antenna_label == first_antenna)
            });
            let snapshot = AcquisitionSnapshot::V3(bundle);
            assert!(!snapshot.projected_slots().is_empty());
            assert!(snapshot.final_completed_slot_id().is_some());
            let plans = authorized_plans(&snapshot).unwrap();
            assert_eq!(
                plans.last().map(|plan| plan.completed_slot_id.as_str()),
                snapshot.final_completed_slot_id().as_deref()
            );
        }
    }

    #[test]
    fn due_confirmations_fetch_once_and_atomically_commit_through_the_importer() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        let before = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        let transport = SequencedEmptyTransport {
            calls: Cell::new(0),
            first_received_at: now,
        };
        let runtime = WsprLiveAcquisitionState::default();

        let outcome = advance_with_transport(
            &active,
            &runtime,
            WsprLiveAcquisitionRequest::default(),
            now,
            &transport,
        )
        .unwrap();

        assert_eq!(transport.calls.get(), 2);
        let WsprLiveAcquisitionOutcome::AwaitingAcknowledgement {
            revision,
            captured_through,
            retry_available,
            successful_windows,
            returned,
            ..
        } = outcome
        else {
            panic!("all-zero final acquisition must await acknowledgement: {outcome:?}")
        };
        assert_eq!(captured_through, now - chrono::Duration::minutes(5));
        assert_eq!((successful_windows, returned), (1, 0));
        assert!(retry_available);
        assert_eq!(revision, before.session_state.revision + 2);
        let after = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(after.session_state.lifecycle, SessionLifecycleV2::Running);
        let summary = after
            .adapter_records
            .iter()
            .find(|record| record.record_type == "wspr_live_import_summary")
            .unwrap();
        assert_eq!(
            summary.meta.provenance.acquisition_channel.as_str(),
            "https-query"
        );
        assert_eq!(
            after.adapter_records.len(),
            before.adapter_records.len() + 4
        );

        let retried = advance_with_transport(
            &active,
            &runtime,
            WsprLiveAcquisitionRequest { retry: true },
            now + chrono::Duration::seconds(1),
            &transport,
        )
        .unwrap();
        let WsprLiveAcquisitionOutcome::AwaitingAcknowledgement {
            retry_available,
            successful_windows,
            returned,
            created,
            ..
        } = retried
        else {
            panic!("bounded retry must still await acknowledgement")
        };
        assert!(!retry_available);
        assert_eq!((successful_windows, returned, created), (2, 0, 0));
        assert_eq!(transport.calls.get(), 4);
        let after_retry = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(
            after_retry.session_state.lifecycle,
            SessionLifecycleV2::Running
        );

        let suppressed = advance_with_transport(
            &active,
            &runtime,
            WsprLiveAcquisitionRequest { retry: true },
            now + chrono::Duration::seconds(2),
            &transport,
        )
        .unwrap();
        assert!(matches!(
            suppressed,
            WsprLiveAcquisitionOutcome::AwaitingAcknowledgement {
                retry_available: false,
                ..
            }
        ));
        assert_eq!(transport.calls.get(), 4);
        assert_eq!(
            BundleStore::new(&path).read_v3_checkpointed().unwrap(),
            after_retry
        );
    }

    #[test]
    fn operator_end_commits_while_http_waits_and_stale_response_is_discarded() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        let before = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        let transport = MutatingTransport {
            calls: Cell::new(0),
            received_at: now,
            mutate: || {
                crate::open_session::with_waiting_foreground_operation(&active, || {
                    end_session_during_acquisition(&path, now);
                    crate::open_session::reload_active_session(&active, &path)?;
                    Ok(())
                })
                .unwrap();
            },
        };

        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now,
            &transport,
        )
        .unwrap();

        assert!(matches!(
            outcome,
            WsprLiveAcquisitionOutcome::Dormant { .. }
        ));
        assert_eq!(transport.calls.get(), 1);
        let after = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(after.session_state.lifecycle, SessionLifecycleV2::Ended);
        assert_eq!(after.adapter_records, before.adapter_records);
        assert_eq!(after.observations, before.observations);
        assert_eq!(
            after.session_state.revision,
            before.session_state.revision + 1
        );
    }

    #[test]
    fn census_query_failure_is_durable_without_failing_spot_capture() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        let before = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        let transport = CensusFailingTransport {
            calls: Cell::new(0),
            received_at: now,
        };

        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now,
            &transport,
        )
        .unwrap();

        assert!(matches!(
            outcome,
            WsprLiveAcquisitionOutcome::AwaitingAcknowledgement {
                returned: 0,
                retry_available: true,
                ..
            }
        ));
        assert_eq!(transport.calls.get(), 2);
        let after = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(after.observations, before.observations);
        let failure = after
            .adapter_records
            .iter()
            .find(|record| {
                record.record_type == "wspr_live_activity_census_summary"
                    && record.disposition == antennabench_core::v2::AdapterDisposition::Unsupported
            })
            .expect("typed census failure must be durable");
        assert_eq!(
            failure.reason.as_str(),
            "wspr-live.activity-census-query-failed"
        );
    }

    #[test]
    fn already_committed_all_zero_final_response_waits_without_fetching_or_duplicating_evidence() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        commit_final_response(&active, &path, now);
        let committed = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(
            committed.session_state.lifecycle,
            SessionLifecycleV2::Running
        );
        let transport = FakeTransport {
            calls: Cell::new(0),
            response: Err(WsprLiveAcquisitionError::Transport(
                "already committed evidence must not be fetched again".into(),
            )),
        };

        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now + chrono::Duration::seconds(1),
            &transport,
        )
        .unwrap();

        assert!(matches!(
            outcome,
            WsprLiveAcquisitionOutcome::AwaitingAcknowledgement {
                retry_available: true,
                ..
            }
        ));
        assert_eq!(transport.calls.get(), 0);
        let waiting = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(waiting.session_state.lifecycle, SessionLifecycleV2::Running);
        assert_eq!(waiting.adapter_records, committed.adapter_records);
        assert_eq!(waiting.observations, committed.observations);
    }

    #[test]
    fn finalization_failure_keeps_committed_evidence_and_exposes_the_backend_detail() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        let plan = commit_final_response(&active, &path, now);
        let committed = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        let store = BundleStore::new(&path);
        let mut writer = store.open_v3_writer().unwrap();
        let mutation_id = writer.allocate_id("mutation");
        let event = OperatorEventV3 {
            meta: RecordMetaV3 {
                schema_version: writer.snapshot().manifest.schema_version,
                session_id: writer.snapshot().manifest.session_id.clone(),
                recorded_at: now,
                provenance: Provenance::from_legacy(
                    RecordSource::Operator,
                    env!("CARGO_PKG_VERSION"),
                ),
                mutation: MutationMember {
                    mutation_id: mutation_id.clone(),
                    member_index: 0,
                    member_count: 1,
                },
                runtime_context_id: None,
            },
            event_id: writer.allocate_id("event"),
            occurred_at: now,
            time_basis: EventTimeBasisV2::ObservedNow,
            uncertainty_seconds: None,
            slot_id: None,
            payload: OperatorEventPayloadV3::SessionInterrupted {
                reason: Some("test concurrent lifecycle change".into()),
            },
        };
        writer
            .append_event(LiveEventMutationV3 {
                expected_revision: writer.checkpoint().revision,
                mutation_id,
                event,
            })
            .unwrap();
        drop(writer);

        let outcome = finish_final_capture(
            &active,
            &WsprLiveAcquisitionState::default(),
            &path,
            &plan,
            now + chrono::Duration::seconds(1),
            plan.query.window_end,
        )
        .unwrap();
        let WsprLiveAcquisitionOutcome::Failed {
            message, detail, ..
        } = outcome
        else {
            panic!("changed lifecycle must report a finalization failure")
        };
        assert_eq!(
            message,
            "Public spots were saved, but the session could not end automatically."
        );
        assert!(detail.contains("current lifecycle: Interrupted"));
        let after = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(after.adapter_records, committed.adapter_records);
        assert_eq!(after.observations, committed.observations);
        let diagnostic = after.diagnostics.last().unwrap();
        assert_eq!(
            diagnostic.operation,
            DiagnosticOperationV6::WsprLiveAcquisition
        );
        assert_eq!(diagnostic.phase, DiagnosticPhaseV6::Finalize);
        assert_eq!(diagnostic.outcome, DiagnosticOutcomeV6::Partial);
        assert_eq!(
            diagnostic.evidence_effect,
            EvidenceEffectV6::PrimaryEvidenceCommitted
        );
        assert_eq!(
            diagnostic.retry.disposition,
            RetryDispositionV6::RequiresStateChange
        );
        let reopened_state = ActiveSessionState::default();
        let reopened = crate::open_session::open_session_at_path(&reopened_state, path.clone())
            .expect("reopen partial finalization bundle");
        let crate::open_session::OpenSessionOutcome::Opened { session } = reopened else {
            panic!("reopen must select the bundle")
        };
        let presented = session.operational_history.diagnostics.last().unwrap();
        assert_eq!(presented.outcome, "partial");
        assert_eq!(presented.evidence_effect, "primary_evidence_committed");
        assert_eq!(presented.phase, "finalize");
    }

    #[test]
    fn rejected_wspr_live_batch_is_diagnosable_from_a_lossless_copy() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        let store = BundleStore::new(&path);
        let before = store.read_v3_checkpointed().unwrap();
        let snapshot = AcquisitionSnapshot::V3(before.clone());
        let plan = authorized_plans(&snapshot).unwrap().remove(0);
        let config = WsprLiveImportConfig {
            session_callsign: plan.query.session_callsign.clone(),
            window_start: plan.query.window_start,
            window_end: plan.query.window_end,
            selected_bands: snapshot
                .projected_slots()
                .into_iter()
                .map(|slot| slot.band)
                .collect(),
            captured_at: now,
            source_locator: Some("https://wspr.live".into()),
            confirmed_cycles: snapshot.confirmed_cycles(),
        };
        let mut writer = crate::build_context::open_v3_writer(&store).unwrap();
        let payload = SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.jsonl_line_bytes",
            "adapter_records",
            262_144,
            Some(301_337),
            "bytes",
        );
        let payload = persist_wspr_live_commit_failure(
            &mut writer,
            &config,
            "wspr-live-rejected-import",
            payload,
        );
        assert_eq!(
            payload
                .diagnostic_persistence
                .as_ref()
                .map(|status| status.status),
            Some("persisted")
        );
        drop(writer);

        let copy = temp.path().join("support-copy.session.antennabundle");
        store.copy_losslessly_to(&copy).unwrap();
        let copied = BundleStore::new(&copy).read_v3_checkpointed().unwrap();
        assert_eq!(copied.adapter_records, before.adapter_records);
        assert_eq!(copied.observations, before.observations);
        let diagnostic = copied.diagnostics.last().unwrap();
        assert_eq!(
            diagnostic.operation,
            DiagnosticOperationV6::WsprLiveAcquisition
        );
        assert_eq!(diagnostic.phase, DiagnosticPhaseV6::Preflight);
        assert_eq!(diagnostic.code, "resource.jsonl_line_bytes");
        assert_eq!(diagnostic.revision_before, diagnostic.revision_after);
        assert_eq!(diagnostic.evidence_effect, EvidenceEffectV6::NoneCommitted);
        assert_eq!(
            diagnostic.retry.disposition,
            RetryDispositionV6::RequiresInputChange
        );
        assert!(diagnostic.targets.iter().any(|target| matches!(
            target,
            DiagnosticTargetV6::AcquisitionWindow { start, end }
                if *start == config.window_start && *end == config.window_end
        )));
        assert!(diagnostic.causes[0].facts.iter().any(|fact| {
            fact.name == "observed_bytes" && fact.value == DiagnosticFactValueV6::U64(301_337)
        }));
        assert!(diagnostic.causes[0].facts.iter().any(|fact| {
            fact.name == "limit_bytes" && fact.value == DiagnosticFactValueV6::U64(262_144)
        }));
        assert!(copied
            .runtime_contexts
            .iter()
            .any(|context| { context.context_id == diagnostic.runtime_context_id }));
        let reopened_state = ActiveSessionState::default();
        let reopened = crate::open_session::open_session_at_path(&reopened_state, copy.clone())
            .expect("reopen copied regression bundle");
        let crate::open_session::OpenSessionOutcome::Opened { session } = reopened else {
            panic!("reopen must select the copied bundle")
        };
        let history = &session.operational_history;
        let presented = history.diagnostics.last().unwrap();
        assert_eq!(presented.code, "resource.jsonl_line_bytes");
        assert_eq!(presented.operation, "wspr_live_acquisition");
        assert_eq!(presented.phase, "preflight");
        assert_eq!(presented.evidence_effect, "none_committed");
        assert!(presented
            .causes
            .iter()
            .any(|cause| cause.contains("observed_bytes=301337")
                && cause.contains("limit_bytes=262144")));
        assert!(history.contexts.iter().any(|context| {
            context.context_id == presented.runtime_context_id
                && context.app_version.is_some()
                && context.os_family.is_some()
        }));
        assert!(history
            .support_summary
            .contains("resource.jsonl_line_bytes"));
        assert!(history.support_summary.contains("observed_bytes=301337"));
        assert!(history.support_summary.contains("limit_bytes=262144"));
        assert!(!history.support_summary.contains(&config.session_callsign));
        assert!(!history.support_summary.contains("FN42"));
        assert!(history.support_summary.len() <= history.support_summary_max_bytes);
        let second_state = ActiveSessionState::default();
        let second = crate::open_session::open_session_at_path(&second_state, copy)
            .expect("reopen copied regression bundle a second time");
        let crate::open_session::OpenSessionOutcome::Opened { session: second } = second else {
            panic!("second reopen must select the copied bundle")
        };
        assert_eq!(
            history.support_summary,
            second.operational_history.support_summary
        );
        drop(active);
    }

    #[test]
    fn non_empty_automatic_capture_aligns_the_provider_slot_to_its_confirmed_cycle() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        let before = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(
            before.schedule.wspr_cycle_intents[0].direction,
            Some(WsprCycleDirection::Receive)
        );
        let transport = FakeTransport {
            calls: Cell::new(0),
            response: Ok(confirmed_receive_response(now)),
        };

        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now,
            &transport,
        )
        .unwrap();

        assert_eq!(transport.calls.get(), 2);
        let WsprLiveAcquisitionOutcome::Captured {
            total,
            accepted,
            observations_created,
            checked_at,
            ..
        } = outcome
        else {
            panic!("non-empty acquisition must capture: {outcome:?}")
        };
        assert_eq!((total, accepted, observations_created), (1, 1, 1));
        assert_eq!(checked_at, now);

        let after = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        let observation = after.observations.last().unwrap();
        assert_eq!(observation.raw["direction"], "receive");
        assert_eq!(observation.reporter_call.as_deref(), Some("N1RWJ"));
        assert_eq!(observation.heard_call.as_deref(), Some("K1ABC"));
        let adapter = after
            .adapter_records
            .iter()
            .find(|record| record.record_type == "wspr_live_spot")
            .unwrap();
        assert_eq!(
            adapter.source_time,
            Some("2026-07-15T20:00:00Z".parse().unwrap())
        );
    }

    #[test]
    fn multi_record_capture_may_exceed_one_jsonl_line_in_aggregate() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        let before = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        let transport = FakeTransport {
            calls: Cell::new(0),
            response: Ok(large_confirmed_receive_response(now)),
        };

        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now,
            &transport,
        )
        .unwrap();

        let WsprLiveAcquisitionOutcome::Captured {
            accepted,
            observations_created,
            ..
        } = outcome
        else {
            panic!("large multi-record acquisition must capture: {outcome:?}")
        };
        assert_eq!((accepted, observations_created), (400, 400));
        let after = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        assert_eq!(after.session_state.lifecycle, SessionLifecycleV2::Ended);
        assert!(
            after.session_state.streams["adapter_records"].committed_bytes
                - before.session_state.streams["adapter_records"].committed_bytes
                > 256 * 1024
        );
    }

    #[test]
    fn explicit_opt_out_never_contacts_the_transport() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), false);
        let before = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        let transport = FakeTransport {
            calls: Cell::new(0),
            response: Err(WsprLiveAcquisitionError::Transport(
                "transport must remain unused".into(),
            )),
        };

        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now,
            &transport,
        )
        .unwrap();

        assert_eq!(outcome, WsprLiveAcquisitionOutcome::Disabled);
        assert_eq!(transport.calls.get(), 0);
        assert_eq!(
            BundleStore::new(&path).read_v3_checkpointed().unwrap(),
            before
        );
    }

    #[test]
    fn failures_do_not_mutate_or_automatically_retry_but_restart_may_resume() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        let before = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        let runtime = WsprLiveAcquisitionState::default();
        let offline = FakeTransport {
            calls: Cell::new(0),
            response: Err(WsprLiveAcquisitionError::Transport("offline".into())),
        };

        let first = advance_with_transport(
            &active,
            &runtime,
            WsprLiveAcquisitionRequest::default(),
            now,
            &offline,
        )
        .unwrap();
        assert!(matches!(first, WsprLiveAcquisitionOutcome::Failed { .. }));
        assert_eq!(offline.calls.get(), 1);
        let suppressed = advance_with_transport(
            &active,
            &runtime,
            WsprLiveAcquisitionRequest::default(),
            now + chrono::Duration::seconds(10),
            &offline,
        )
        .unwrap();
        assert!(matches!(
            suppressed,
            WsprLiveAcquisitionOutcome::Failed { .. }
        ));
        assert_eq!(offline.calls.get(), 1);
        assert_eq!(
            BundleStore::new(&path).read_v3_checkpointed().unwrap(),
            before
        );

        let resumed = FakeTransport {
            calls: Cell::new(0),
            response: Ok(empty_response(now + chrono::Duration::seconds(20))),
        };
        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now + chrono::Duration::seconds(20),
            &resumed,
        )
        .unwrap();
        assert!(matches!(
            outcome,
            WsprLiveAcquisitionOutcome::AwaitingAcknowledgement {
                retry_available: true,
                ..
            }
        ));
        assert_eq!(resumed.calls.get(), 2);
    }

    #[test]
    fn invalid_final_response_uses_wspr_live_copy_and_retains_backend_detail() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path(), true);
        let before = BundleStore::new(&path).read_v3_checkpointed().unwrap();
        let transport = FakeTransport {
            calls: Cell::new(0),
            response: Ok(WsprLiveHttpResponse {
                received_at: now,
                status: 200,
                body: b"{".to_vec(),
            }),
        };

        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now,
            &transport,
        )
        .unwrap();
        let WsprLiveAcquisitionOutcome::Failed {
            message, detail, ..
        } = outcome
        else {
            panic!("invalid provider data must report a capture failure")
        };
        assert_eq!(message, "The WSPR.live response could not be saved.");
        assert!(!detail.is_empty());
        assert_eq!(
            BundleStore::new(&path).read_v3_checkpointed().unwrap(),
            before
        );
    }
}
