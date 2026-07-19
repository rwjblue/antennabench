import { WORKFLOWS } from "./models.mjs";

export function initialState(workflow = "setup") {
  return selectWorkflow(
    {
      activeWorkflow: "setup",
      openStatus: "idle",
      session: null,
      reportPresentationId: 0,
      reportStatus: "idle",
      reportError: null,
      reportExportStatus: "idle",
      reportExportError: null,
      reportExportNotice: null,
      error: null,
      notice: null,
      exportStatus: "idle",
      exportError: null,
      exportNotice: null,
      exportedBundleName: null,
      importStatus: "idle",
      importKind: null,
      importError: null,
      importNotice: null,
      setupStatus: "editing",
      setupReview: null,
      setupError: null,
      setupNotice: null,
      conductorStatus: "idle",
      conductor: null,
      conductorError: null,
      wsjtxStatus: "idle",
      wsjtx: null,
      wsjtxError: null,
      wsprLiveAcquisitionStatus: "idle",
      wsprLiveAcquisition: null,
      wsprLiveAcquisitionError: null,
      antennaControllerStatus: "idle",
      antennaControllerCatalog: null,
      antennaController: null,
      antennaControllerError: null,
      antennaControllerOutcome: null,
      antennaControllerProfileNotice: null,
      antennaControllerProfileError: null,
    },
    workflow,
  );
}

export function selectWorkflow(state, workflow) {
  if (!WORKFLOWS.includes(workflow)) {
    throw new RangeError(`Unknown desktop workflow: ${workflow}`);
  }

  if (state.activeWorkflow === workflow) {
    return state;
  }

  return { ...state, activeWorkflow: workflow };
}

export function beginOpenSession(state) {
  return { ...state, openStatus: "loading", error: null, notice: null };
}

export function editSessionSetup(state) {
  if (
    state.setupStatus === "editing"
    && state.setupReview === null
    && state.antennaControllerProfileError === null
  ) return state;
  return {
    ...state,
    setupStatus: "editing",
    setupReview: null,
    setupError: null,
    setupNotice: null,
    antennaControllerProfileError: null,
  };
}

export function beginSetupReview(state) {
  return {
    ...state,
    setupStatus: "reviewing",
    setupReview: null,
    setupError: null,
    setupNotice: null,
    antennaControllerProfileError: null,
  };
}

export function setupReviewSucceeded(state, review) {
  return {
    ...state,
    setupStatus: review.valid ? "reviewed" : "invalid",
    setupReview: review,
    setupError: null,
    setupNotice: null,
  };
}

export function setupReviewFailed(state, error) {
  return {
    ...state,
    setupStatus: "error",
    setupReview: null,
    setupError: normalizeOpenError(error),
    setupNotice: null,
  };
}

export function beginSetupCreation(state) {
  return {
    ...state,
    setupStatus: "creating",
    setupError: null,
    setupNotice: null,
  };
}

export function setupCreationCancelled(state) {
  return {
    ...state,
    setupStatus: "reviewed",
    setupError: null,
    setupNotice: "cancelled",
  };
}

export function setupCreationSucceeded(state, session) {
  return {
    ...state,
    activeWorkflow: "run",
    setupStatus: "created",
    setupError: null,
    setupNotice: "created",
    openStatus: "ready",
    session,
    reportPresentationId: session.presentationId
      ?? (session.reportHtml ? state.reportPresentationId + 1 : state.reportPresentationId),
    reportStatus: session.reportHtml ? "ready" : "unavailable",
    reportError: null,
    reportExportStatus: "idle",
    reportExportError: null,
    reportExportNotice: null,
    error: null,
    notice: null,
    exportStatus: "idle",
    exportError: null,
    exportNotice: null,
    exportedBundleName: null,
    importStatus: "idle",
    importKind: null,
    importError: null,
    importNotice: null,
    conductorStatus: "idle",
    conductor: null,
    conductorError: null,
    conductorPendingAction: null,
    conductorNotice: null,
    wsjtxStatus: "idle",
    wsjtx: null,
    wsjtxError: null,
    wsprLiveAcquisitionStatus: "idle",
    wsprLiveAcquisition: null,
    wsprLiveAcquisitionError: null,
  };
}

export function setupCreationFailed(state, error) {
  return {
    ...state,
    setupStatus: state.setupReview?.valid ? "reviewed" : "error",
    setupError: normalizeOpenError(error),
    setupNotice: null,
  };
}

