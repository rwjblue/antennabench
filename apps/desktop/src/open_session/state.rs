use super::*;

// Centralize active-session mutation and the single foreground-operation exclusion boundary.
#[derive(Default)]
pub(crate) struct ActiveSessionState(pub(super) Mutex<DesktopState>, Condvar);

#[derive(Default)]
pub(super) struct DesktopState {
    pub(super) active: Option<ActiveSession>,
    pub(super) export_source: Option<PathBuf>,
    pub(super) pending_report_export: Option<PendingReportExport>,
    pub(super) foreground_busy: bool,
    pub(super) next_presentation_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReportDestinationIdentity {
    pub(super) length: u64,
    pub(super) modified_nanos: Option<u128>,
    #[cfg(unix)]
    pub(super) device: u64,
    #[cfg(unix)]
    pub(super) inode: u64,
    #[cfg(unix)]
    pub(super) changed_seconds: i64,
    #[cfg(unix)]
    pub(super) changed_nanos: i64,
    pub(super) content_digest: [u8; 32],
}

#[derive(Debug)]
pub(super) struct PendingReportExport {
    pub(super) pending_export_id: String,
    pub(super) destination: PathBuf,
    pub(super) destination_identity: ReportDestinationIdentity,
    pub(super) file_name: String,
    pub(super) presentation_id: u64,
    pub(super) session_id: String,
    pub(super) revision: Option<u64>,
    pub(super) format: ReportExportFormat,
    pub(super) html: String,
}

pub(super) struct ForegroundGuard<'a>(&'a ActiveSessionState);

impl ActiveSessionState {
    pub(super) fn begin_foreground(&self) -> Result<ForegroundGuard<'_>, SessionErrorPayload> {
        let mut state = self.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        if state.foreground_busy {
            return Err(SessionErrorPayload::resource(
                SessionErrorKind::Busy,
                "resource.operation.busy",
                "foreground",
                1,
                Some(2),
                "operations",
            ));
        }
        state.foreground_busy = true;
        Ok(ForegroundGuard(self))
    }

    fn wait_for_foreground(&self) -> Result<ForegroundGuard<'_>, SessionErrorPayload> {
        let mut state = self.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        while state.foreground_busy {
            state = self.1.wait(state).map_err(|_| {
                SessionErrorPayload::report_pipeline("active session state is unavailable")
            })?;
        }
        state.foreground_busy = true;
        Ok(ForegroundGuard(self))
    }
}

pub(crate) fn with_foreground_operation<T>(
    state: &ActiveSessionState,
    operation: impl FnOnce() -> Result<T, SessionErrorPayload>,
) -> Result<T, SessionErrorPayload> {
    let _foreground = state.begin_foreground()?;
    operation()
}

pub(crate) fn with_waiting_foreground_operation<T>(
    state: &ActiveSessionState,
    operation: impl FnOnce() -> Result<T, SessionErrorPayload>,
) -> Result<T, SessionErrorPayload> {
    let _foreground = state.wait_for_foreground()?;
    operation()
}

pub(crate) fn with_suspended_foreground_operation<T>(
    state: &ActiveSessionState,
    operation: impl FnOnce() -> T,
) -> Result<T, SessionErrorPayload> {
    {
        let mut desktop = state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        if !desktop.foreground_busy {
            return Err(SessionErrorPayload::report_pipeline(
                "cannot suspend an inactive foreground operation",
            ));
        }
        desktop.foreground_busy = false;
        state.1.notify_one();
    }
    let result = operation();
    let mut desktop = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    while desktop.foreground_busy {
        desktop = state.1.wait(desktop).map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
    }
    desktop.foreground_busy = true;
    Ok(result)
}

pub(crate) fn reload_active_session(
    state: &ActiveSessionState,
    source: &Path,
) -> Result<OpenedSession, SessionErrorPayload> {
    let refreshed = open_bundle(source).map_err(SessionErrorPayload::from)?;
    let summary = refreshed.summary.clone();
    let mut desktop = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    if desktop
        .active
        .as_ref()
        .is_some_and(|session| session.source != source)
    {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The active session changed while WSPR.live evidence was importing.",
            "the import remains committed to the originally selected session",
        ));
    }
    desktop.active = Some(refreshed);
    desktop.pending_report_export = None;
    Ok(summary)
}

