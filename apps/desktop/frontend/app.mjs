export const WORKFLOWS = Object.freeze(["setup", "run", "transfer", "report"]);

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

export function workflowFromHash(hash) {
  const workflow = hash.replace(/^#/, "");
  return WORKFLOWS.includes(workflow) ? workflow : "setup";
}

export function maidenheadGrid(latitude, longitude) {
  if (!Number.isFinite(latitude) || !Number.isFinite(longitude)) {
    throw new TypeError("Location coordinates must be finite numbers.");
  }
  if (latitude < -90 || latitude > 90 || longitude < -180 || longitude > 180) {
    throw new RangeError("Location coordinates are outside the supported range.");
  }

  const boundedLatitude = latitude === 90 ? 90 - 1e-9 : latitude;
  const boundedLongitude = longitude === 180 ? 180 - 1e-9 : longitude;
  const shiftedLatitude = boundedLatitude + 90;
  const shiftedLongitude = boundedLongitude + 180;
  const fieldLongitude = Math.floor(shiftedLongitude / 20);
  const fieldLatitude = Math.floor(shiftedLatitude / 10);
  const squareLongitude = Math.floor((shiftedLongitude % 20) / 2);
  const squareLatitude = Math.floor(shiftedLatitude % 10);
  const subsquareLongitude = Math.floor((shiftedLongitude % 2) * 12);
  const subsquareLatitude = Math.floor((shiftedLatitude % 1) * 24);

  return `${String.fromCharCode(65 + fieldLongitude)}${String.fromCharCode(65 + fieldLatitude)}${squareLongitude}${squareLatitude}${String.fromCharCode(65 + subsquareLongitude)}${String.fromCharCode(65 + subsquareLatitude)}`;
}

export function currentPosition(geolocation) {
  return new Promise((resolve, reject) => {
    if (typeof geolocation?.getCurrentPosition !== "function") {
      reject(new Error("System location is unavailable in this desktop environment."));
      return;
    }
    geolocation.getCurrentPosition(resolve, reject, {
      enableHighAccuracy: false,
      timeout: 10_000,
      maximumAge: 300_000,
    });
  });
}

export function locationErrorMessage(error) {
  switch (error?.code) {
    case 1: return "Location permission was not granted. Enter the grid manually instead.";
    case 2: return "The system could not determine a location. Enter the grid manually instead.";
    case 3: return "The location request timed out. Enter the grid manually or try again.";
    default: return error?.message || "Location is unavailable. Enter the grid manually instead.";
  }
}

export function wsprRunPlanSummary(roundsValue, antennaCount) {
  const normalizedRounds = typeof roundsValue === "number"
    ? roundsValue
    : Number(String(roundsValue).trim());
  if (
    String(roundsValue).trim().length === 0
    || !Number.isSafeInteger(normalizedRounds)
    || normalizedRounds <= 0
    || !Number.isSafeInteger(antennaCount)
    || antennaCount <= 0
  ) {
    return null;
  }
  const cycles = normalizedRounds * antennaCount;
  const minimumMinutes = cycles * 2;
  if (!Number.isSafeInteger(cycles) || !Number.isSafeInteger(minimumMinutes)) return null;
  return {
    rounds: normalizedRounds,
    antennaCount,
    cycles,
    minimumMinutes,
    text: `${cycles} WSPR ${cycles === 1 ? "cycle" : "cycles"} · at least ${minimumMinutes} ${minimumMinutes === 1 ? "minute" : "minutes"}`,
  };
}

export function conductorActionAvailable(view, action) {
  if (action === "arm_wspr_cycle") {
    return view.lifecycle === "running"
      && view.nextIntent !== null
      && ["between_slots", "switching"].includes(view.phase);
  }
  if (action === "skip_wspr_cycle") {
    return view.lifecycle === "running"
      && view.nextIntent !== null
      && ["between_slots", "switching"].includes(view.phase);
  }
  return lifecycleActionAvailability(view.lifecycle).has(action)
    && !(view.phase === "finalizing" && action === "end");
}

export function createCountdownAnchor(view, sampledAtMilliseconds) {
  if (view?.secondsToTransition === null || view?.secondsToTransition === undefined) return null;
  const seconds = Math.max(0, Math.floor(Number(view.secondsToTransition)));
  const sampledAt = Number(sampledAtMilliseconds);
  if (!Number.isFinite(seconds) || !Number.isFinite(sampledAt)) return null;
  return {
    key: [
      view.sessionId,
      view.revision,
      view.actionToken,
      view.lifecycle,
      view.phase,
      view.currentSlot?.slotId ?? "",
      view.nextSlot?.slotId ?? "",
      seconds,
    ].join(":"),
    seconds,
    sampledAtMilliseconds: sampledAt,
  };
}

export function projectCountdown(anchor, nowMilliseconds) {
  if (!anchor) return null;
  const now = Number(nowMilliseconds);
  if (!Number.isFinite(now)) return anchor.seconds;
  const elapsedSeconds = Math.floor(Math.max(0, now - anchor.sampledAtMilliseconds) / 1000);
  return Math.max(0, anchor.seconds - elapsedSeconds);
}

export function formatActiveRunTime(value, options = {}) {
  const instant = new Date(value);
  const now = new Date(options.now ?? Date.now());
  const locale = options.locale;
  const timeZone = options.timeZone;
  const dayFormatter = new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "numeric",
    day: "numeric",
    timeZone,
  });
  const sameDay = dayFormatter.format(instant) === dayFormatter.format(now);
  return new Intl.DateTimeFormat(locale, sameDay
    ? { hour: "numeric", minute: "2-digit", timeZone }
    : { month: "short", day: "numeric", hour: "numeric", minute: "2-digit", timeZone }
  ).format(instant);
}

export function viewModel(state) {
  return WORKFLOWS.map((workflow) => ({
    workflow,
    active: workflow === state.activeWorkflow,
  }));
}

export function beginOpenSession(state) {
  return { ...state, openStatus: "loading", error: null, notice: null };
}

export function editSessionSetup(state) {
  if (state.setupStatus === "editing" && state.setupReview === null) return state;
  return {
    ...state,
    setupStatus: "editing",
    setupReview: null,
    setupError: null,
    setupNotice: null,
  };
}