export function openSessionSucceeded(state, session) {
  return {
    ...state,
    activeWorkflow: "report",
    openStatus: "ready",
    session,
    reportPresentationId: session.presentationId
      ?? (session.reportHtml ? state.reportPresentationId + 1 : state.reportPresentationId),
    reportStatus: session.reportHtml ? "ready" : "unavailable",
    reportError: null,
    reportExportStatus: "idle",
    reportExportError: null,
    reportExportNotice: null,
    error: null,
    notice: null,
    exportStatus: "idle",
    exportError: null,
    exportNotice: null,
    exportedBundleName: null,
    importStatus: "idle",
    importKind: null,
    importError: null,
    importNotice: null,
    conductorStatus: "idle",
    conductor: null,
    conductorError: null,
    wsjtxStatus: "idle",
    wsjtx: null,
    wsjtxError: null,
    wsprLiveAcquisitionStatus: "idle",
    wsprLiveAcquisition: null,
    wsprLiveAcquisitionError: null,
  };
}

export function openSessionCancelled(state) {
  return {
    ...state,
    openStatus: state.session ? "ready" : "idle",
    error: null,
    notice: "cancelled",
  };
}

export function openSessionFailed(state, error) {
  return { ...state, openStatus: "error", error: normalizeOpenError(error), notice: null };
}

export function beginExportSession(state) {
  return {
    ...state,
    exportStatus: "loading",
    exportError: null,
    exportNotice: null,
    exportedBundleName: null,
  };
}

export function exportSessionSucceeded(state, bundleName) {
  return {
    ...state,
    exportStatus: "ready",
    exportError: null,
    exportNotice: null,
    exportedBundleName: bundleName,
  };
}

export function exportSessionCancelled(state) {
  return {
    ...state,
    exportStatus: "idle",
    exportError: null,
    exportNotice: "cancelled",
    exportedBundleName: null,
  };
}

export function exportSessionFailed(state, error) {
  return {
    ...state,
    exportStatus: "error",
    exportError: normalizeOpenError(error),
    exportNotice: null,
    exportedBundleName: null,
  };
}

export function beginWsprLiveImport(state) {
  return {
    ...state,
    importStatus: "loading",
    importKind: "wspr_live",
    importError: null,
    importNotice: null,
  };
}

export function wsprLiveImportSucceeded(state, outcome) {
  return {
    ...state,
    importStatus: "ready",
    importError: null,
    importNotice: outcome,
    session: {
      ...state.session,
      ...outcome.session,
      reportHtml: null,
    },
    reportStatus: "unavailable",
    reportError: null,
  };
}

export function wsprLiveImportCancelled(state) {
  return { ...state, importStatus: "idle", importError: null, importNotice: "cancelled" };
}

export function wsprLiveImportFailed(state, error) {
  return {
    ...state,
    importStatus: "error",
    importError: normalizeOpenError(error),
    importNotice: null,
  };
}

export function beginRbnImport(state) {
  return {
    ...state,
    importStatus: "loading",
    importKind: "rbn",
    importError: null,
    importNotice: null,
  };
}

export function rbnImportSucceeded(state, outcome) {
  return wsprLiveImportSucceeded(state, outcome);
}

export function rbnImportCancelled(state) {
  return { ...state, importStatus: "idle", importError: null, importNotice: "cancelled" };
}

export function rbnImportFailed(state, error) {
  return {
    ...state,
    importStatus: "error",
    importError: normalizeOpenError(error),
    importNotice: null,
  };
}

export function normalizeOpenError(error) {
  if (
    error &&
    typeof error === "object" &&
    typeof error.kind === "string" &&
    typeof error.message === "string"
  ) {
    return {
      kind: error.kind,
      message: error.message,
      detail: typeof error.detail === "string" ? error.detail : "",
    };
  }

  return {
    kind: "report_pipeline",
    message: "The local report could not be prepared.",
    detail: error instanceof Error ? error.message : String(error),
  };
}

export function beginConductorLoad(state) {
  return {
    ...state,
    conductorStatus: state.conductor ? "refreshing" : "loading",
    conductorError: null,
  };
}

export function conductorLoadSucceeded(state, conductor) {
  const completedAction = state.conductorPendingAction;
  return {
    ...state,
    conductorStatus: "ready",
    conductor,
    conductorError: null,
    conductorPendingAction: null,
    conductorNotice: completedAction
      ? conductorActionCompletedLabel(completedAction)
      : state.conductorNotice,
  };
}

export function conductorPollSucceeded(state, conductor) {
  return {
    ...state,
    conductor,
  };
}

export function beginConductorMutation(state, action = "operator_action") {
  return {
    ...state,
    conductorStatus: "mutating",
    conductorError: null,
    conductorPendingAction: action,
    conductorNotice: null,
  };
}

export function conductorMutationFailed(state, error) {
  return {
    ...state,
    conductorStatus: "error",
    conductorError: normalizeOpenError(error),
    conductorPendingAction: null,
    conductorNotice: null,
  };
}

export function beginAntennaControllerAction(state, status = "loading") {
  return {
    ...state,
    antennaControllerStatus: status,
    antennaControllerOutcome: null,
    antennaControllerError: null,
    antennaControllerProfileNotice: ["saving", "deleting"].includes(status)
      ? null
      : state.antennaControllerProfileNotice,
    antennaControllerProfileError: ["saving", "deleting"].includes(status)
      ? null
      : state.antennaControllerProfileError,
  };
}

