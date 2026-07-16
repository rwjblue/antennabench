export function invokeOpenSession(invoke) {
  return invoke("open_session_bundle");
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

export function invokeExportActiveSessionReport(invoke) {
  return invoke("export_active_session_report");
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