export function beginSetupReview(state) {
  return {
    ...state,
    setupStatus: "reviewing",
    setupReview: null,
    setupError: null,
    setupNotice: null,
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

export function invokeOpenSession(invoke) {
  return invoke("open_session_bundle");
}

export function invokeReviewSessionSetup(invoke, draft) {
  return invoke("review_session_setup", { draft });
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

export function updateReportFrame(reportFrame, state) {
  if (state.session === null || typeof state.session.reportHtml !== "string") return false;

  const presentationId = String(state.reportPresentationId);
  if (reportFrame.dataset.presentationId === presentationId) return false;

  reportFrame.srcdoc = state.session.reportHtml;
  reportFrame.dataset.presentationId = presentationId;
  return true;
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
  return {
    ...state,
    reportExportStatus: "ready",
    reportExportError: null,
    reportExportNotice: `${outcome.fileName} · revision ${outcome.revision ?? "legacy"}`,
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

function mount(root, browserWindow) {
  let state = initialState(workflowFromHash(browserWindow.location.hash));
  let countdownAnchor = null;
  let countdownAnchorKey = null;
  let transitionRefreshKey = null;
  const monotonicNow = () => browserWindow.performance?.now?.() ?? Date.now();
  const navigation = [...root.querySelectorAll("[data-workflow]")];
  const panels = [...root.querySelectorAll("[data-panel]")];
  const setupForm = root.querySelector("[data-setup-form]");
  const setupStatus = root.querySelector("[data-setup-status]");
  const setupReviewButton = root.querySelector("[data-review-setup]");
  const setupCreateButton = root.querySelector("[data-create-session]");
  const setupAddAntennaButton = root.querySelector("[data-add-antenna]");
  const useCurrentLocationButton = root.querySelector("[data-use-current-location]");
  const locationStatus = root.querySelector("[data-location-status]");
  const stationGrid = root.querySelector('[data-setup-field="grid"]');
  const setupAntennaTemplate = root.querySelector("[data-antenna-template]");
  const setupFeedback = root.querySelector("[data-setup-feedback]");
  const setupFeedbackMessage = root.querySelector("[data-setup-feedback-message]");
  const setupFeedbackDetail = root.querySelector("[data-setup-feedback-detail]");
  const setupDiagnostics = root.querySelector("[data-setup-diagnostics]");
  const setupReviewPanel = root.querySelector("[data-setup-review]");
  const setupReviewStation = root.querySelector("[data-review-station]");
  const setupReviewAntennas = root.querySelector("[data-review-antennas]");
  const setupReviewShape = root.querySelector("[data-review-shape]");
  const setupReviewSlots = root.querySelector("[data-review-slots]");
  const setupRunPlanSummary = root.querySelector("[data-run-plan-summary]");
  const conductorPanel = root.querySelector("[data-conductor]");
  const conductorEmpty = root.querySelector("[data-conductor-empty]");
  const conductorStatus = root.querySelector("[data-conductor-status]");
  const conductorRevision = root.querySelector("[data-conductor-revision]");
  const conductorLifecycle = root.querySelector("[data-conductor-lifecycle]");
  const conductorNow = root.querySelector("[data-conductor-now]");
  const conductorAntennaInUse = root.querySelector("[data-conductor-antenna-in-use]");
  const conductorPhase = root.querySelector("[data-conductor-phase]");
  const conductorGuidance = root.querySelector("[data-conductor-guidance]");
  const conductorCountdown = root.querySelector("[data-conductor-countdown]");
  const currentSlot = root.querySelector("[data-current-slot]");
  const nextSlot = root.querySelector("[data-next-slot]");
  const conductorRefreshButtons = [...root.querySelectorAll("[data-conductor-refresh]")];
  const lifecycleButtons = [...root.querySelectorAll("[data-conductor-action]")];
  const evidenceForm = root.querySelector("[data-evidence-form]");
  const entryPanel = root.querySelector("[data-entry-panel]");
  const correctionsPanel = root.querySelector("[data-corrections-panel]");
  const addRunNote = root.querySelector("[data-add-run-note]");
  const openCorrections = root.querySelector("[data-open-corrections]");
  const evidenceKind = root.querySelector("[data-evidence-kind]");
  const evidenceSlot = root.querySelector("[data-evidence-slot]");
  const evidenceAntenna = root.querySelector("[data-evidence-antenna]");
  const evidenceFrequency = root.querySelector("[data-evidence-frequency]");
  const evidenceMode = root.querySelector("[data-evidence-mode]");
  const evidencePower = root.querySelector("[data-evidence-power]");
  const evidenceCallsign = root.querySelector("[data-evidence-callsign]");
  const evidenceCadence = root.querySelector("[data-evidence-cadence]");
  const evidenceDetail = root.querySelector("[data-evidence-detail]");
  const conductorFeedback = root.querySelector("[data-conductor-feedback]");
  const conductorFeedbackMessage = root.querySelector("[data-conductor-feedback-message]");
  const conductorFeedbackDetail = root.querySelector("[data-conductor-feedback-detail]");
  const conductorDiagnostics = root.querySelector("[data-conductor-diagnostics]");
  const conductorEvents = root.querySelector("[data-conductor-events]");
  const wsprLivePhase = root.querySelector("[data-wspr-live-phase]");
  const wsprLiveDetail = root.querySelector("[data-wspr-live-detail]");
  const wsprLiveDiagnostic = root.querySelector("[data-wspr-live-diagnostic]");
  const wsprLiveRetry = root.querySelector("[data-wspr-live-retry]");
  const wsprLiveEndWithout = root.querySelector("[data-wspr-live-end-without]");
  const wsjtxForm = root.querySelector("[data-wsjtx-form]");
  const wsjtxBindAddress = root.querySelector("[data-wsjtx-bind-address]");
  const wsjtxPort = root.querySelector("[data-wsjtx-port]");
  const wsjtxClientId = root.querySelector("[data-wsjtx-client-id]");
  const wsjtxStart = root.querySelector("[data-wsjtx-start]");
  const wsjtxStop = root.querySelector("[data-wsjtx-stop]");
  const wsjtxPhase = root.querySelector("[data-wsjtx-phase]");
  const wsjtxCounts = root.querySelector("[data-wsjtx-counts]");
  const wsjtxDiagnostic = root.querySelector("[data-wsjtx-diagnostic]");
  const openButton = root.querySelector("[data-open-session]");
  const exportButton = root.querySelector("[data-export-session]");
  const importWsprLiveButton = root.querySelector("[data-import-wspr-live]");
  const importRbnButton = root.querySelector("[data-import-rbn]");
  const transferStatus = root.querySelector("[data-transfer-status]");
  const openFeedback = root.querySelector("[data-open-feedback]");
  const feedbackMessage = root.querySelector("[data-feedback-message]");
  const feedbackDetail = root.querySelector("[data-feedback-detail]");
  const exportFeedback = root.querySelector("[data-export-feedback]");
  const exportFeedbackMessage = root.querySelector("[data-export-feedback-message]");
  const exportFeedbackDetail = root.querySelector("[data-export-feedback-detail]");
  const importFeedback = root.querySelector("[data-import-feedback]");
  const importFeedbackMessage = root.querySelector("[data-import-feedback-message]");
  const importFeedbackDetail = root.querySelector("[data-import-feedback-detail]");
  const reportStatus = root.querySelector("[data-report-status]");
  const reportPlaceholder = root.querySelector("[data-report-placeholder]");
  const reportViewer = root.querySelector("[data-report-viewer]");
  const reportFrame = root.querySelector("[data-report-frame]");
  const reportBundleName = root.querySelector("[data-report-bundle]");
  const reportRevision = root.querySelector("[data-report-revision]");
  const reportSummary = root.querySelector("[data-report-summary]");
  const reportRefreshButton = root.querySelector("[data-report-refresh]");
  const reportExportButton = root.querySelector("[data-report-export]");
  const reportFeedback = root.querySelector("[data-report-feedback]");
  const reportFeedbackMessage = root.querySelector("[data-report-feedback-message]");
  const reportFeedbackDetail = root.querySelector("[data-report-feedback-detail]");

  const render = () => {
    for (const item of viewModel(state)) {
      const button = navigation.find(
        (candidate) => candidate.dataset.workflow === item.workflow,
      );
      const panel = panels.find(
        (candidate) => candidate.dataset.panel === item.workflow,
      );

      button.classList.toggle("active", item.active);
      button.setAttribute("aria-current", item.active ? "page" : "false");
      panel.hidden = !item.active;
    }

    const setupBusy = ["reviewing", "creating"].includes(state.setupStatus);
    setupForm.setAttribute("aria-busy", String(setupBusy));
    setupReviewButton.disabled = setupBusy;
    setupReviewButton.textContent = state.setupStatus === "reviewing"
      ? "Validating…"
      : "Review normalized plan";
    setupCreateButton.disabled = state.setupStatus !== "reviewed";
    setupCreateButton.textContent = state.setupStatus === "creating"
      ? "Creating…"
      : "Create session";
    setupStatus.textContent = setupStatusText(state);
    setupStatus.classList.toggle(
      "muted",
      ["editing", "invalid", "error"].includes(state.setupStatus),
    );

    const setupFeedbackState = setupFeedbackModel(state);
    setupFeedback.hidden = setupFeedbackState === null;
    if (setupFeedbackState) {
      setupFeedback.dataset.kind = setupFeedbackState.kind;
      setupFeedbackMessage.textContent = setupFeedbackState.message;
      setupFeedbackDetail.textContent = setupFeedbackState.detail;
      setupFeedbackDetail.hidden = setupFeedbackState.detail.length === 0;
    }

    const diagnostics = state.setupReview?.diagnostics ?? [];
    setupDiagnostics.replaceChildren(
      ...diagnostics.map((diagnostic) => {
        const item = root.createElement("li");
        const field = root.createElement("strong");
        field.textContent = diagnostic.field;
        const message = root.createElement("span");
        message.textContent = `${diagnostic.message} (${diagnostic.code})`;
        item.append(field, message);
        return item;
      }),
    );
    setupDiagnostics.hidden = diagnostics.length === 0;

    const plan = state.setupReview?.plan ?? null;
    setupReviewPanel.hidden = plan === null;
    if (plan) {
      setupReviewStation.textContent = `${plan.station.callsign} · ${plan.station.grid} · ${plan.station.powerWatts ?? "power not set"}${plan.station.powerWatts === null ? "" : " W"}`;
      setupReviewAntennas.textContent = plan.antennas
        .map((antenna, index) => `${String.fromCharCode(65 + index)}: ${antenna.label}${antenna.context ? ` — ${antenna.context}` : ""}`)
        .join("\n");
      const signalSummary = plan.signalPlan
        ? `${humanizeIdentifier(plan.signalPlan.mode)} · ${humanizeIdentifier(plan.signalPlan.collectionProfile)} · ${plan.signalPlan.frequenciesHz.length} frequencies`
        : `WSPR.live ${plan.wsprLiveAcquisitionEnabled ? "enabled" : "off"}`;
      const runLength = plan.signalPlan
        ? `${plan.slots.length} planned signal slots`
        : `${plan.slots.length} WSPR cycles · at least ${plan.slots.length * 2} minutes`;
      setupReviewShape.textContent = `${humanizeIdentifier(plan.mode)} · ${humanizeIdentifier(plan.goal)} · ${runLength} · ${signalSummary}`;
      setupReviewSlots.replaceChildren(
        ...plan.slots.map((slot) => {
          const row = root.createElement("tr");
          for (const value of [
            slot.sequenceNumber,
            slot.antennaLabel,
            slot.band,
            slot.signal
              ? `${slot.signal.frequencyHz} Hz · ${slot.signal.frequencyVariantId} · ${slot.signal.counterbalanceBlockId}/${slot.signal.counterbalancePosition}`
              : "—",
          ]) {
            const cell = root.createElement("td");
            cell.textContent = String(value);
            row.append(cell);
          }
          return row;
        }),
      );
    }

    const conductorBusy = ["loading", "refreshing", "mutating"].includes(state.conductorStatus);
    const hasConductor = state.conductor !== null;
    conductorPanel.hidden = !hasConductor;
    conductorEmpty.hidden = hasConductor;
    conductorStatus.textContent = conductorStatusText(state);
    conductorStatus.classList.toggle("muted", !hasConductor || state.conductorStatus === "error");
    conductorRefreshButtons.forEach((button) => { button.disabled = conductorBusy; });
    evidenceForm.setAttribute("aria-busy", String(conductorBusy));
    evidenceForm.querySelector("button[type=submit]").disabled = conductorBusy || !hasConductor;

    const conductorFeedbackState = conductorFeedbackModel(state);
    conductorFeedback.hidden = conductorFeedbackState === null;
    if (conductorFeedbackState) {
      conductorFeedback.dataset.kind = conductorFeedbackState.kind;
      conductorFeedbackMessage.textContent = conductorFeedbackState.message;
      conductorFeedbackDetail.textContent = conductorFeedbackState.detail;
      conductorFeedbackDetail.hidden = conductorFeedbackState.detail.length === 0;
    }

    if (hasConductor) {
      const view = state.conductor;
      const nextAnchor = createCountdownAnchor(view, monotonicNow());
      if (nextAnchor?.key !== countdownAnchorKey) {
        countdownAnchor = nextAnchor;
        countdownAnchorKey = nextAnchor?.key ?? null;
        transitionRefreshKey = null;
      }
      conductorRevision.textContent = `${view.bundleName} · revision ${view.revision}`;
      conductorLifecycle.textContent = humanizeIdentifier(view.lifecycle);
      conductorNow.textContent = formatReviewTime(view.now);
      conductorAntennaInUse.textContent = view.antennaInUse ?? "None";
      conductorPhase.textContent = humanizeIdentifier(view.phase);
      conductorGuidance.textContent = view.guidance;
      const projectedSeconds = state.conductorStatus === "ready"
        ? projectCountdown(countdownAnchor, monotonicNow())
        : view.secondsToTransition;
      conductorCountdown.textContent = projectedSeconds === null
        ? ""
        : formatCountdown(projectedSeconds);
      renderSlot(currentSlot, view.currentSlot, root, view.now);
      if (view.nextSlot) renderSlot(nextSlot, view.nextSlot, root, view.now);
      else renderIntent(nextSlot, view.nextIntent, root);
      replaceSelectOptions(evidenceSlot, [
        { value: "", label: "No slot / session note" },
        ...view.slots.map((slot) => ({
          value: slot.slotId,
          label: `#${slot.sequenceNumber} · ${slot.plannedAntenna} · ${slot.band}`,
        })),
      ]);
      replaceSelectOptions(
        evidenceAntenna,
        view.antennas.map((antenna) => ({ value: antenna, label: antenna })),
      );

      const evidenceAllowed = ["running", "interrupted"].includes(view.lifecycle);
      evidenceForm.querySelector("button[type=submit]").disabled = conductorBusy || !evidenceAllowed;
      lifecycleButtons.forEach((button) => {
        const action = button.dataset.conductorAction;
        const isArmAction = action === "arm_wspr_cycle";
        if (isArmAction && view.nextIntent) {
          button.textContent = `${view.nextIntent.antennaLabel} ready`;
        }
        const available = conductorActionAvailable(view, action);
        button.hidden = !available;
        button.disabled = conductorBusy
          || !available;
      });
      conductorDiagnostics.replaceChildren(
        ...view.diagnostics.map((diagnostic) => {
          const item = root.createElement("li");
          const code = root.createElement("strong");
          code.textContent = diagnostic.slotId
            ? `${diagnostic.code} · ${diagnostic.slotId}`
            : diagnostic.code;
          const message = root.createElement("span");
          message.textContent = diagnostic.message;
          item.append(code, message);
          return item;
        }),
      );
      conductorDiagnostics.hidden = view.diagnostics.length === 0;
      conductorEvents.replaceChildren(
        ...view.effectiveEvents.map((event) => conductorEventElement(
          root,
          event,
          conductorBusy || !evidenceAllowed,
        )),
      );
      const wsjtxBusy = ["refreshing", "starting", "stopping"].includes(state.wsjtxStatus);
      const wsjtxRunning = ["running", "stale"].includes(state.wsjtx?.phase);
      wsjtxForm.setAttribute("aria-busy", String(wsjtxBusy));
      wsjtxStart.disabled = conductorBusy || wsjtxBusy || wsjtxRunning || view.lifecycle !== "running";
      wsjtxStop.disabled = conductorBusy || wsjtxBusy || !wsjtxRunning;
      wsjtxPhase.textContent = state.wsjtx
        ? `${humanizeIdentifier(state.wsjtx.phase)}${state.wsjtx.bindAddress ? ` · ${state.wsjtx.bindAddress}` : ""}`
        : "Not started";
      wsjtxCounts.textContent = state.wsjtx
        ? `${state.wsjtx.receivedDatagrams} received · ${state.wsjtx.committedMutations} committed · ${state.wsjtx.ignoredDatagrams} explicit non-observation disposition(s)`
        : "Manual operation remains available without WSJT-X.";
      const adapterDiagnostic = state.wsjtxError ?? state.wsjtx?.diagnostic ?? null;
      wsjtxDiagnostic.hidden = adapterDiagnostic === null;
      if (adapterDiagnostic) {
        wsjtxDiagnostic.textContent = adapterDiagnostic.message ?? adapterDiagnostic.detail;
        if (adapterDiagnostic.code) wsjtxDiagnostic.textContent += ` (${adapterDiagnostic.code})`;
      }
      const wsprLiveModel = wsprLiveAcquisitionModel(state);
      wsprLivePhase.textContent = wsprLiveModel.phase;
      wsprLiveDetail.textContent = wsprLiveModel.detail;
      wsprLiveDiagnostic.hidden = wsprLiveModel.diagnostic.length === 0;
      wsprLiveDiagnostic.textContent = wsprLiveModel.diagnostic;
      wsprLiveRetry.hidden = !wsprLiveModel.retry;
      wsprLiveRetry.disabled = conductorBusy || state.wsprLiveAcquisitionStatus === "fetching";
      wsprLiveEndWithout.hidden = !wsprLiveModel.endWithout;
      wsprLiveEndWithout.disabled = conductorBusy || state.wsprLiveAcquisitionStatus === "fetching";
    } else {
      lifecycleButtons.forEach((button) => { button.disabled = true; });
      conductorDiagnostics.replaceChildren();
      conductorEvents.replaceChildren();
      wsjtxStart.disabled = true;
      wsjtxStop.disabled = true;
    }

    openButton.disabled = state.openStatus === "loading";
    openButton.textContent = state.openStatus === "loading" ? "Opening…" : "Choose bundle";
    const exportLoading = state.exportStatus === "loading";
    const importLoading = state.importStatus === "loading";
    exportButton.disabled = state.session === null || state.openStatus === "loading" || exportLoading;
    exportButton.textContent = state.session === null
      ? "Open a bundle first"
      : exportLoading
        ? "Exporting…"
        : "Export copy";
    importWsprLiveButton.disabled = state.session?.lifecycle !== "running" || importLoading;
    importWsprLiveButton.textContent = state.session?.lifecycle !== "running"
      ? "Open a running session first"
      : importLoading
        ? "Importing…"
        : "Choose WSPR.live JSON";
    const rbnEligible = state.session?.schemaVersion === 3
      && !["draft", "ready"].includes(state.session?.lifecycle);
    importRbnButton.disabled = !rbnEligible || importLoading;
    importRbnButton.textContent = state.session === null
      ? "Open a session first"
      : state.session.schemaVersion !== 3
        ? "This older session cannot import RBN evidence"
        : !rbnEligible
          ? "Start the session first"
          : importLoading
            ? "Importing…"
            : "Choose RBN ZIP";
    transferStatus.textContent = transferStatusText(state);
    transferStatus.classList.toggle("muted", state.openStatus !== "ready");

    const feedback = openFeedbackModel(state);
    openFeedback.hidden = feedback === null;
    if (feedback) {
      openFeedback.dataset.kind = feedback.kind;
      feedbackMessage.textContent = feedback.message;
      feedbackDetail.textContent = feedback.detail;
      feedbackDetail.hidden = feedback.detail.length === 0;
    }

    const exportFeedbackState = exportFeedbackModel(state);
    exportFeedback.hidden = exportFeedbackState === null;
    if (exportFeedbackState) {
      exportFeedback.dataset.kind = exportFeedbackState.kind;
      exportFeedbackMessage.textContent = exportFeedbackState.message;
      exportFeedbackDetail.textContent = exportFeedbackState.detail;
      exportFeedbackDetail.hidden = exportFeedbackState.detail.length === 0;
    }

    const importFeedbackState = importFeedbackModel(state);
    importFeedback.hidden = importFeedbackState === null;
    if (importFeedbackState) {
      importFeedback.dataset.kind = importFeedbackState.kind;
      importFeedbackMessage.textContent = importFeedbackState.message;
      importFeedbackDetail.textContent = importFeedbackState.detail;
      importFeedbackDetail.hidden = importFeedbackState.detail.length === 0;
    }

    const hasSession = state.session !== null;
    const hasReport = typeof state.session?.reportHtml === "string";
    const reportBusy = state.reportStatus === "refreshing" || state.reportExportStatus === "loading";
    reportStatus.textContent = state.reportStatus === "refreshing"
      ? "Refreshing"
      : hasReport
        ? `${humanizeIdentifier(state.session.completeness ?? "full_detail")} · revision ${state.session.revision ?? "legacy"}`
        : "Unavailable";
    reportStatus.classList.toggle("muted", !hasReport);
    reportPlaceholder.hidden = hasSession;
    reportViewer.hidden = !hasSession;
    reportFrame.hidden = !hasReport;
    reportRefreshButton.disabled = reportBusy;
    reportExportButton.disabled = reportBusy || !hasReport;
    reportRefreshButton.textContent = state.reportStatus === "refreshing"
      ? "Refreshing…"
      : "Refresh committed snapshot";
    reportExportButton.textContent = state.reportExportStatus === "loading"
      ? "Exporting…"
      : "Export this HTML snapshot";
    const reportFeedbackState = reportFeedbackModel(state);
    reportFeedback.hidden = reportFeedbackState === null;
    if (reportFeedbackState) {
      reportFeedback.dataset.kind = reportFeedbackState.kind;
      reportFeedbackMessage.textContent = reportFeedbackState.message;
      reportFeedbackDetail.textContent = reportFeedbackState.detail;
      reportFeedbackDetail.hidden = reportFeedbackState.detail.length === 0;
    }
    if (hasSession) {
      reportBundleName.textContent = state.session.bundleName;
      reportRevision.textContent = `Revision ${state.session.revision ?? "legacy"} · ${humanizeIdentifier(state.session.lifecycle ?? "static")}`;
      reportSummary.textContent = `${state.session.callsign} · ${state.session.grid} · ${state.session.antennaCount} antennas · ${state.session.slotCount} slots · ${state.session.observationCount} observations`;
      if (hasReport) updateReportFrame(reportFrame, state);
    }
  };

  const advanceWsprLive = async (retry = false) => {
    if (
      state.conductor?.lifecycle !== "running"
      || state.wsprLiveAcquisitionStatus === "fetching"
    ) return;
    state = beginWsprLiveAcquisition(state);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") throw new Error("The native desktop bridge is unavailable.");
      const outcome = await invokeAdvanceSessionWsprLive(invoke, retry);
      state = wsprLiveAcquisitionSucceeded(state, outcome);
      render();
      if (["captured", "completed"].includes(outcome.status)) {
        await refreshConductor(false);
        await refreshReport();
      }
    } catch (error) {
      state = wsprLiveAcquisitionFailed(state, error);
      render();
    }
  };

  const refreshConductor = async (advanceAcquisition = true) => {
    if (["loading", "refreshing", "mutating"].includes(state.conductorStatus)) return;
    state = beginConductorLoad(state);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") {
        throw new Error("The native desktop bridge is unavailable.");
      }
      state = conductorLoadSucceeded(
        state,
        await invokeActiveSessionConductor(invoke),
      );
    } catch (error) {
      state = conductorMutationFailed(state, error);
    }
    render();
    if (state.conductor) {
      await refreshWsjtxStatus();
      if (advanceAcquisition && state.conductor.lifecycle === "running") {
        await advanceWsprLive();
      }
    }
  };

  const refreshWsjtxStatus = async () => {
    if (["starting", "stopping"].includes(state.wsjtxStatus)) return;
    state = beginWsjtxAction(state);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") throw new Error("The native desktop bridge is unavailable.");
      state = wsjtxActionSucceeded(state, await invokeActiveSessionWsjtxStatus(invoke));
    } catch (error) {
      state = wsjtxActionFailed(state, error);
    }
    render();
  };

  const refreshReport = async () => {
    if (
      !state.session
      || state.reportStatus === "refreshing"
      || state.reportExportStatus === "loading"
    ) return;
    state = beginReportRefresh(state);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") throw new Error("The native desktop bridge is unavailable.");
      state = reportRefreshSucceeded(
        state,
        await invokeRefreshActiveSessionReport(invoke),
      );
    } catch (error) {
      state = reportRefreshFailed(state, error);
    }
    render();
  };

  const submitConductorAction = async (action) => {
    if (!state.conductor || state.conductorStatus === "mutating") return;
    const request = {
      actionToken: state.conductor.actionToken,
      expectedRevision: state.conductor.revision,
      action,
    };
    state = beginConductorMutation(state, action.kind);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") {
        throw new Error("The native desktop bridge is unavailable.");
      }
      state = conductorLoadSucceeded(
        state,
        await invokeMutateSessionConductor(invoke, request),
      );
    } catch (error) {
      state = conductorMutationFailed(state, error);
    }
    render();
    if (state.conductorStatus === "ready") {
      await refreshWsjtxStatus();
      await advanceWsprLive();
      await refreshReport();
    }
  };

  for (const button of navigation) {
    button.addEventListener("click", async () => {
      state = selectWorkflow(state, button.dataset.workflow);
      browserWindow.history.replaceState(null, "", `#${state.activeWorkflow}`);
      render();
      root.querySelector("main").focus({ preventScroll: true });
      if (state.activeWorkflow === "run" && state.session) await refreshConductor();
      if (state.activeWorkflow === "report" && state.session) await refreshReport();
    });
  }

  browserWindow.addEventListener("hashchange", async () => {
    state = selectWorkflow(
      state,
      workflowFromHash(browserWindow.location.hash),
    );
    render();
    if (state.activeWorkflow === "run" && state.session) await refreshConductor();
    if (state.activeWorkflow === "report" && state.session) await refreshReport();
  });

  const refreshConductorOnReturn = () => {
    if (
      root.ownerDocument.visibilityState !== "hidden"
      && state.activeWorkflow === "run"
      && state.session
    ) {
      void refreshConductor();
    }
  };
  browserWindow.addEventListener("focus", refreshConductorOnReturn);
  root.ownerDocument.addEventListener?.("visibilitychange", refreshConductorOnReturn);

  conductorRefreshButtons.forEach((button) => {
    button.addEventListener("click", refreshConductor);
  });

  wsprLiveRetry.addEventListener("click", () => advanceWsprLive(true));
  wsprLiveEndWithout.addEventListener("click", async () => {
    if (!browserWindow.confirm("End this session without the final automatic WSPR.live capture? Existing evidence will remain.")) return;
    await submitConductorAction({
      kind: "end",
      reason: "Operator ended finalization without automatic WSPR.live public spots.",
    });
  });

  lifecycleButtons.forEach((button) => {
    button.addEventListener("click", async () => {
      const kind = button.dataset.conductorAction;
      if (kind === "arm_wspr_cycle") {
        const intent = state.conductor?.nextIntent;
        if (!intent) return;
        await submitConductorAction({
          kind,
          intentId: intent.intentId,
          antennaLabel: intent.antennaLabel,
        });
        return;
      }
      if (kind === "skip_wspr_cycle") {
        const intent = state.conductor?.nextIntent;
        if (!intent) return;
        const reason = browserWindow.prompt(`Optional reason for skipping cycle ${intent.sequenceNumber}:`, "");
        if (reason === null) return;
        await submitConductorAction({
          kind,
          intentId: intent.intentId,
          reason,
        });
        return;
      }
      if (kind === "start" || kind === "resume") {
        await submitConductorAction({ kind, note: null });
        return;
      }
      if (kind === "abandon" && !browserWindow.confirm("Abandon this session? Existing evidence will remain, but the lifecycle is terminal.")) return;
      const detail = browserWindow.prompt(`Optional ${kind} reason:`, "");
      if (detail === null) return;
      await submitConductorAction({ kind, reason: detail });
    });
  });

  evidenceCallsign.addEventListener("input", () => {
    evidenceCallsign.value = evidenceCallsign.value.toUpperCase();
  });

  addRunNote.addEventListener("click", () => {
    entryPanel.open = true;
    evidenceKind.value = "add_note";
    evidenceDetail.focus();
  });

  openCorrections.addEventListener("click", () => {
    entryPanel.open = true;
    correctionsPanel.open = true;
    correctionsPanel.scrollIntoView?.({ behavior: "smooth", block: "start" });
  });

  evidenceForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    await submitConductorAction(readEvidenceAction(
      evidenceKind.value,
      evidenceSlot.value,
      evidenceAntenna.value,
      evidenceDetail.value,
      readSignalEvidenceFields(
        evidenceFrequency,
        evidenceMode,
        evidencePower,
        evidenceCallsign,
        evidenceCadence,
      ),
    ));
  });

  conductorEvents.addEventListener("click", async (event) => {
    const button = event.target.closest("button[data-event-action]");
    if (!button) return;
    const targetEventId = button.dataset.eventId;
    const reason = browserWindow.prompt("Correction reason (required):", "");
    if (reason === null || reason.trim().length === 0) return;
    if (button.dataset.eventAction === "retract") {
      await submitConductorAction({
        kind: "retract_event",
        targetEventId,
        reason,
      });
      return;
    }
    const replacement = readEvidenceReplacement(
      evidenceKind.value,
      evidenceAntenna.value,
      evidenceDetail.value,
      readSignalEvidenceFields(
        evidenceFrequency,
        evidenceMode,
        evidencePower,
        evidenceCallsign,
        evidenceCadence,
      ),
    );
    await submitConductorAction({
      kind: "replace_event",
      targetEventId,
      slotId: evidenceSlot.value || null,
      replacement,
      reason,
    });
  });

  wsjtxStart.addEventListener("click", async () => {
    state = beginWsjtxAction(state, "starting");
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") throw new Error("The native desktop bridge is unavailable.");
      state = wsjtxActionSucceeded(state, await invokeStartSessionWsjtx(invoke, {
        bindAddress: wsjtxBindAddress.value,
        port: Number(wsjtxPort.value),
        expectedClientId: wsjtxClientId.value,
      }));
    } catch (error) {
      state = wsjtxActionFailed(state, error);
    }
    render();
  });

  wsjtxStop.addEventListener("click", async () => {
    state = beginWsjtxAction(state, "stopping");
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") throw new Error("The native desktop bridge is unavailable.");
      state = wsjtxActionSucceeded(state, await invokeStopSessionWsjtx(invoke));
    } catch (error) {
      state = wsjtxActionFailed(state, error);
    }
    render();
  });

  setupForm.addEventListener("input", (event) => {
    if (event.target.matches?.('[data-setup-field="callsign"], [data-setup-field="signalTransmittedCallsign"]')) {
      event.target.value = event.target.value.toUpperCase();
    }
    syncSignalPlanFields(setupForm);
    updateWsprRunPlanSummary(setupForm, setupRunPlanSummary);
    if (!setupBusyState(state)) {
      state = editSessionSetup(state);
      render();
    }
  });

  useCurrentLocationButton.addEventListener("click", async () => {
    useCurrentLocationButton.disabled = true;
    useCurrentLocationButton.textContent = "Locating…";
    locationStatus.textContent = "Requesting system location…";
    try {
      const position = await currentPosition(browserWindow.navigator?.geolocation);
      stationGrid.value = maidenheadGrid(
        position.coords.latitude,
        position.coords.longitude,
      );
      state = editSessionSetup(state);
      render();
      locationStatus.textContent = `Estimated ${stationGrid.value}; raw coordinates were not saved.`;
    } catch (error) {
      locationStatus.textContent = locationErrorMessage(error);
    } finally {
      useCurrentLocationButton.disabled = false;
      useCurrentLocationButton.textContent = "Use current location";
    }
  });

  setupForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    state = beginSetupReview(state);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") {
        throw new Error("The native desktop bridge is unavailable.");
      }
      const review = await invokeReviewSessionSetup(invoke, readSetupDraft(setupForm));
      state = setupReviewSucceeded(state, review);
    } catch (error) {
      state = setupReviewFailed(state, error);
    }
    render();
  });

  setupCreateButton.addEventListener("click", async () => {
    const reviewId = state.setupReview?.reviewId;
    if (!reviewId) return;
    state = beginSetupCreation(state);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") {
        throw new Error("The native desktop bridge is unavailable.");
      }
      const outcome = await invokeCreateSessionFromReview(invoke, reviewId);
      if (outcome.status === "cancelled") {
        state = setupCreationCancelled(state);
      } else if (outcome.status === "created" && outcome.session) {
        state = setupCreationSucceeded(state, outcome.session);
      } else {
        throw new Error("The desktop command returned an unexpected response.");
      }
    } catch (error) {
      state = setupCreationFailed(state, error);
    }
    render();
    if (state.setupStatus === "created") {
      browserWindow.history.replaceState(null, "", "#run");
      await refreshReport();
      await refreshConductor();
    }
  });

  setupAddAntennaButton.addEventListener("click", () => {
    const fragment = setupAntennaTemplate.content.cloneNode(true);
    setupAddAntennaButton.before(fragment);
    refreshAntennaRows(setupForm);
    updateWsprRunPlanSummary(setupForm, setupRunPlanSummary);
    state = editSessionSetup(state);
    render();
  });

  setupForm.addEventListener("click", (event) => {
    const removeButton = event.target.closest("[data-remove-antenna]");
    if (!removeButton) return;
    const rows = setupForm.querySelectorAll("[data-antenna-row]");
    if (rows.length <= 1) return;
    removeButton.closest("[data-antenna-row]").remove();
    refreshAntennaRows(setupForm);
    updateWsprRunPlanSummary(setupForm, setupRunPlanSummary);
    state = editSessionSetup(state);
    render();
  });

  openButton.addEventListener("click", async () => {
    state = beginOpenSession(state);
    render();

    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") {
        throw new Error("The native desktop bridge is unavailable.");
      }

      const outcome = await invokeOpenSession(invoke);
      if (outcome.status === "cancelled") {
        state = openSessionCancelled(state);
      } else if (outcome.status === "opened" && outcome.session) {
        state = openSessionSucceeded(state, outcome.session);
        browserWindow.history.replaceState(null, "", "#report");
      } else {
        throw new Error("The desktop command returned an unexpected response.");
      }
    } catch (error) {
      state = openSessionFailed(state, error);
    }

    render();
    if (state.openStatus === "ready") await refreshReport();
  });

  exportButton.addEventListener("click", async () => {
    state = beginExportSession(state);
    render();

    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") {
        throw new Error("The native desktop bridge is unavailable.");
      }

      const outcome = await invokeExportSession(invoke);
      if (outcome.status === "cancelled") {
        state = exportSessionCancelled(state);
      } else if (outcome.status === "exported" && outcome.bundleName) {
        state = exportSessionSucceeded(state, outcome.bundleName);
      } else {
        throw new Error("The desktop command returned an unexpected response.");
      }
    } catch (error) {
      state = exportSessionFailed(state, error);
    }

    render();
  });

  importWsprLiveButton.addEventListener("click", async () => {
    if (state.session?.lifecycle !== "running" || state.importStatus === "loading") return;
    state = beginWsprLiveImport(state);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") throw new Error("The native desktop bridge is unavailable.");
      const outcome = await invokeImportActiveSessionWsprLive(invoke);
      if (outcome.status === "cancelled") {
        state = wsprLiveImportCancelled(state);
      } else if (outcome.status === "imported" && outcome.session) {
        state = wsprLiveImportSucceeded(state, outcome);
      } else {
        throw new Error("The desktop command returned an unexpected response.");
      }
    } catch (error) {
      state = wsprLiveImportFailed(state, error);
    }
    render();
    if (state.importStatus === "ready") await refreshReport();
  });
  importRbnButton.addEventListener("click", async () => {
    const eligible = state.session?.schemaVersion === 3
      && !["draft", "ready"].includes(state.session?.lifecycle);
    if (!eligible || state.importStatus === "loading") return;
    state = beginRbnImport(state);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") throw new Error("The native desktop bridge is unavailable.");
      const outcome = await invokeImportActiveSessionRbn(invoke);
      if (outcome.status === "cancelled") {
        state = rbnImportCancelled(state);
      } else if (outcome.status === "imported" && outcome.session) {
        state = rbnImportSucceeded(state, outcome);
      } else {
        throw new Error("The desktop command returned an unexpected response.");
      }
    } catch (error) {
      state = rbnImportFailed(state, error);
    }
    render();
    if (state.importStatus === "ready") await refreshReport();
  });

  reportRefreshButton.addEventListener("click", refreshReport);
  reportExportButton.addEventListener("click", async () => {
    if (!state.session?.reportHtml || state.reportExportStatus === "loading") return;
    state = beginReportExport(state);
    render();
    try {
      const invoke = browserWindow.__TAURI__?.core?.invoke;
      if (typeof invoke !== "function") throw new Error("The native desktop bridge is unavailable.");
      const outcome = await invokeExportActiveSessionReport(invoke);
      state = outcome.status === "cancelled"
        ? reportExportCancelled(state)
        : reportExportSucceeded(state, outcome);
    } catch (error) {
      state = reportExportFailed(state, error);
    }
    render();
  });

  syncSignalPlanFields(setupForm);
  updateWsprRunPlanSummary(setupForm, setupRunPlanSummary);
  render();
  const invoke = browserWindow.__TAURI__?.core?.invoke;
  if (typeof invoke === "function") {
    void invokeLoadStationPreferences(invoke)
      .then((preferences) => applyStationPreferences(setupForm, preferences))
      .catch(() => {});
  }
  browserWindow.setInterval?.(() => {
    if (
      state.activeWorkflow === "run" &&
      state.conductorStatus === "ready" &&
      state.conductor?.lifecycle === "running"
    ) {
      void refreshConductor();
    }
    if (state.activeWorkflow === "report" && state.session) void refreshReport();
  }, 5000);
  browserWindow.setInterval?.(() => {
    if (
      state.activeWorkflow !== "run"
      || state.conductorStatus !== "ready"
      || state.conductor?.lifecycle !== "running"
    ) return;
    const projectedSeconds = projectCountdown(countdownAnchor, monotonicNow());
    conductorCountdown.textContent = projectedSeconds === null
      ? ""
      : formatCountdown(projectedSeconds);
    if (
      projectedSeconds === 0
      && countdownAnchor?.seconds > 0
      && transitionRefreshKey !== countdownAnchor.key
    ) {
      transitionRefreshKey = countdownAnchor.key;
      void refreshConductor();
    }
  }, 1000);
}