export function antennaControllerProfileSucceeded(state, catalog, notice) {
  return {
    ...state,
    antennaControllerStatus: "ready",
    antennaControllerCatalog: catalog,
    antennaControllerError: null,
    antennaControllerProfileNotice: notice,
    antennaControllerProfileError: null,
  };
}

export function antennaControllerProfileActionFailed(state, error) {
  const normalized = normalizeOpenError(error);
  return {
    ...state,
    antennaControllerStatus: "error",
    antennaControllerError: normalized,
    antennaControllerProfileError: normalized,
  };
}

export function antennaControllerCatalogSucceeded(state, catalog) {
  return {
    ...state,
    antennaControllerStatus: "ready",
    antennaControllerCatalog: catalog,
    antennaControllerError: null,
  };
}

export function antennaControllerViewSucceeded(state, controller) {
  return {
    ...state,
    antennaControllerStatus: "ready",
    antennaController: controller,
    antennaControllerError: null,
  };
}

export function antennaControllerRunSucceeded(state, outcome) {
  return {
    ...state,
    antennaControllerStatus: "ready",
    antennaControllerOutcome: outcome,
    antennaControllerError: null,
  };
}

export function antennaControllerActionFailed(state, error) {
  return {
    ...state,
    antennaControllerStatus: "error",
    antennaControllerError: normalizeOpenError(error),
  };
}

export function beginWsjtxAction(state, action = "refreshing") {
  return { ...state, wsjtxStatus: action, wsjtxError: null };
}

export function wsjtxActionSucceeded(state, status) {
  return { ...state, wsjtxStatus: "ready", wsjtx: status, wsjtxError: null };
}

export function wsjtxActionFailed(state, error) {
  return {
    ...state,
    wsjtxStatus: "error",
    wsjtxError: normalizeOpenError(error),
  };
}

export function beginWsprLiveAcquisition(state) {
  return {
    ...state,
    wsprLiveAcquisitionStatus: "fetching",
    wsprLiveAcquisitionError: null,
  };
}

export function wsprLiveAcquisitionSucceeded(state, outcome) {
  const sessionChanged = ["captured", "completed"].includes(outcome.status);
  return {
    ...state,
    openStatus: sessionChanged ? "ready" : state.openStatus,
    session: sessionChanged ? outcome.session : state.session,
    wsprLiveAcquisitionStatus: "ready",
    wsprLiveAcquisition: outcome,
    wsprLiveAcquisitionError: null,
  };
}

export function wsprLiveAcquisitionFailed(state, error) {
  return {
    ...state,
    wsprLiveAcquisitionStatus: "error",
    wsprLiveAcquisitionError: normalizeOpenError(error),
  };
}

export function beginReportRefresh(state) {
  return { ...state, reportStatus: "refreshing", reportError: null };
}

export function reportRefreshSucceeded(state, presentation) {
  return {
    ...state,
    reportStatus: "ready",
    reportError: null,
    reportPresentationId: presentation.presentationId,
    session: state.session ? {
      ...state.session,
      reportHtml: presentation.reportHtml,
      revision: presentation.revision,
      lifecycle: presentation.lifecycle,
      completeness: presentation.completeness,
      hasControllerEvidence: presentation.hasControllerEvidence,
      presentationId: presentation.presentationId,
      reportAvailable: true,
    } : state.session,
  };
}

export function reportRefreshFailed(state, error) {
  return {
    ...state,
    reportStatus: state.session?.reportHtml ? "ready" : "unavailable",
    reportError: normalizeOpenError(error),
  };
}

export function beginReportExport(state) {
  return {
    ...state,
    reportExportStatus: "loading",
    reportExportError: null,
    reportExportNotice: null,
  };
}

export function reportExportSucceeded(state, outcome) {
  const label = outcome.format === "compact_summary_html"
    ? "compact summary"
    : "full evidence report";
  return {
    ...state,
    reportExportStatus: "ready",
    reportExportError: null,
    reportExportNotice: `${label}: ${outcome.fileName} · revision ${outcome.revision ?? "legacy"}`,
  };
}

export function reportExportCancelled(state) {
  return {
    ...state,
    reportExportStatus: "idle",
    reportExportError: null,
    reportExportNotice: "cancelled",
  };
}

export function reportExportFailed(state, error) {
  return {
    ...state,
    reportExportStatus: "error",
    reportExportError: normalizeOpenError(error),
    reportExportNotice: null,
  };
}

function conductorActionCompletedLabel(action) {
  switch (action) {
    case "start": return "Session started.";
    case "resume": return "Session resumed.";
    case "arm_wspr_cycle": return "Next WSPR cycle scheduled.";
    case "skip_wspr_cycle": return "Cycle skipped.";
    case "interrupt": return "Session paused.";
    case "end": return "Session ended.";
    case "abandon": return "Session abandoned.";
    default: return "Entry saved.";
  }
}
