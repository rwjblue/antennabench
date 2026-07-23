export function invokeOpenManagedSession(invoke, locatorId) {
  return invoke("open_managed_session", { locatorId });
}

export function invokeListManagedSessions(invoke) {
  return invoke("list_managed_sessions");
}

export function invokeRevealManagedSessionsDirectory(invoke) {
  return invoke("reveal_managed_sessions_directory");
}

export function invokeRevealManagedSession(invoke, locatorId) {
  return invoke("reveal_managed_session", { locatorId });
}

export function invokeDeleteManagedSession(invoke, locatorId) {
  return invoke("delete_managed_session", { locatorId });
}

export function invokeImportManagedSession(invoke) {
  return invoke("import_managed_session");
}

export function invokeExportManagedSession(invoke, locatorId) {
  return invoke("export_managed_session", { locatorId });
}

export function invokeReviewSessionSetup(invoke, draft) {
  return invoke("review_session_setup", { draft });
}

export function invokeStationLocation(invoke) {
  return invoke("request_station_location");
}

export function invokeLoadStationPreferences(invoke) {
  return invoke("load_station_preferences");
}

export function invokeCreateSessionFromReview(invoke, reviewId) {
  return invoke("create_session_from_review", { reviewId });
}

export function invokeActiveSessionReport(invoke) {
  return invoke("active_session_report");
}

export function invokeRefreshActiveSessionReport(invoke, displayedPresentationId) {
  if (displayedPresentationId === undefined || displayedPresentationId === null) {
    return invoke("refresh_active_session_report");
  }
  return invoke("refresh_active_session_report", { displayedPresentationId });
}

export function invokeOpenReportWindow(invoke, displayedPresentationId, documentKind) {
  return invoke("open_report_window", { displayedPresentationId, documentKind });
}

export function invokeReportWindowDocument(invoke) {
  return invoke("report_window_document");
}

export function invokeExportActiveSessionReport(
  invoke,
  format,
  controllerEvidence = "complete",
  operationalHistory = "omitted",
  displayedPresentationId,
) {
  const payload = {
    format,
    controllerEvidence,
    operationalHistory,
  };
  if (displayedPresentationId !== undefined && displayedPresentationId !== null) {
    payload.displayedPresentationId = displayedPresentationId;
  }
  return invoke("export_active_session_report", payload);
}

export function invokeConfirmReportExport(invoke, pendingExportId) {
  return invoke("confirm_report_export", { pendingExportId });
}

export function invokeCancelReportExport(invoke, pendingExportId) {
  return invoke("cancel_report_export", { pendingExportId });
}

export function invokeImportActiveSessionWsprLive(invoke) {
  return invoke("import_active_session_wspr_live");
}

export function invokeImportActiveSessionRbn(invoke) {
  return invoke("import_active_session_rbn");
}

export function invokeActiveSessionConductor(invoke) {
  return invoke("active_session_conductor");
}

export function invokeMutateSessionConductor(invoke, request) {
  return invoke("mutate_active_session_conductor", { request });
}

export function invokeActiveSessionWsjtxStatus(invoke) {
  return invoke("active_session_wsjtx_status");
}

export function invokeStartSessionWsjtx(invoke, request) {
  return invoke("start_active_session_wsjtx", { request });
}

export function invokeStopSessionWsjtx(invoke) {
  return invoke("stop_active_session_wsjtx");
}

export function invokeAdvanceSessionWsprLive(invoke, retry = false) {
  return invoke("advance_active_session_wspr_live", { request: { retry } });
}

export function invokeAntennaControllerProfiles(invoke) {
  return invoke("antenna_controller_profiles");
}

export function invokeSaveAntennaControllerProfile(invoke, draft) {
  return invoke("save_antenna_controller_profile", { draft });
}

export function invokeDeleteAntennaControllerProfile(invoke, profileId, profileRevision) {
  return invoke("delete_antenna_controller_profile", { profileId, profileRevision });
}

export function invokeActiveSessionAntennaController(invoke) {
  return invoke("active_session_antenna_controller");
}

export function invokeAttachSessionAntennaController(invoke, request) {
  return invoke("attach_active_session_antenna_controller", { request });
}

export function invokeRunSessionAntennaController(invoke, request) {
  return invoke("run_active_session_antenna_controller", { request });
}
