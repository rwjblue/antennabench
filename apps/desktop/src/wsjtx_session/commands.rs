use super::*;

#[tauri::command]
pub(crate) fn active_session_wsjtx_status(
    active_state: State<'_, ActiveSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
) -> Result<WsjtxReceiverStatus, SessionErrorPayload> {
    let (source, _) = active_session_source(active_state.inner())?;
    let now = Utc::now();
    let mut status = wsjtx_state.status_for_source(&source, now);
    if let Ok(snapshot) = read_wsjtx_snapshot(&BundleStore::new(source)) {
        project_setup_warnings(&snapshot, &mut status, now);
    }
    check_status_ipc(&status)?;
    Ok(status)
}

#[tauri::command]
pub(crate) fn start_active_session_wsjtx(
    request: StartWsjtxRequest,
    active_state: State<'_, ActiveSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
) -> Result<WsjtxReceiverStatus, SessionErrorPayload> {
    with_foreground_operation(active_state.inner(), || {
        let (source, _) = active_session_source(active_state.inner())?;
        let status = start_receiver(
            wsjtx_state.inner(),
            source.clone(),
            request,
            Arc::new(SystemLivePersistenceHooks),
        )
        .map_err(|payload| {
            crate::operation_diagnostics::persist_wsjtx_start_failure(&source, payload)
        })?;
        check_status_ipc(&status)?;
        Ok(status)
    })
}

#[tauri::command]
pub(crate) fn stop_active_session_wsjtx(
    active_state: State<'_, ActiveSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
) -> Result<WsjtxReceiverStatus, SessionErrorPayload> {
    let (source, _) = active_session_source(active_state.inner())?;
    wsjtx_state.stop_for_source(&source, "The operator stopped WSJT-X reception.");
    let status = wsjtx_state.status_for_source(&source, Utc::now());
    check_status_ipc(&status)?;
    Ok(status)
}
