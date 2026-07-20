//! Live-session checkpoint read/mutation orchestration and typed error preservation.

use std::sync::Arc;

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_core::v2::V2_BUNDLE_SUFFIX;
use antennabench_storage::LivePersistenceError;
use chrono::{DateTime, Utc};

#[derive(Default)]
pub(crate) struct ConductorSessionState(Mutex<ConductorRuntime>);

#[derive(Default)]
struct ConductorRuntime {
    initialized_source: Option<PathBuf>,
    pending_actions: VecDeque<PendingAction>,
}

#[derive(Debug, Clone)]
pub(super) struct PendingAction {
    pub(super) token: String,
    pub(super) session_id: String,
    pub(super) expected_revision: u64,
    pub(super) occurred_at: Option<DateTime<Utc>>,
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

impl ControllerActionPort for ConductorSessionState {
    fn authorize_controller_action(
        &self,
        token: &str,
        session_id: &str,
        expected_revision: u64,
        now: DateTime<Utc>,
    ) -> Result<(), SessionErrorPayload> {
        let pending = self
            .0
            .lock()
            .map_err(|_| SessionErrorPayload::report_pipeline("conductor state is unavailable"))?
            .resolve_action(token, now)
            .ok_or_else(|| {
                SessionErrorPayload::new(
                    SessionErrorKind::StaleRevision,
                    "Refresh Active Run before submitting this controller action.",
                    "the Rust-issued action token is missing or expired",
                )
            })?;
        if pending.session_id != session_id || pending.expected_revision != expected_revision {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::StaleRevision,
                "Refresh Active Run before submitting this controller action.",
                "the action token does not match this session revision",
            ));
        }
        Ok(())
    }
}

fn ensure_live_source(source: &Path) -> Result<(), SessionErrorPayload> {
    let valid = source
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(V2_BUNDLE_SUFFIX));
    if valid {
        Ok(())
    } else {
        Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "The live conductor requires a checkpointed session bundle.",
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
            "session writer lock is busy",
        )
        .with_operation("session.writer_busy", "admission"),
        LivePersistenceError::StaleRevision { expected, actual } => SessionErrorPayload::new(
            SessionErrorKind::StaleRevision,
            "The session changed. Refresh the conductor before retrying.",
            format!("expected checkpoint revision {expected}, actual revision {actual}"),
        )
        .with_operation("session.stale_revision", "admission"),
        LivePersistenceError::MutationConflict { mutation_id } => SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "That action token was already used for different evidence.",
            format!("conflicting mutation ID {mutation_id}"),
        )
        .with_operation("session.mutation_conflict", "admission"),
        LivePersistenceError::Capability { message } => SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "This filesystem cannot safely conduct a live session.",
            message,
        )
        .with_operation("session.capability_unavailable", "admission"),
        error @ LivePersistenceError::RecoveryRequired { .. } => SessionErrorPayload::new(
            SessionErrorKind::Verification,
            "The session must be recovered before it can be changed.",
            error.to_string(),
        )
        .with_operation("session.recovery_required", "recover"),
        error @ (LivePersistenceError::InvalidMutation { .. }
        | LivePersistenceError::PlanFrozen { .. }) => SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The requested conductor action is not valid for this session.",
            error.to_string(),
        )
        .with_operation("session.invalid_mutation", "preflight"),
        LivePersistenceError::ResourceLimit {
            code,
            stream,
            observed,
            limit,
        } => SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            code,
            stream,
            limit,
            Some(observed),
            "bounded_units",
        ),
        error @ LivePersistenceError::ExternalModification { .. } => SessionErrorPayload::new(
            SessionErrorKind::Verification,
            "The session changed outside AntennaBench and live mutation was stopped.",
            error.to_string(),
        )
        .with_operation("session.external_modification", "checkpoint"),
        error @ LivePersistenceError::Io { .. } => SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The conductor could not durably update the session.",
            error.to_string(),
        )
        .with_operation("session.persistence_io", "checkpoint"),
        error @ LivePersistenceError::CheckpointVerification { .. } => SessionErrorPayload::new(
            SessionErrorKind::Verification,
            "The conductor could not verify a coherent checkpoint.",
            error.to_string(),
        )
        .with_operation("session.checkpoint_verification", "checkpoint"),
    }
}

pub(crate) trait ControllerActionPort {
    /// Admits only an already-issued conductor token for controller assistance.
    /// The port deliberately grants neither process authority nor persistence access.
    fn authorize_controller_action(
        &self,
        token: &str,
        session_id: &str,
        expected_revision: u64,
        now: DateTime<Utc>,
    ) -> Result<(), SessionErrorPayload>;
}

use antennabench_core::{
    SCHEMA_VERSION_V2, SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5, SCHEMA_VERSION_V6,
};
use antennabench_storage::{
    BundleStore, LiveEventMutationV3, LiveMutationMemberV2, LiveMutationV2, LivePersistenceHooks,
};

use crate::open_session::{
    active_session_source, check_ipc_payload, storage_error_payload, with_foreground_operation,
    with_waiting_foreground_operation, ActiveSessionState, SessionErrorKind, SessionErrorPayload,
};

use super::view::{build_view, build_view_v3, recovery_view};
use super::MAX_PENDING_ACTION_TOKENS;
use super::{
    actions::{event_for_action, event_for_action_v3, ConductorMutationRequest},
    ConductorView, CONDUCTOR_VIEW_IPC_BYTES,
};

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