impl Drop for ForegroundGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0 .0.lock() {
            state.foreground_busy = false;
            self.0 .1.notify_one();
        }
    }
}

#[derive(Debug)]
pub(super) struct ActiveSession {
    pub(super) source: PathBuf,
    pub(super) live_projection: Option<BundleV3Contents>,
    pub(super) summary: OpenedSession,
    pub(super) presentation: Option<ReportPresentation>,
}

pub(crate) fn active_session_live_projection(
    state: &ActiveSessionState,
) -> Result<Option<(PathBuf, String, BundleV3Contents)>, SessionErrorPayload> {
    let active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    let session = active.active.as_ref().ok_or_else(|| {
        SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "Create or open a session before using this Active Run control.",
            "no active session is available",
        )
    })?;
    let projection = session.live_projection.clone().map(|bundle| {
        (
            session.source.clone(),
            session.summary.bundle_name.clone(),
            bundle,
        )
    });
    drop(active);
    if let Some((source, _, bundle)) = &projection {
        let checkpoint = BundleStore::new(source)
            .read_v3_checkpoint_state()
            .map_err(|error| {
                SessionErrorPayload::new(
                    SessionErrorKind::Resource,
                    "The Active Run checkpoint is temporarily unavailable.",
                    error.to_string(),
                )
            })?;
        if checkpoint != bundle.session_state {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The Active Run changed outside the current in-memory projection. Reopen it before continuing.",
                "session-state.json no longer matches the projected durable checkpoint",
            ));
        }
    }
    Ok(projection)
}

pub(crate) fn update_active_session_live_projection(
    state: &ActiveSessionState,
    source: &Path,
    bundle: &BundleV3Contents,
) -> Result<OpenedSession, SessionErrorPayload> {
    let mut desktop = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    let session = desktop.active.as_mut().ok_or_else(|| {
        SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "Create or open a session before using this Active Run control.",
            "no active session is available",
        )
    })?;
    if session.source != source || session.summary.session_id != bundle.manifest.session_id {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The active session changed while live evidence was being processed.",
            "the projection update belongs to a different active session",
        ));
    }
    let unchanged_compact_projection = session.live_projection.as_ref() == Some(bundle);
    session.summary.revision = Some(bundle.session_state.revision);
    session.summary.lifecycle = Some(bundle.session_state.lifecycle);
    if !unchanged_compact_projection {
        session.summary.observation_count = bundle.observations.len();
    }
    session.live_projection = Some(super::projection::compact_live_projection(bundle.clone()));
    Ok(session.summary.clone())
}

pub(crate) fn activate_created_bundle(
    state: &ActiveSessionState,
    path: PathBuf,
) -> Result<OpenedSession, SessionErrorPayload> {
    let mut session = open_bundle(&path).map_err(SessionErrorPayload::from)?;
    let summary = session.summary.clone();
    check_ipc_payload(
        &OpenSessionOutcome::Opened {
            session: Box::new(summary.clone()),
        },
        SESSION_SUMMARY_IPC_BYTES,
        "session_summary",
    )?;
    let mut active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    assign_presentation_id(&mut active, &mut session);
    active.active = Some(session);
    active.export_source = Some(path);
    Ok(summary)
}

pub(crate) fn active_session_source(
    state: &ActiveSessionState,
) -> Result<(PathBuf, String), SessionErrorPayload> {
    let active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    let session = active.active.as_ref().ok_or_else(|| {
        SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "Create or open a schema-v2 session before using the conductor.",
            "no active session is available",
        )
    })?;
    Ok((session.source.clone(), session.summary.bundle_name.clone()))
}

pub(super) fn assign_presentation_id(state: &mut DesktopState, session: &mut ActiveSession) {
    state.pending_report_export = None;
    if let Some(presentation) = &mut session.presentation {
        state.next_presentation_id = state.next_presentation_id.saturating_add(1);
        presentation.presentation_id = state.next_presentation_id;
    }
}
