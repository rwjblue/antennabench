export function invokeOpenSessionFromAnotherLocation(invoke) {
  return invoke("open_session_bundle");
}

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

export function invokeRefreshActiveSessionReport(invoke) {
  return invoke("refresh_active_session_report");
}

export function invokeExportActiveSessionReport(
  invoke,
  format,
  controllerEvidence = "complete",
  operationalHistory = "omitted",
) {
  return invoke("export_active_session_report", { format, controllerEvidence, operationalHistory });
}

export function invokeExportSession(invoke) {
  return invoke("export_active_session");
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

export function invokeDeleteAntennaControllerProfile(invoke, profileId) {
  return invoke("delete_antenna_controller_profile", { profileId });
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