function conductorStatusText(state) {
  if (state.conductorStatus === "loading") return "Recovering session";
  if (state.conductorStatus === "refreshing") return "Refreshing";
  if (state.conductorStatus === "mutating") return "Committing";
  if (state.conductorStatus === "error") return "Action failed";
  if (state.conductor) return humanizeIdentifier(state.conductor.lifecycle);
  return state.session ? "Ready to load" : "No active session";
}

function conductorFeedbackModel(state) {
  if (["loading", "refreshing"].includes(state.conductorStatus)) {
    return {
      kind: "loading",
      message: "Reading and recovering the coherent checkpoint…",
      detail: "A session left running is durably marked interrupted before actions are shown.",
    };
  }
  if (state.conductorStatus === "mutating") {
    return {
      kind: "loading",
      message: `${conductorActionLabel(state.conductorPendingAction)}…`,
      detail: "Saving this action to the session.",
    };
  }
  if (state.conductorError) return { kind: "error", ...state.conductorError };
  if (state.conductorNotice) {
    return {
      kind: "ready",
      message: state.conductorNotice,
      detail: "The active run now reflects the saved action.",
    };
  }
  const recovery = state.conductor?.recovery;
  if (recovery?.interruptionRecorded) {
    return {
      kind: "ready",
      message: "A previously running session was recovered as interrupted.",
      detail: `Recovery moved revision ${recovery.startingRevision} to ${recovery.finalRevision}; ${recovery.artifactCount} preserved recovery artifact(s).`,
    };
  }
  if (recovery && recovery.disposition !== "clean") {
    return {
      kind: "ready",
      message: `Recovery completed: ${humanizeIdentifier(recovery.disposition)}.`,
      detail: `${recovery.artifactCount} recovery artifact(s) were preserved.`,
    };
  }
  return null;
}

