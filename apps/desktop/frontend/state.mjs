import { conductorActionAvailable, WORKFLOWS, wsjtxReadinessModel } from "./models.mjs";

function hasCoherentReport(session) {
  return session?.presentationId !== undefined
    && session?.presentationId !== null
    && typeof session?.reportHtml === "string"
    && typeof session?.summaryHtml === "string";
}

function reportModeForSession(state, session) {
  return typeof session?.sessionId === "string"
    && state.session?.sessionId === session.sessionId
    ? state.reportMode
    : "summary";
}

export function initialState(workflow = "saved") {
  return selectWorkflow(
    {
      activeWorkflow: "setup",
      openStatus: "idle",
      openSource: null,
      openIntent: null,
      catalogStatus: "idle",
      managedCatalog: null,
      catalogError: null,
      catalogRowOperation: null,
      catalogRowError: null,
      catalogRowNotice: null,
      catalogImportStatus: "idle",
      catalogImportError: null,
      catalogImportNotice: null,
      catalogDeleteStatus: "idle",
      catalogDeleteTarget: null,
      catalogDeleteError: null,
      catalogDeleteNotice: null,
      managedLocationNotice: null,
      activeManagedLocatorId: null,
      session: null,
      reportPresentationId: null,
      reportMode: "summary",
      pendingReportPresentation: null,
      reportWindowStatus: "idle",
      reportWindowError: null,
      reportWindowNotice: null,
      reportStatus: "idle",
      reportError: null,
      reportExportStatus: "idle",
      reportExportPending: null,
      reportExportError: null,
      reportExportNotice: null,
      supportCopyStatus: "idle",
      supportCopyError: null,
      error: null,
      notice: null,
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
      conductorPendingAction: null,
      conductorNotice: null,
      skipCycleDialog: null,
      skipCycleStatus: "idle",
      skipCycleError: null,
      skipCycleNotice: null,
      wsjtxStatus: "idle",
      wsjtx: null,
      wsjtxError: null,
      wsjtxReadinessAcknowledgement: null,
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
      antennaControllerProfileRefreshError: null,
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

  return {
    ...state,
    activeWorkflow: workflow,
    ...(workflow === "run" ? {} : {
      skipCycleDialog: null,
      skipCycleStatus: "idle",
      skipCycleError: null,
      skipCycleNotice: null,
    }),
  };
}

export function beginOpenSession(state, source = "external", intent = null, locatorId = null) {
  return {
    ...state,
    openStatus: "loading",
    openSource: source,
    openIntent: intent,
    catalogRowOperation: locatorId ? { locatorId, kind: "opening" } : null,
    catalogRowError: null,
    reportExportStatus: "idle",
    reportExportPending: null,
    reportExportError: null,
    reportExportNotice: null,
    error: null,
    notice: null,
  };
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

export function setupCreationSucceeded(state, session, managedLocation = null) {
  return {
    ...state,
    activeWorkflow: "run",
    setupStatus: "created",
    setupError: null,
    setupNotice: "created",
    openStatus: "ready",
    session,
    reportMode: reportModeForSession(state, session),
    pendingReportPresentation: null,
    reportWindowStatus: "idle",
    reportWindowError: null,
    reportWindowNotice: null,
    reportPresentationId: session.presentationId ?? null,
    reportStatus: hasCoherentReport(session) ? "ready" : "unavailable",
    reportError: null,
    reportExportStatus: "idle",
    reportExportPending: null,
    reportExportError: null,
    reportExportNotice: null,
    supportCopyStatus: "idle",
    supportCopyError: null,
    error: null,
    notice: null,
    managedLocationNotice: managedLocation,
    activeManagedLocatorId: managedLocation?.locatorId ?? null,
    importStatus: "idle",
    importKind: null,
    importError: null,
    importNotice: null,
    conductorStatus: "idle",
    conductor: null,
    conductorError: null,
    conductorPendingAction: null,
    conductorNotice: null,
    skipCycleDialog: null,
    skipCycleStatus: "idle",
    skipCycleError: null,
    skipCycleNotice: null,
    wsjtxStatus: "idle",
    wsjtx: null,
    wsjtxError: null,
    wsjtxReadinessAcknowledgement: null,
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

export function openSessionSucceeded(
  state,
  session,
  workflow = "report",
  notice = null,
  intent = state.openIntent,
) {
  return {
    ...state,
    activeWorkflow: workflow,
    openStatus: "ready",
    openIntent: intent,
    session,
    reportMode: reportModeForSession(state, session),
    pendingReportPresentation: null,
    reportWindowStatus: "idle",
    reportWindowError: null,
    reportWindowNotice: null,
    reportPresentationId: session.presentationId ?? null,
    reportStatus: hasCoherentReport(session) ? "ready" : "unavailable",
    reportError: null,
    reportExportStatus: "idle",
    reportExportPending: null,
    reportExportError: null,
    reportExportNotice: null,
    supportCopyStatus: "idle",
    supportCopyError: null,
    error: null,
    notice,
    activeManagedLocatorId: state.openSource === "managed"
      ? state.catalogRowOperation?.locatorId ?? null
      : null,
    catalogRowOperation: null,
    catalogRowError: null,
    importStatus: "idle",
    importKind: null,
    importError: null,
    importNotice: null,
    conductorStatus: "idle",
    conductor: null,
    conductorError: null,
    conductorPendingAction: null,
    conductorNotice: null,
    skipCycleDialog: null,
    skipCycleStatus: "idle",
    skipCycleError: null,
    skipCycleNotice: null,
    wsjtxStatus: "idle",
    wsjtx: null,
    wsjtxError: null,
    wsjtxReadinessAcknowledgement: null,
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
    catalogRowOperation: null,
  };
}

export function openSessionFailed(state, error) {
  const normalized = normalizeOpenError(error);
  return {
    ...state,
    openStatus: "error",
    error: normalized,
    notice: null,
    catalogRowError: state.catalogRowOperation
      ? { locatorId: state.catalogRowOperation.locatorId, error: normalized }
      : null,
    catalogRowOperation: null,
  };
}

export function beginManagedCatalogLoad(state) {
  return {
    ...state,
    catalogStatus: state.managedCatalog ? "refreshing" : "loading",
    catalogError: null,
  };
}

export function managedCatalogLoadSucceeded(state, catalog) {
  const imported = state.catalogImportNotice;
  const refreshedImported = imported
    ? catalog.entries?.find((entry) => entry.bundleName === imported.bundleName)
    : null;
  return {
    ...state,
    catalogStatus: "ready",
    managedCatalog: catalog,
    catalogError: null,
    catalogImportNotice: imported
      ? { ...imported, locatorId: refreshedImported?.locatorId ?? null }
      : null,
  };
}

export function managedCatalogLoadFailed(state, error) {
  return {
    ...state,
    catalogStatus: "error",
    catalogError: normalizeOpenError(error),
  };
}

export function beginManagedReveal(state, locatorId = null) {
  return {
    ...state,
    catalogRowOperation: { locatorId, kind: locatorId ? "revealing" : "revealing_folder" },
    catalogRowError: null,
    catalogRowNotice: null,
  };
}

export function managedRevealSucceeded(state) {
  return { ...state, catalogRowOperation: null, catalogRowError: null };
}

export function managedRevealFailed(state, error) {
  const normalized = normalizeOpenError(error);
  return {
    ...state,
    catalogRowError: state.catalogRowOperation?.locatorId
      ? { locatorId: state.catalogRowOperation.locatorId, error: normalized }
      : null,
    catalogError: state.catalogRowOperation?.locatorId ? state.catalogError : normalized,
    catalogRowOperation: null,
  };
}

export function requestManagedDelete(state, entry) {
  if (!entry?.locatorId || state.catalogRowOperation) return state;
  return {
    ...state,
    catalogDeleteStatus: "confirming",
    catalogDeleteTarget: {
      locatorId: entry.locatorId,
      callsign: entry.callsign ?? null,
      bundleName: entry.bundleName,
    },
    catalogDeleteError: null,
    catalogDeleteNotice: null,
  };
}

export function cancelManagedDelete(state) {
  if (state.catalogDeleteStatus === "deleting") return state;
  return {
    ...state,
    catalogDeleteStatus: "cancelled",
    catalogDeleteTarget: null,
    catalogDeleteError: null,
    catalogDeleteNotice: null,
  };
}

export function beginManagedDelete(state) {
  const locatorId = state.catalogDeleteTarget?.locatorId;
  if (!locatorId || state.catalogDeleteStatus === "deleting") return state;
  return {
    ...state,
    catalogDeleteStatus: "deleting",
    catalogDeleteError: null,
  };
}

export function managedDeleteSucceeded(state, outcome) {
  const locatorId = state.catalogDeleteTarget?.locatorId;
  return {
    ...state,
    catalogDeleteStatus: "succeeded",
    catalogDeleteTarget: null,
    catalogDeleteError: null,
    catalogDeleteNotice: outcome.bundleName,
    managedCatalog: state.managedCatalog ? {
      ...state.managedCatalog,
      entries: state.managedCatalog.entries.filter((entry) => entry.locatorId !== locatorId),
    } : null,
  };
}

export function managedDeleteFailed(state, error) {
  const normalized = normalizeOpenError(error);
  return {
    ...state,
    catalogDeleteStatus: "failed",
    catalogDeleteError: normalized,
  };
}

export function beginManagedImport(state) {
  return {
    ...state,
    catalogImportStatus: "loading",
    catalogImportError: null,
    catalogImportNotice: null,
  };
}

export function managedImportSucceeded(state, location) {
  return {
    ...state,
    catalogImportStatus: "ready",
    catalogImportError: null,
    catalogImportNotice: location,
  };
}

export function managedImportCancelled(state) {
  return {
    ...state,
    catalogImportStatus: "idle",
    catalogImportError: null,
    catalogImportNotice: null,
  };
}

export function managedImportFailed(state, error) {
  return {
    ...state,
    catalogImportStatus: "error",
    catalogImportError: normalizeOpenError(error),
    catalogImportNotice: null,
  };
}

export function beginManagedExport(state, locatorId) {
  return {
    ...state,
    catalogRowOperation: { locatorId, kind: "exporting" },
    catalogRowError: null,
    catalogRowNotice: null,
  };
}

export function managedExportSucceeded(state, bundleName) {
  return {
    ...state,
    catalogRowOperation: null,
    catalogRowError: null,
    catalogRowNotice: {
      locatorId: state.catalogRowOperation?.locatorId ?? null,
      message: `Exported ${bundleName}.`,
    },
  };
}

export function managedExportCancelled(state) {
  return {
    ...state,
    catalogRowOperation: null,
    catalogRowError: null,
    catalogRowNotice: {
      locatorId: state.catalogRowOperation?.locatorId ?? null,
      message: "Bundle export cancelled.",
    },
  };
}

export function managedExportFailed(state, error) {
  const normalized = normalizeOpenError(error);
  return {
    ...state,
    catalogRowError: {
      locatorId: state.catalogRowOperation?.locatorId ?? null,
      error: normalized,
    },
    catalogRowOperation: null,
    catalogRowNotice: null,
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
      summaryHtml: null,
    },
    pendingReportPresentation: null,
    reportWindowStatus: "idle",
    reportWindowError: null,
    reportWindowNotice: null,
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
  return reconcileSkipCycleDialog({
    ...state,
    conductorStatus: "ready",
    conductor,
    session: reconcileSessionWithConductor(state.session, conductor),
    conductorError: null,
    conductorPendingAction: null,
    conductorNotice: completedAction
      ? conductorActionCompletedLabel(completedAction)
      : state.conductorNotice,
  }, conductor);
}

export function conductorPollSucceeded(state, conductor) {
  return reconcileSkipCycleDialog({
    ...state,
    conductor,
    session: reconcileSessionWithConductor(state.session, conductor),
  }, conductor);
}

export function requestSkipCycle(state) {
  const conductor = state.conductor;
  const intent = conductor?.nextIntent;
  if (
    !conductor
    || !intent
    || state.skipCycleStatus === "submitting"
    || !conductorActionAvailable(conductor, "skip_wspr_cycle")
  ) return state;
  return {
    ...state,
    skipCycleDialog: {
      actionToken: conductor.actionToken,
      expectedRevision: conductor.revision,
      intentId: intent.intentId,
      sequenceNumber: intent.sequenceNumber,
      antennaLabel: intent.antennaLabel ?? null,
      direction: intent.direction ?? null,
      band: intent.band ?? null,
    },
    skipCycleStatus: "confirming",
    skipCycleError: null,
    skipCycleNotice: null,
  };
}

export function cancelSkipCycle(state) {
  if (state.skipCycleStatus === "submitting") return state;
  return {
    ...state,
    skipCycleDialog: null,
    skipCycleStatus: "idle",
    skipCycleError: null,
    skipCycleNotice: null,
  };
}

export function beginSkipCycleMutation(state) {
  if (!state.skipCycleDialog || state.skipCycleStatus === "submitting") return state;
  return {
    ...beginConductorMutation(state, "skip_wspr_cycle"),
    skipCycleStatus: "submitting",
    skipCycleError: null,
    skipCycleNotice: null,
  };
}

export function skipCycleMutationSucceeded(state, conductor) {
  return {
    ...conductorLoadSucceeded({ ...state, skipCycleDialog: null }, conductor),
    skipCycleDialog: null,
    skipCycleStatus: "succeeded",
    skipCycleError: null,
    skipCycleNotice: "Cycle skipped.",
  };
}

export function skipCycleMutationFailed(state, error) {
  return {
    ...conductorMutationFailed(state, error),
    skipCycleDialog: null,
    skipCycleStatus: "error",
    skipCycleError: normalizeOpenError(error),
    skipCycleNotice: null,
  };
}

function reconcileSkipCycleDialog(state, conductor) {
  const presented = state.skipCycleDialog;
  if (!presented || state.skipCycleStatus === "submitting") return state;
  const current = conductor?.nextIntent;
  const unchanged = conductor?.lifecycle === "running"
    && conductor.revision === presented.expectedRevision
    && conductor.actionToken === presented.actionToken
    && current?.intentId === presented.intentId;
  if (unchanged) return state;
  return {
    ...state,
    skipCycleDialog: null,
    skipCycleStatus: "error",
    skipCycleError: {
      kind: "stale_revision",
      message: "The cycle changed before it was skipped.",
      detail: "Review the current run state, then choose Skip this cycle again if it is still appropriate.",
    },
    skipCycleNotice: null,
  };
}

function reconcileSessionWithConductor(session, conductor) {
  if (!session) return session;
  return {
    ...session,
    lifecycle: conductor.lifecycle,
    revision: conductor.revision,
  };
}

export function setWsjtxReadinessAcknowledged(state, acknowledged) {
  const readiness = wsjtxReadinessModel(state);
  return {
    ...state,
    wsjtxReadinessAcknowledgement: acknowledged && readiness.visible ? readiness.key : null,
  };
}

export function beginConductorMutation(state, action = "operator_action") {
  return {
    ...state,
    conductorStatus: "mutating",
    conductorError: null,
    conductorPendingAction: action,
    conductorNotice: null,
    ...(action === "skip_wspr_cycle" ? {} : {
      skipCycleError: null,
      skipCycleNotice: null,
    }),
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
    antennaControllerProfileRefreshError: ["saving", "deleting"].includes(status)
      ? null
      : state.antennaControllerProfileRefreshError,
  };
}

function normalizedControllerProfileName(name) {
  return name.trim().toLowerCase();
}

function catalogWithSavedControllerProfile(catalog, savedProfile) {
  const normalizedName = normalizedControllerProfileName(savedProfile.name);
  const profiles = (catalog?.profiles ?? []).filter((profile) => (
    profile.profileId !== savedProfile.profileId
    && normalizedControllerProfileName(profile.name) !== normalizedName
  ));
  profiles.push(savedProfile);
  profiles.sort((left, right) => {
    const leftName = normalizedControllerProfileName(left.name);
    const rightName = normalizedControllerProfileName(right.name);
    if (leftName < rightName) return -1;
    if (leftName > rightName) return 1;
    return left.profileId < right.profileId ? -1 : left.profileId > right.profileId ? 1 : 0;
  });
  return {
    inputStyle: catalog?.inputStyle ?? "one_line",
    profiles,
    ...(catalog?.migrationNotice ? { migrationNotice: catalog.migrationNotice } : {}),
  };
}

export function antennaControllerProfileSaveCommitted(state, savedProfile) {
  return {
    ...state,
    antennaControllerStatus: "reconciling",
    antennaControllerCatalog: catalogWithSavedControllerProfile(
      state.antennaControllerCatalog,
      savedProfile,
    ),
    antennaControllerError: null,
    antennaControllerProfileNotice: { kind: "saved", profileId: savedProfile.profileId },
    antennaControllerProfileError: null,
    antennaControllerProfileRefreshError: null,
  };
}

export function antennaControllerProfileDeleteCommitted(state, profileId) {
  return {
    ...state,
    antennaControllerStatus: "reconciling",
    antennaControllerCatalog: state.antennaControllerCatalog
      ? {
          ...state.antennaControllerCatalog,
          profiles: state.antennaControllerCatalog.profiles.filter(
            (profile) => profile.profileId !== profileId,
          ),
        }
      : null,
    antennaControllerError: null,
    antennaControllerProfileNotice: { kind: "deleted", profileId: "" },
    antennaControllerProfileError: null,
    antennaControllerProfileRefreshError: null,
  };
}

export function antennaControllerProfileActionFailed(state, error) {
  const normalized = normalizeOpenError(error);
  return {
    ...state,
    antennaControllerStatus: "error",
    antennaControllerError: normalized,
    antennaControllerProfileError: normalized,
    antennaControllerProfileRefreshError: null,
  };
}

export function antennaControllerProfileReconciliationFailed(state, error) {
  const normalized = normalizeOpenError(error);
  return {
    ...state,
    antennaControllerStatus: "ready",
    antennaControllerError: null,
    antennaControllerProfileError: null,
    antennaControllerProfileRefreshError: {
      kind: "profile_refresh_failed_after_commit",
      message: "The profile change is saved, but the profile list could not be refreshed.",
      detail: normalized.detail ?? normalized.message,
    },
  };
}

export function antennaControllerCatalogSucceeded(state, catalog) {
  return {
    ...state,
    antennaControllerStatus: "ready",
    antennaControllerCatalog: catalog,
    antennaControllerError: null,
    antennaControllerProfileRefreshError: null,
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
  if (!hasCoherentReport(presentation)) {
    throw new TypeError("A report presentation must contain both immutable document variants.");
  }
  if (
    presentation.sessionId !== undefined
    && state.session?.sessionId !== undefined
    && presentation.sessionId !== state.session.sessionId
  ) {
    throw new RangeError("A report presentation cannot cross active-session identity.");
  }
  const presentationChanged = String(presentation.presentationId)
    !== String(state.reportPresentationId);
  return {
    ...state,
    reportStatus: "ready",
    reportError: null,
    pendingReportPresentation: null,
    reportWindowStatus: "idle",
    reportWindowError: null,
    reportWindowNotice: null,
    reportPresentationId: presentation.presentationId,
    reportExportStatus: presentationChanged ? "idle" : state.reportExportStatus,
    reportExportPending: presentationChanged ? null : state.reportExportPending,
    reportExportError: presentationChanged ? null : state.reportExportError,
    reportExportNotice: presentationChanged ? null : state.reportExportNotice,
    session: state.session ? {
      ...state.session,
      reportHtml: presentation.reportHtml,
      summaryHtml: presentation.summaryHtml,
      revision: presentation.revision,
      lifecycle: presentation.lifecycle,
      completeness: presentation.completeness,
      hasControllerEvidence: presentation.hasControllerEvidence,
      operationalHistory: presentation.operationalHistory ?? state.session.operationalHistory,
      presentationId: presentation.presentationId,
      reportAvailable: true,
    } : state.session,
  };
}

export function reportRefreshPending(state, presentation) {
  if (!hasCoherentReport(presentation)) {
    throw new TypeError("A pending report presentation must contain both immutable document variants.");
  }
  if (
    presentation.sessionId !== undefined
    && state.session?.sessionId !== undefined
    && presentation.sessionId !== state.session.sessionId
  ) {
    throw new RangeError("A pending report presentation cannot cross active-session identity.");
  }
  if (String(presentation.presentationId) === String(state.reportPresentationId)) return state;
  const existing = state.pendingReportPresentation;
  if (
    existing
    && Number(presentation.presentationId) <= Number(existing.presentationId)
  ) return state;
  return {
    ...state,
    pendingReportPresentation: presentation,
    reportError: null,
  };
}

export function applyPendingReportPresentation(state) {
  if (!state.pendingReportPresentation) return state;
  return reportRefreshSucceeded(state, state.pendingReportPresentation);
}

export function beginReportWindowOpen(state) {
  return {
    ...state,
    reportWindowStatus: "loading",
    reportWindowError: null,
    reportWindowNotice: null,
  };
}

export function reportWindowOpenSucceeded(state, outcome) {
  return {
    ...state,
    reportWindowStatus: "ready",
    reportWindowError: null,
    reportWindowNotice: outcome,
  };
}

export function reportWindowOpenFailed(state, error) {
  return {
    ...state,
    reportWindowStatus: "error",
    reportWindowError: normalizeOpenError(error),
    reportWindowNotice: null,
  };
}

export function selectReportMode(state, reportMode) {
  if (!["summary", "full_evidence"].includes(reportMode)) {
    throw new RangeError(`Unknown report mode: ${reportMode}`);
  }
  if (state.reportMode === reportMode) return state;
  return {
    ...state,
    reportMode,
    reportWindowStatus: "idle",
    reportWindowError: null,
    reportWindowNotice: null,
  };
}

export function beginSupportSummaryCopy(state) {
  return { ...state, supportCopyStatus: "copying", supportCopyError: null };
}

export function supportSummaryCopySucceeded(state) {
  return { ...state, supportCopyStatus: "copied", supportCopyError: null };
}

export function supportSummaryCopyFailed(state, error) {
  return {
    ...state,
    supportCopyStatus: "error",
    supportCopyError: normalizeOpenError(error),
  };
}

export function reportRefreshFailed(state, error) {
  return {
    ...state,
    reportStatus: hasCoherentReport(state.session) ? "ready" : "unavailable",
    reportError: normalizeOpenError(error),
  };
}

export function reportRefreshSuperseded(state) {
  return {
    ...state,
    reportStatus: hasCoherentReport(state.session) ? "ready" : "unavailable",
    reportError: null,
  };
}

export function beginReportExport(state) {
  return {
    ...state,
    reportExportStatus: "loading",
    reportExportPending: null,
    reportExportError: null,
    reportExportNotice: null,
  };
}

export function reportExportConfirmationRequired(state, outcome) {
  return {
    ...state,
    reportExportStatus: "confirming",
    reportExportPending: {
      pendingExportId: outcome.pendingExportId,
      fileName: outcome.fileName,
      format: outcome.format,
      revision: outcome.revision,
    },
    reportExportError: null,
    reportExportNotice: null,
  };
}

export function beginReportReplacement(state) {
  if (!state.reportExportPending || state.reportExportStatus === "replacing") return state;
  return { ...state, reportExportStatus: "replacing", reportExportError: null };
}

export function beginReportExportCancellation(state) {
  if (!state.reportExportPending || state.reportExportStatus === "replacing") return state;
  return { ...state, reportExportStatus: "cancelling", reportExportError: null };
}

export function reportExportSucceeded(state, outcome) {
  const label = outcome.format === "summary_html"
    ? "Summary"
    : "full evidence report";
  return {
    ...state,
    reportExportStatus: "ready",
    reportExportPending: null,
    reportExportError: null,
    reportExportNotice: `${label}: ${outcome.fileName} · revision ${outcome.revision ?? "legacy"}`,
  };
}

export function reportExportCancelled(state) {
  return {
    ...state,
    reportExportStatus: "idle",
    reportExportPending: null,
    reportExportError: null,
    reportExportNotice: "cancelled",
  };
}

export function reportExportFailed(state, error) {
  return {
    ...state,
    reportExportStatus: "error",
    reportExportPending: null,
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
