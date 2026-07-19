use super::*;

// Centralize active-session mutation and the single foreground-operation exclusion boundary.
#[derive(Default)]
pub(crate) struct ActiveSessionState(pub(super) Mutex<DesktopState>);

#[derive(Default)]
pub(super) struct DesktopState {
    pub(super) active: Option<ActiveSession>,
    pub(super) export_source: Option<PathBuf>,
    pub(super) foreground_busy: bool,
    pub(super) next_presentation_id: u64,
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
}

pub(crate) fn with_foreground_operation<T>(
    state: &ActiveSessionState,
    operation: impl FnOnce() -> Result<T, SessionErrorPayload>,
) -> Result<T, SessionErrorPayload> {
    let _foreground = state.begin_foreground()?;
    operation()
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
    Ok(summary)
}

impl Drop for ForegroundGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0 .0.lock() {
            state.foreground_busy = false;
        }
    }
}

#[derive(Debug)]
pub(super) struct ActiveSession {
    pub(super) source: PathBuf,
    pub(super) summary: OpenedSession,
    pub(super) presentation: Option<ReportPresentation>,
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
    if let Some(presentation) = &mut session.presentation {
        state.next_presentation_id = state.next_presentation_id.saturating_add(1);
        presentation.presentation_id = state.next_presentation_id;
    }
}