function conductorActionLabel(action) {
  switch (action) {
    case "start": return "Starting the session";
    case "resume": return "Resuming the session";
    case "arm_wspr_cycle": return "Scheduling the next WSPR cycle";
    case "skip_wspr_cycle": return "Skipping this cycle";
    case "interrupt": return "Pausing the session";
    case "end": return "Ending the session";
    case "abandon": return "Abandoning the session";
    case "operator_action":
    case null:
    case undefined: return "Saving the action";
    default: return "Saving the entry";
  }
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

function wsprLiveAcquisitionModel(state) {
  if (state.wsprLiveAcquisitionStatus === "fetching") {
    return {
      phase: "Fetching from WSPR.live…",
      detail: "The bounded native client is retrieving one cumulative response.",
      diagnostic: "",
      retry: false,
    };
  }
  if (state.wsprLiveAcquisitionError) {
    return {
      phase: "Automatic acquisition unavailable",
      detail: state.wsprLiveAcquisitionError.message,
      diagnostic: state.wsprLiveAcquisitionError.detail,
      retry: true,
      endWithout: state.conductor?.phase === "finalizing",
    };
  }
  const outcome = state.wsprLiveAcquisition;
  if (outcome?.status === "disabled") {
    return {
      phase: "Automatic public spots are off",
      detail: "This session remains fully local. Manual WSPR.live JSON import is still available as a recovery or offline path.",
      diagnostic: "",
      retry: false,
    };
  }
  if (!outcome || outcome.status === "dormant") {
    return {
      phase: "Waiting for antenna confirmation",
      detail: "The first confirmation establishes actual state. Later confirmations authorize the preceding segment; the final confirmed segment is captured after it ends.",
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "waiting") {
    return {
      phase: "Waiting for source ingestion",
      detail: `Segment ${outcome.completedSlotId} becomes eligible at ${formatReviewTime(outcome.notBefore)}. Later requests overlap earlier windows to recover delayed spots.`,
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "up_to_date") {
    return {
      phase: "Captured through authorized segments",
      detail: `WSPR.live evidence is committed through ${formatReviewTime(outcome.capturedThrough)}. Completeness remains unknown.`,
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "captured") {
    return {
      phase: "Public spots captured",
      detail: `${outcome.observationsCreated} observation(s) committed through ${formatReviewTime(outcome.capturedThrough)}; ${outcome.duplicate} duplicate(s) and ${outcome.conflict} conflict(s) retained explicitly.`,
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "completed") {
    return {
      phase: "Final public spots captured",
      detail: `WSPR.live evidence is committed through ${formatReviewTime(outcome.capturedThrough)} and the durable session ended automatically.`,
      diagnostic: "",
      retry: false,
    };
  }
  return {
    phase: "WSPR.live acquisition failed",
    detail: outcome.message,
    diagnostic: outcome.detail,
    retry: true,
    endWithout: state.conductor?.phase === "finalizing",
  };
}

function lifecycleActionAvailability(lifecycle) {
  switch (lifecycle) {
    case "ready": return new Set(["start", "abandon"]);
    case "running": return new Set(["interrupt", "end", "abandon"]);
    case "interrupted": return new Set(["resume", "end", "abandon"]);
    default: return new Set();
  }
}

function formatCountdown(seconds) {
  const safe = Math.max(0, Number(seconds));
  const minutes = Math.floor(safe / 60);
  const remainder = Math.floor(safe % 60);
  return `${String(minutes).padStart(2, "0")}:${String(remainder).padStart(2, "0")}`;
}

function renderSlot(container, slot, root, now) {
  container.replaceChildren();
  if (!slot) {
    const empty = root.createElement("p");
    empty.className = "muted-copy";
    empty.textContent = "None";
    container.append(empty);
    return;
  }
  const title = root.createElement("strong");
  title.textContent = `Cycle ${slot.sequenceNumber}`;
  const timing = root.createElement("div");
  timing.className = "slot-timing";
  for (const value of [
    slot.band,
    slot.plannedAntenna,
    formatActiveRunTime(slot.startsAt, { now }),
  ]) {
    const item = root.createElement("span");
    item.textContent = value;
    timing.append(item);
  }
  const actual = root.createElement("p");
  actual.className = "slot-evidence";
  actual.textContent = slot.actualAntenna
    ? `Actual: ${slot.actualAntenna}`
    : `Actual: not confirmed · ${humanizeIdentifier(slot.evidenceStatus)}`;
  container.append(title, timing, actual);
  if (slot.plannedSignal) {
    const plannedSignal = root.createElement("p");
    plannedSignal.textContent = `Planned signal: ${slot.plannedSignal.mode.toUpperCase()} · ${slot.plannedSignal.frequencyHz} Hz · ${slot.plannedSignal.transmittedCallsign}`;
    const actualSignal = root.createElement("p");
    actualSignal.textContent = slot.actualSignal
      ? `Actual signal: ${slot.actualSignal.mode?.toUpperCase() ?? "mode unconfirmed"} · ${slot.actualSignal.frequencyHz ?? "frequency unconfirmed"} Hz · ${slot.actualSignal.transmittedCallsign ?? "callsign unconfirmed"} · ${humanizeIdentifier(slot.signalStatus)}`
      : `Actual signal: not confirmed · ${humanizeIdentifier(slot.signalStatus)}`;
    container.append(plannedSignal, actualSignal);
  }
}

function renderIntent(container, intent, root) {
  container.replaceChildren();
  if (!intent) {
    const empty = root.createElement("p");
    empty.className = "muted-copy";
    empty.textContent = "None";
    container.append(empty);
    return;
  }
  const title = root.createElement("strong");
  title.textContent = `#${intent.sequenceNumber} · ${intent.antennaLabel}`;
  const band = root.createElement("p");
  band.textContent = intent.band;
  const timing = root.createElement("p");
  timing.textContent = "Timing will be set after you confirm the antenna is ready.";
  container.append(title, band, timing);
}

function replaceSelectOptions(select, options) {
  const signature = JSON.stringify(options);
  if (select.dataset.options === signature) return;
  const selected = select.value;
  select.replaceChildren(...options.map(({ value, label }) => {
    const option = select.ownerDocument.createElement("option");
    option.value = value;
    option.textContent = label;
    return option;
  }));
  select.dataset.options = signature;
  if (options.some(({ value }) => value === selected)) select.value = selected;
}

function conductorEventElement(root, event, disabled) {
  const article = root.createElement("article");
  const context = root.createElement("div");
  const kind = root.createElement("span");
  kind.textContent = event.slotId
    ? `${humanizeIdentifier(event.kind)} · ${event.slotId}`
    : humanizeIdentifier(event.kind);
  const summary = root.createElement("strong");
  summary.textContent = event.summary;
  const time = root.createElement("small");
  time.textContent = formatReviewTime(event.occurredAt);
  context.append(kind, summary, time);
  const actions = root.createElement("div");
  for (const [action, label] of [["replace", "Replace"], ["retract", "Retract"]]) {
    const button = root.createElement("button");
    button.type = "button";
    button.dataset.eventAction = action;
    button.dataset.eventId = event.sourceEventId;
    button.textContent = label;
    button.disabled = disabled;
    actions.append(button);
  }
  article.append(context, actions);
  return article;
}

function optionalNumber(value) {
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : Number(trimmed);
}

function readSignalEvidenceFields(frequency, mode, power, callsign, cadence) {
  return {
    frequencyHz: optionalNumber(frequency.value),
    mode: mode.value || null,
    powerWatts: optionalNumber(power.value),
    transmittedCallsign: callsign.value.toUpperCase(),
    cadenceFollowed: cadence.value === "" ? null : cadence.value === "true",
  };
}

export function readEvidenceAction(kind, slotId, antennaLabel, detail, signal = {}) {
  switch (kind) {
    case "confirm_antenna": return {
      kind,
      slotId,
      antennaLabel,
      note: detail,
    };
    case "confirm_signal": return {
      kind,
      slotId,
      frequencyHz: signal.frequencyHz ?? null,
      mode: signal.mode ?? null,
      powerWatts: signal.powerWatts ?? null,
      transmittedCallsign: (signal.transmittedCallsign ?? "").toUpperCase(),
      cadenceFollowed: signal.cadenceFollowed ?? null,
      note: detail,
    };
    case "mark_missed": return { kind, slotId, reason: detail };
    case "mark_bad": return { kind, slotId, reason: detail };
    case "add_note": return { kind, slotId: slotId || null, note: detail };
    default: throw new RangeError(`Unknown conductor evidence kind: ${kind}`);
  }
}

export function readEvidenceReplacement(kind, antennaLabel, detail, signal = {}) {
  switch (kind) {
    case "confirm_antenna": return { kind, antennaLabel, note: detail };
    case "confirm_signal": return {
      kind,
      frequencyHz: signal.frequencyHz ?? null,
      mode: signal.mode ?? null,
      powerWatts: signal.powerWatts ?? null,
      transmittedCallsign: (signal.transmittedCallsign ?? "").toUpperCase(),
      cadenceFollowed: signal.cadenceFollowed ?? null,
      note: detail,
    };
    case "mark_missed": return { kind, reason: detail };
    case "mark_bad": return { kind, reason: detail };
    case "add_note": return { kind, note: detail };
    default: throw new RangeError(`Unknown conductor evidence kind: ${kind}`);
  }
}

function transferStatusText(state) {
  if (state.importStatus === "loading") return "Importing evidence";
  if (state.importStatus === "error") return "Import failed";
  if (state.openStatus === "loading") return "Opening bundle";
  if (state.openStatus === "ready") return "Bundle open";
  if (state.openStatus === "error") return "Open failed";
  return "No bundle open";
}

function setupBusyState(state) {
  return ["reviewing", "creating"].includes(state.setupStatus);
}

function setupStatusText(state) {
  switch (state.setupStatus) {
    case "reviewing": return "Validating";
    case "reviewed": return "Ready to create";
    case "creating": return "Creating";
    case "created": return "Session ready";
    case "invalid": return "Needs changes";
    case "error": return "Setup failed";
    default: return "Draft";
  }
}

function setupFeedbackModel(state) {
  if (state.setupStatus === "reviewing") {
    return {
      kind: "loading",
      message: "Normalizing and validating the plan…",
      detail: "No destination is created during review.",
    };
  }
  if (state.setupStatus === "creating") {
    return {
      kind: "loading",
      message: "Creating and reopening the checkpointed session…",
      detail: "The destination is published only after complete verification.",
    };
  }
  if (state.setupError) return { kind: "error", ...state.setupError };
  if (state.setupStatus === "invalid") {
    return {
      kind: "error",
      message: "The plan needs changes before it can be created.",
      detail: "Use the field diagnostics below, then review again.",
    };
  }
  if (state.setupNotice === "cancelled") {
    return {
      kind: "cancelled",
      message: "Creation cancelled.",
      detail: "The reviewed plan remains ready and no destination was changed.",
    };
  }
  if (state.setupNotice === "created" && state.session) {
    return {
      kind: "ready",
      message: `${state.session.bundleName} is the active session.`,
      detail: `Checkpoint revision 0 is ready with ${state.session.slotCount} planned slots.`,
    };
  }
  if (state.setupStatus === "reviewed") {
    return {
      kind: "ready",
      message: "The normalized plan passed strict creation validation.",
      detail: "Review the exact UTC-backed schedule, then create the session.",
    };
  }
  return null;
}

function humanizeIdentifier(value) {
  return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function formatReviewTime(value) {
  const instant = new Date(value);
  return `${new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(instant)} · ${instant.toISOString()}`;
}

function optionalField(row, field) {
  return row.querySelector(`[data-antenna-field="${field}"]`)?.value ?? "";
}

export function readSetupDraft(form) {
  const value = (field) => form.querySelector(`[data-setup-field="${field}"]`).value;
  const signalPlanEnabled = form.querySelector('[data-setup-field="signalPlanEnabled"]').checked;
  return {
    station: {
      callsign: value("callsign").toUpperCase(),
      grid: value("grid"),
      powerWatts: value("powerWatts"),
      operatorNotes: value("operatorNotes"),
    },
    antennas: [...form.querySelectorAll("[data-antenna-row]")].map((row) => ({
      label: optionalField(row, "label"),
      facets: optionalField(row, "facets"),
      heightM: optionalField(row, "heightM"),
      radialCount: optionalField(row, "radialCount"),
      radialLengthM: optionalField(row, "radialLengthM"),
      orientationDegrees: optionalField(row, "orientationDegrees"),
      tuner: optionalField(row, "tuner"),
      feedline: optionalField(row, "feedline"),
      notes: optionalField(row, "notes"),
    })),
    schedule: {
      mode: value("mode"),
      goal: value("goal"),
      band: value("band"),
      rounds: value("rounds"),
    },
    wsprLiveAcquisitionEnabled: form.querySelector('[data-setup-field="wsprLiveAcquisitionEnabled"]').checked,
    signalPlan: signalPlanEnabled ? {
      mode: value("signalMode"),
      collectionProfile: value("signalCollectionProfile"),
      plannedPowerWatts: value("signalPlannedPowerWatts"),
      transmittedCallsign: value("signalTransmittedCallsign").toUpperCase(),
      differingIdentityValidated: form.querySelector('[data-setup-field="signalDifferingIdentityValidated"]').checked,
      message: value("signalMessage"),
      repetitionCount: value("signalRepetitionCount"),
      keySpeedWpm: value("signalKeySpeedWpm"),
      transmitSeconds: value("signalTransmitSeconds"),
      intervalSeconds: value("signalIntervalSeconds"),
      frequenciesHz: value("signalFrequenciesHz"),
    } : null,
  };
}

export function applyStationPreferences(form, preferences) {
  if (!preferences) return false;
  const fields = {
    callsign: preferences.callsign ?? "",
    grid: preferences.grid ?? "",
    powerWatts: preferences.powerWatts ?? "",
    operatorNotes: preferences.operatorNotes ?? "",
  };
  const controls = Object.keys(fields).map((field) =>
    form.querySelector(`[data-setup-field="${field}"]`)
  );
  if (controls.some((control) => control.value.trim().length > 0)) return false;
  controls.forEach((control, index) => {
    const field = Object.keys(fields)[index];
    control.value = field === "callsign" ? fields[field].toUpperCase() : fields[field];
  });
  return true;
}

function syncSignalPlanFields(form) {
  const enabled = form.querySelector('[data-setup-field="signalPlanEnabled"]').checked;
  const fields = form.querySelector("[data-signal-plan-fields]");
  fields.hidden = !enabled;
  for (const control of fields.querySelectorAll("input, select, textarea")) {
    control.disabled = !enabled;
  }
  const wsprLive = form.querySelector('[data-setup-field="wsprLiveAcquisitionEnabled"]');
  if (enabled) wsprLive.checked = false;
  wsprLive.disabled = enabled;
}

function refreshAntennaRows(form) {
  const rows = [...form.querySelectorAll("[data-antenna-row]")];
  rows.forEach((row, index) => {
    row.querySelector("[data-antenna-title]").textContent = `Antenna ${String.fromCharCode(65 + index)}`;
    row.querySelector("[data-remove-antenna]").disabled = rows.length <= 1;
  });
}

function updateWsprRunPlanSummary(form, output) {
  const summary = wsprRunPlanSummary(
    form.querySelector('[data-setup-field="rounds"]').value,
    form.querySelectorAll("[data-antenna-row]").length,
  );
  output.textContent = summary?.text ?? "Enter complete rounds to estimate the run length.";
}

function openFeedbackModel(state) {
  if (state.openStatus === "loading") {
    return {
      kind: "loading",
      message: "Reading and validating the selected bundle…",
      detail: "The source directory will not be changed.",
    };
  }
  if (state.error) return { kind: "error", ...state.error };
  if (state.notice === "cancelled") {
    return { kind: "cancelled", message: "Open cancelled.", detail: "No session was changed." };
  }
  if (state.session) {
    return {
      kind: "ready",
      message: `${state.session.bundleName} is ready.`,
      detail: "Its local report was rebuilt in memory from the source bundle.",
    };
  }
  return null;
}

function exportFeedbackModel(state) {
  if (state.exportStatus === "loading") {
    return {
      kind: "loading",
      message: "Copying and verifying the active bundle…",
      detail: "Original durable files and attachments are preserved byte-for-byte.",
    };
  }
  if (state.exportError) return { kind: "error", ...state.exportError };
  if (state.exportNotice === "cancelled") {
    return {
      kind: "cancelled",
      message: "Export cancelled.",
      detail: "The active session was not changed.",
    };
  }
  if (state.exportedBundleName) {
    return {
      kind: "ready",
      message: `${state.exportedBundleName} was exported and verified.`,
      detail: "The original bundle remains the active session.",
    };
  }
  return null;
}

function importFeedbackModel(state) {
  if (state.importStatus === "loading") {
    const rbn = state.importKind === "rbn";
    return {
      kind: "loading",
      message: rbn
        ? "Validating and committing RBN archive evidence…"
        : "Validating and committing WSPR.live evidence…",
      detail: rbn
        ? "The exact ZIP, bounded row dispositions, and public reports commit under one checkpoint."
        : "The exact response and its bounded row dispositions commit under one checkpoint.",
    };
  }
  if (state.importError) return { kind: "error", ...state.importError };
  if (state.importNotice === "cancelled") {
    const source = state.importKind === "rbn" ? "RBN archive" : "WSPR.live";
    return {
      kind: "cancelled",
      message: `${source} import cancelled.`,
      detail: "The active session was not changed.",
    };
  }
  if (state.importNotice) {
    const result = state.importNotice;
    const omitted = result.omitted ? `, ${result.omitted} omitted by the retention bound` : "";
    return {
      kind: "ready",
      message: `${result.observationsCreated} imported spot observation(s) committed at revision ${result.revision}.`,
      detail: `${result.total} rows: ${result.accepted} accepted, ${result.filtered} filtered, ${result.malformed} malformed, ${result.unsupported} unsupported, ${result.duplicate} duplicate, ${result.conflict} conflict${omitted}. Source completeness is unknown.`,
    };
  }
  return null;
}

function reportFeedbackModel(state) {
  if (state.reportStatus === "refreshing") {
    return {
      kind: "loading",
      message: "Building one verified committed snapshot…",
      detail: "The prior coherent report remains visible until the new revision is verified.",
    };
  }
  if (state.reportExportStatus === "loading") {
    return {
      kind: "loading",
      message: "Exporting the visible standalone HTML snapshot…",
      detail: "The destination is created without overwriting an existing file.",
    };
  }
  if (state.reportExportError) return { kind: "error", ...state.reportExportError };
  if (state.reportError) return { kind: "error", ...state.reportError };
  if (state.reportExportNotice === "cancelled") {
    return {
      kind: "cancelled",
      message: "Report export cancelled.",
      detail: "The visible coherent report was retained.",
    };
  }
  if (state.reportExportNotice) {
    return {
      kind: "ready",
      message: "The standalone report snapshot was exported.",
      detail: state.reportExportNotice,
    };
  }
  return null;
}

if (typeof document !== "undefined") {
  mount(document, window);
}