pub(super) fn read_conductor_with_hooks(
    active_state: &ActiveSessionState,
    conductor_state: &ConductorSessionState,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<ConductorView, SessionErrorPayload> {
    with_foreground_operation(active_state, || {
        let (source, bundle_name) = active_session_source(active_state)?;
        ensure_live_source(&source)?;
        let store = BundleStore::new(&source);
        let schema_version = store.schema_version().map_err(storage_error_payload)?;
        let initialized = conductor_state
            .0
            .lock()
            .map_err(|_| SessionErrorPayload::report_pipeline("conductor state is unavailable"))?
            .is_initialized(&source);
        let recovery = if !initialized {
            let report = match schema_version {
                SCHEMA_VERSION_V2 => store.recover_v2_with_hooks(hooks.clone()),
                SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6 => {
                    crate::build_context::recover_v3_with_hooks(&store, hooks.clone())
                }
                actual => {
                    return Err(SessionErrorPayload::new(
                        SessionErrorKind::Unsupported,
                        "This session schema cannot be conducted by this AntennaBench version.",
                        format!("unsupported schema version {actual}"),
                    ));
                }
            }
            .map_err(live_error_payload)?;
            conductor_state
                .0
                .lock()
                .map_err(|_| {
                    SessionErrorPayload::report_pipeline("conductor state is unavailable")
                })?
                .mark_initialized(source.clone());
            Some(recovery_view(&report))
        } else {
            None
        };
        let now = hooks.now();
        let action_token = hooks.new_id("mutation");
        let view = match schema_version {
            SCHEMA_VERSION_V2 => {
                let bundle = store.read_v2_checkpointed().map_err(live_error_payload)?;
                register_view_action(
                    conductor_state,
                    &bundle.manifest.session_id,
                    bundle.session_state.revision,
                    action_token.clone(),
                )?;
                build_view(bundle_name, &bundle, now, action_token, recovery)
            }
            SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6 => {
                let bundle = store.read_v3_checkpointed().map_err(live_error_payload)?;
                register_view_action(
                    conductor_state,
                    &bundle.manifest.session_id,
                    bundle.session_state.revision,
                    action_token.clone(),
                )?;
                build_view_v3(bundle_name, &bundle, now, action_token, recovery)
            }
            actual => {
                return Err(SessionErrorPayload::new(
                    SessionErrorKind::Unsupported,
                    "This session schema cannot be conducted by this AntennaBench version.",
                    format!("unsupported schema version {actual}"),
                ));
            }
        };
        check_ipc_payload(&view, CONDUCTOR_VIEW_IPC_BYTES, "conductor_view")?;
        Ok(view)
    })
}

pub(super) fn mutate_conductor_with_hooks(
    active_state: &ActiveSessionState,
    conductor_state: &ConductorSessionState,
    request: ConductorMutationRequest,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<ConductorView, SessionErrorPayload> {
    with_waiting_foreground_operation(active_state, || {
        let (source, bundle_name) = active_session_source(active_state)?;
        ensure_live_source(&source)?;
        let store = BundleStore::new(&source);
        let schema_version = store.schema_version().map_err(storage_error_payload)?;
        match schema_version {
            SCHEMA_VERSION_V2 => {
                mutate_conductor_v2(&store, bundle_name, conductor_state, request, hooks)
            }
            SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6 => {
                mutate_conductor_v3(&store, bundle_name, conductor_state, request, hooks)
            }
            actual => Err(SessionErrorPayload::new(
                SessionErrorKind::Unsupported,
                "This session schema cannot be conducted by this AntennaBench version.",
                format!("unsupported schema version {actual}"),
            )),
        }
    })
}

fn mutate_conductor_v2(
    store: &BundleStore,
    bundle_name: String,
    conductor_state: &ConductorSessionState,
    request: ConductorMutationRequest,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<ConductorView, SessionErrorPayload> {
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
}

fn mutate_conductor_v3(
    store: &BundleStore,
    bundle_name: String,
    conductor_state: &ConductorSessionState,
    request: ConductorMutationRequest,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<ConductorView, SessionErrorPayload> {
    let snapshot = store.read_v3_checkpointed().map_err(live_error_payload)?;
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
    let event = event_for_action_v3(
        &snapshot.manifest.session_id,
        snapshot.manifest.schema_version,
        &pending,
        request.action,
    )?;
    let mutation = LiveEventMutationV3 {
        expected_revision: request.expected_revision,
        mutation_id: pending.token.clone(),
        event,
    };
    let append_result = {
        let mut writer = crate::build_context::open_v3_writer_with_hooks(store, hooks.clone())
            .map_err(live_error_payload)?;
        writer.append_event(mutation)
    };
    if let Err(error) = append_result {
        let committed = store.read_v3_checkpointed().ok().is_some_and(|bundle| {
            bundle.session_state.last_committed_mutation_id.as_deref()
                == Some(pending.token.as_str())
        });
        if !committed {
            return Err(live_error_payload(error));
        }
    }
    let bundle = store.read_v3_checkpointed().map_err(live_error_payload)?;
    let now = hooks.now();
    let action_token = hooks.new_id("mutation");
    register_view_action(
        conductor_state,
        &bundle.manifest.session_id,
        bundle.session_state.revision,
        action_token.clone(),
    )?;
    let view = build_view_v3(bundle_name, &bundle, now, action_token, None);
    check_ipc_payload(&view, CONDUCTOR_VIEW_IPC_BYTES, "conductor_view")?;
    Ok(view)
}
