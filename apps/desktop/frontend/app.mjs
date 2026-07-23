import { createDesktopController } from "./controller.mjs";
import { collectDesktopElements } from "./elements.mjs";
import {
  applyStationPreferences,
  normalizeMaidenheadGrid,
  readControllerProfileDraft,
  readEvidenceAction,
  readEvidenceReplacement,
  readSetupDraft,
  readSignalEvidenceFields,
  selectSetupQuestion,
  syncSetupQuestionToMode,
  syncWsprLiveForSignalPlan,
} from "./forms.mjs";
import {
  createReportDocumentUrls,
  createWorkflowScrollMemory,
  focusSetupOutcome,
  installContextualHelp,
  locationLookupMessage,
  maidenheadGrid,
  recommendedNoteTarget,
  releaseReportFrame,
  startupWorkflowFromHash,
  wsjtxReadinessModel,
  workflowFromHash,
} from "./models.mjs";
import { initialState } from "./state.mjs";
import {
  formatCountdown,
  renderNavigation,
  renderEvidenceImports,
  renderReport,
  renderRun,
  renderSavedSessions,
  renderSetup,
} from "./renderers.mjs";

export function mount(root, browserWindow) {
  const rootDocument = root.ownerDocument ?? root;
  let state = initialState(startupWorkflowFromHash(browserWindow.location.hash));
  let countdownAnchor = null;
  let countdownAnchorKey = null;
  let noteShortcutInitialized = false;
  let deleteTrigger = null;
  let reportExportTrigger = null;
  let skipCycleTrigger = null;
  let hydratedControllerProfile = null;
  let controllerProfileReconciliationKey = null;
  let controllerProfileReconciliationAttempts = 0;
  let controllerProfileReconciliationTimer = null;
  const workflowScrollMemory = createWorkflowScrollMemory(state.activeWorkflow);
  const monotonicNow = () => browserWindow.performance?.now?.() ?? Date.now();
  const preferredScrollBehavior = browserWindow.matchMedia?.("(prefers-reduced-motion: reduce)").matches
    ? "auto"
    : "smooth";
  const elements = collectDesktopElements(root);
  const reportDocuments = createReportDocumentUrls(browserWindow);
  const {
    mainContent,
    savedNew,
    savedImport,
    savedRevealFolder,
    savedRefresh,
    savedEmptyNew,
    savedEmptyImport,
    savedImportOpen,
    savedImportReveal,
    savedCatalog,
    savedDeleteDialog,
    savedDeleteCancel,
    savedDeleteConfirm,
    managedLocationReveal,
    setupForm,
    setupStatus,
    setupReviewButton,
    setupCreateButton,
    setupAddAntennaButton,
    useCurrentLocationButton,
    locationStatus,
    stationGrid,
    setupAntennaTemplate,
    setupFeedback,
    setupFeedbackMessage,
    setupFeedbackDetail,
    setupDiagnostics,
    setupReviewPanel,
    setupReviewStation,
    setupReviewAntennas,
    setupReviewShape,
    setupReviewSchedule,
    setupReviewCounterbalance,
    setupReviewTransitions,
    setupReviewSequence,
    setupReviewCanDescribe,
    setupReviewCannotEstablish,
    setupReviewSlots,
    controllerSetupFields,
    controllerProfileSelect,
    controllerProfileSave,
    controllerProfileDelete,
    controllerProfileRefresh,
    conductorPanel,
    conductorEmpty,
    conductorStatus,
    conductorLifecycle,
    conductorAntennaInUse,
    conductorPhase,
    conductorGuidance,
    conductorCountdown,
    currentSlot,
    nextSlot,
    evidenceForm,
    entryPanel,
    correctionsPanel,
    addRunNote,
    openCorrections,
    evidenceKind,
    evidenceSlot,
    evidenceAntenna,
    evidenceFrequency,
    evidenceMode,
    evidencePower,
    evidenceCallsign,
    evidenceCadence,
    evidenceDetail,
    conductorFeedback,
    conductorFeedbackMessage,
    conductorFeedbackDetail,
    conductorDiagnostics,
    conductorEvents,
    antennaControllerAttach,
    antennaControllerRun,
    antennaControllerRetry,
    antennaControllerSave,
    wsprLivePhase,
    wsprLiveDetail,
    wsprLiveDiagnostic,
    wsprLiveRetry,
    wsprLiveEndWithout,
    wsjtxForm,
    wsjtxBindAddress,
    wsjtxPort,
    wsjtxClientId,
    wsjtxStart,
    wsjtxStop,
    wsjtxPhase,
    wsjtxRequirement,
    wsjtxCounts,
    wsjtxDiagnostic,
    importWsprLiveButton,
    importRbnButton,
    importFeedback,
    importFeedbackMessage,
    importFeedbackDetail,
    reportStatus,
    reportPlaceholder,
    reportViewer,
    reportFrame,
    reportBundleName,
    reportRevision,
    reportSummary,
    reportSavedButton,
    reportActiveRunButton,
    reportSummaryModeButton,
    reportFullModeButton,
    reportUpdateButton,
    reportWindowButton,
    reportRefreshButton,
    reportDiagnosticsButton,
    reportDiagnosticsDialog,
    reportDiagnosticsClose,
    reportExportButton,
    reportExportDialog,
    reportExportClose,
    reportSummaryExportButton, reportFullExportButton, reportControllerHandling,
    reportOperationalHandling, copySupportSummary,
    reportReplaceDialog, reportReplaceCancel, reportReplaceConfirm,
    reportFeedback,
    reportFeedbackMessage,
    reportFeedbackDetail,
    navigation,
    panels,
    conductorRefreshButtons,
    lifecycleButtons,
    wsjtxReadinessAcknowledge,
    skipCycleDialog,
    skipCycleReason,
    skipCycleCancel,
    skipCycleConfirm,
  } = elements;
  installContextualHelp(root);

  const render = () => {
    const workflowScrollTop = workflowScrollMemory.transition(
      state.activeWorkflow,
      mainContent.scrollTop,
    );
    renderNavigation(elements, state);
    renderSavedSessions(elements, state, root);
    renderSetup(elements, state, root);
    hydratedControllerProfile = syncControllerProfileDraft(
      setupForm,
      state.antennaControllerCatalog,
      state.antennaControllerSelectedProfile,
      hydratedControllerProfile,
    );
    scheduleControllerProfileReconciliation();
    const countdown = renderRun(elements, state, root, {
      monotonicNow,
      countdownAnchor,
      countdownKey: countdownAnchorKey,
    });
    countdownAnchor = countdown.anchor;
    countdownAnchorKey = countdown.key;
    renderEvidenceImports(elements, state);
    renderReport(elements, state, reportDocuments);
    if (workflowScrollTop !== null) mainContent.scrollTop = workflowScrollTop;
  };


  const controller = createDesktopController({
    state,
    invoke: browserWindow.__TAURI__?.core?.invoke,
    render(nextState) {
      const hadSkipDialog = state.skipCycleDialog !== null;
      state = nextState;
      render();
      if (hadSkipDialog && state.skipCycleDialog === null && skipCycleTrigger) {
        const trigger = skipCycleTrigger;
        skipCycleTrigger = null;
        Promise.resolve().then(() => trigger.isConnected && trigger.focus());
      }
    },
    navigate(workflow) {
      browserWindow.history.replaceState(null, "", `#${workflow}`);
    },
    monotonicNow,
    setInterval: (callback, milliseconds) => browserWindow.setInterval?.(callback, milliseconds),
    clearInterval: (timer) => browserWindow.clearInterval?.(timer),
    onFocus(callback) {
      browserWindow.addEventListener("focus", callback);
      return () => browserWindow.removeEventListener?.("focus", callback);
    },
    onVisibilityChange(callback) {
      rootDocument.addEventListener?.("visibilitychange", callback);
      return () => rootDocument.removeEventListener?.("visibilitychange", callback);
    },
    onHashChange(callback) {
      const listener = async () => {
        await callback(workflowFromHash(browserWindow.location.hash));
        focusActiveHeading(elements, state.activeWorkflow);
      };
      browserWindow.addEventListener("hashchange", listener);
      return () => browserWindow.removeEventListener?.("hashchange", listener);
    },
    isVisible: () => rootDocument.visibilityState !== "hidden",
    prompt: (message, initial) => browserWindow.prompt(message, initial),
    confirm: (message) => browserWindow.confirm(message),
    copyText: (value) => browserWindow.navigator.clipboard.writeText(value),
    getCountdownAnchor: () => countdownAnchor,
    renderCountdown(seconds) {
      conductorCountdown.textContent = seconds === null ? "" : formatCountdown(seconds);
    },
    onDispose() {
      if (controllerProfileReconciliationTimer !== null) {
        browserWindow.clearTimeout?.(controllerProfileReconciliationTimer);
        controllerProfileReconciliationTimer = null;
      }
      browserWindow.removeEventListener?.("pageshow", reconcileControllerProfileDomSelection);
      browserWindow.removeEventListener?.("focus", reconcileControllerProfileDomSelection);
      if (skipCycleDialog.open) skipCycleDialog.close?.();
      skipCycleDialog.removeAttribute("open");
      if (reportDiagnosticsDialog.open) reportDiagnosticsDialog.close?.();
      reportDiagnosticsDialog.removeAttribute("open");
      if (reportExportDialog.open) reportExportDialog.close?.();
      reportExportDialog.removeAttribute("open");
      skipCycleTrigger = null;
      releaseReportFrame(reportFrame, reportDocuments);
    },
  });

  function scheduleControllerProfileReconciliation() {
    const catalogKey = state.antennaControllerCatalog?.profiles
      .map((profile) => `${profile.profileId}:${profile.revision}`)
      .join("|") ?? "";
    if (catalogKey !== controllerProfileReconciliationKey) {
      controllerProfileReconciliationKey = catalogKey;
      controllerProfileReconciliationAttempts = 0;
      if (controllerProfileReconciliationTimer !== null) {
        browserWindow.clearTimeout?.(controllerProfileReconciliationTimer);
        controllerProfileReconciliationTimer = null;
      }
    }
    if (state.antennaControllerSelectedProfile) {
      controllerProfileReconciliationAttempts = 80;
      return;
    }
    if (!catalogKey || controllerProfileReconciliationAttempts >= 80
      || controllerProfileReconciliationTimer !== null) return;
    controllerProfileReconciliationTimer = browserWindow.setTimeout?.(() => {
      controllerProfileReconciliationTimer = null;
      controllerProfileReconciliationAttempts += 1;
      if (reconcileControllerProfileDomSelection()) {
        controllerProfileReconciliationAttempts = 80;
        return;
      }
      scheduleControllerProfileReconciliation();
    }, 25) ?? null;
  }

  function reconcileControllerProfileDomSelection() {
    const restoredProfileId = controllerProfileSelect.value;
    const selectedProfileId = state.antennaControllerSelectedProfile?.profileId ?? "";
    if (
      restoredProfileId === ""
      || selectedProfileId !== ""
      || !state.antennaControllerCatalog?.profiles.some(
        (profile) => profile.profileId === restoredProfileId,
      )
    ) return false;
    controller.selectAntennaControllerProfile(restoredProfileId);
    return true;
  }

  browserWindow.addEventListener("pageshow", reconcileControllerProfileDomSelection);
  browserWindow.addEventListener("focus", reconcileControllerProfileDomSelection);


  for (const button of navigation) {
    button.addEventListener("click", async () => {
      await controller.selectWorkflow(button.dataset.workflow);
      focusActiveHeading(elements, state.activeWorkflow);
    });
  }

  reportSavedButton.addEventListener("click", async () => {
    await controller.selectWorkflow("saved");
    focusActiveHeading(elements, "saved");
  });
  reportActiveRunButton.addEventListener("click", async () => {
    await controller.selectWorkflow("run");
    focusActiveHeading(elements, "run");
  });

  const startNewSession = async () => {
    await controller.selectWorkflow("setup");
    focusActiveHeading(elements, "setup");
  };
  savedNew.addEventListener("click", startNewSession);
  savedEmptyNew.addEventListener("click", startNewSession);
  const importManaged = async () => controller.importManagedSession();
  savedImport.addEventListener("click", importManaged);
  savedEmptyImport.addEventListener("click", importManaged);
  savedImportOpen.addEventListener("click", async () => {
    const locatorId = state.catalogImportNotice?.locatorId;
    if (!locatorId) return;
    await controller.openManagedSession(locatorId, null);
    focusActiveHeading(elements, state.activeWorkflow);
  });
  savedImportReveal.addEventListener("click", async () => {
    const locatorId = state.catalogImportNotice?.locatorId;
    if (locatorId) await controller.revealManagedSession(locatorId);
  });
  savedRevealFolder.addEventListener("click", () => controller.revealManagedSessionsDirectory());
  savedRefresh.addEventListener("click", () => controller.loadManagedSessions());
  savedCatalog.addEventListener("click", async (event) => {
    const button = event.target.closest("[data-saved-action]");
    if (!button) return;
    const row = button.closest(".saved-row");
    if (button.dataset.savedAction === "details") {
      const details = row?.querySelector("details");
      if (details) details.open = true;
      details?.querySelector("summary")?.focus();
      return;
    }
    if (button.dataset.savedAction === "reveal") {
      await controller.revealManagedSession(button.dataset.locatorId);
      return;
    }
    if (button.dataset.savedAction === "delete") {
      const entry = state.managedCatalog?.entries?.find(
        (candidate) => candidate.locatorId === button.dataset.locatorId,
      );
      if (!entry || state.activeManagedLocatorId === entry.locatorId) return;
      deleteTrigger = button;
      controller.requestManagedSessionDeletion(entry);
      Promise.resolve().then(() => savedDeleteCancel.focus());
      return;
    }
    if (button.dataset.savedAction === "export") {
      const locatorId = button.dataset.locatorId;
      await controller.exportManagedSession(locatorId);
      [...savedCatalog.querySelectorAll('[data-saved-action="export"]')]
        .find((candidate) => candidate.dataset.locatorId === locatorId)
        ?.focus();
      return;
    }
    await controller.openManagedSession(button.dataset.locatorId, button.dataset.intent);
    focusActiveHeading(elements, state.activeWorkflow);
  });
  const cancelDelete = () => {
    if (state.catalogDeleteStatus === "deleting") return;
    const locatorId = deleteTrigger?.dataset.locatorId;
    controller.cancelManagedSessionDeletion();
    const trigger = locatorId
      ? [...savedCatalog.querySelectorAll('[data-saved-action="delete"]')]
        .find((button) => button.dataset.locatorId === locatorId)
      : deleteTrigger;
    deleteTrigger = null;
    trigger?.focus();
  };
  savedDeleteCancel.addEventListener("click", cancelDelete);
  savedDeleteDialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    cancelDelete();
  });
  savedDeleteDialog.addEventListener("keydown", (event) => {
    if (event.key !== "Tab") return;
    const focusable = [savedDeleteCancel, savedDeleteConfirm].filter((button) => !button.disabled);
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable.at(-1);
    if (event.shiftKey && rootDocument.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && rootDocument.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  });
  savedDeleteConfirm.addEventListener("click", async () => {
    if (state.catalogDeleteStatus === "deleting") return;
    await controller.deleteManagedSession();
    if (state.catalogDeleteStatus === "succeeded") deleteTrigger = null;
  });
  managedLocationReveal.addEventListener("click", () => {
    const locatorId = state.managedLocationNotice?.locatorId;
    if (locatorId) return controller.revealManagedSession(locatorId);
    return undefined;
  });

  conductorRefreshButtons.forEach((button) => {
    button.addEventListener("click", () => controller.refreshConductor());
  });

  wsprLiveRetry.addEventListener("click", () => controller.advanceWsprLive(true));
  wsprLiveEndWithout.addEventListener("click", async () => {
    if (!controller.confirm("End this session without the final automatic bidirectional WSPR.live capture? Existing evidence will remain.")) return;
    await controller.submitConductorAction({
      kind: "end",
      reason: "Operator ended finalization without automatic bidirectional WSPR.live spots.",
    });
  });

  lifecycleButtons.forEach((button) => {
    button.addEventListener("click", async () => {
      const kind = button.dataset.conductorAction;
      if (["start", "resume"].includes(kind)) {
        const readiness = wsjtxReadinessModel(state);
        if (readiness.visible && !readiness.acknowledged) return;
      }
      if (kind === "arm_wspr_cycle") {
        const intent = state.conductor?.nextIntent;
        if (!intent) return;
        await controller.submitConductorAction({
          kind,
          intentId: intent.intentId,
          antennaLabel: intent.antennaLabel,
        });
        return;
      }
      if (kind === "skip_wspr_cycle") {
        skipCycleTrigger = button;
        controller.requestSkipCycle();
        Promise.resolve().then(() => skipCycleReason.focus());
        return;
      }
      if (kind === "start" || kind === "resume") {
        await controller.submitConductorAction({ kind, note: null });
        return;
      }
      if (kind === "abandon" && !controller.confirm("Abandon this session? Existing evidence will remain, but the lifecycle is terminal.")) return;
      const detail = controller.prompt(`Optional ${kind} reason:`, "");
      if (detail === null) return;
      await controller.submitConductorAction({ kind, reason: detail });
    });
  });

  const cancelSkipCycle = () => {
    if (state.skipCycleStatus === "submitting") return;
    controller.cancelSkipCycle();
  };
  skipCycleCancel.addEventListener("click", cancelSkipCycle);
  skipCycleDialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    cancelSkipCycle();
  });
  skipCycleDialog.addEventListener("keydown", (event) => {
    if (event.key !== "Tab") return;
    const focusable = [skipCycleReason, skipCycleCancel, skipCycleConfirm]
      .filter((control) => !control.disabled);
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable.at(-1);
    if (event.shiftKey && rootDocument.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && rootDocument.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  });
  skipCycleConfirm.addEventListener("click", async () => {
    if (state.skipCycleStatus === "submitting") return;
    await controller.submitSkipCycle(skipCycleReason.value);
  });

  wsjtxReadinessAcknowledge.addEventListener("change", () => {
    controller.setWsjtxReadinessAcknowledged(wsjtxReadinessAcknowledge.checked);
  });

  evidenceCallsign.addEventListener("input", () => {
    evidenceCallsign.value = evidenceCallsign.value.toUpperCase();
  });

  addRunNote.addEventListener("click", () => {
    if (!noteShortcutInitialized) {
      evidenceSlot.value = recommendedNoteTarget(state.conductor);
      noteShortcutInitialized = true;
    }
    entryPanel.open = true;
    evidenceKind.value = "add_note";
    evidenceDetail.focus();
  });

  entryPanel.addEventListener("toggle", () => {
    if (entryPanel.open) return;
    noteShortcutInitialized = false;
    evidenceForm.reset();
  });

  openCorrections.addEventListener("click", () => {
    entryPanel.open = true;
    correctionsPanel.open = true;
    correctionsPanel.scrollIntoView?.({ behavior: preferredScrollBehavior, block: "start" });
  });

  evidenceForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    const action = readEvidenceAction(
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
    );
    await controller.submitConductorAction(action);
    if (action.kind === "add_note" && state.conductorStatus === "ready") {
      noteShortcutInitialized = false;
      evidenceForm.reset();
      entryPanel.open = false;
    }
  });

  conductorEvents.addEventListener("click", async (event) => {
    const button = event.target.closest("button[data-event-action]");
    if (!button) return;
    const targetEventId = button.dataset.eventId;
    const reason = controller.prompt("Correction reason (required):", "");
    if (reason === null || reason.trim().length === 0) return;
    if (button.dataset.eventAction === "retract") {
      await controller.submitConductorAction({
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
    await controller.submitConductorAction({
      kind: "replace_event",
      targetEventId,
      slotId: evidenceSlot.value || null,
      replacement,
      reason,
    });
  });

  wsjtxStart.addEventListener("click", async () => {
    await controller.startWsjtx({
        bindAddress: wsjtxBindAddress.value,
        port: Number(wsjtxPort.value),
        expectedClientId: wsjtxClientId.value,
    });
  });

  wsjtxStop.addEventListener("click", async () => {
    await controller.stopWsjtx();
  });

  controllerProfileSelect.addEventListener("change", () => {
    controller.selectAntennaControllerProfile(controllerProfileSelect.value);
    controller.editSetup();
  });

  controllerProfileSave.addEventListener("click", async () => {
    const saved = await controller.saveAntennaControllerProfile(readControllerProfileDraft(setupForm));
    if (saved) controller.editSetup();
  });

  controllerProfileDelete.addEventListener("click", async () => {
    const profile = state.antennaControllerCatalog?.profiles?.find(
      (candidate) => candidate.profileId === controllerProfileSelect.value,
    );
    if (!profile) return;
    if (!controller.confirm(`Delete the controller profile “${profile.name}” from this computer? Existing sessions that used it will fall back to manual switching.`)) return;
    const deleted = await controller.deleteAntennaControllerProfile(profile.profileId);
    if (deleted) {
      controller.editSetup();
    }
  });

  controllerProfileRefresh.addEventListener("click", async () => {
    await controller.refreshAntennaControllerProfiles();
  });

  antennaControllerAttach.addEventListener("click", async () => {
    const current = state.antennaController;
    const profile = state.antennaControllerCatalog?.profiles?.find(
      (candidate) => candidate.profileId === current?.profileId,
    );
    if (!current || !profile) return;
    await controller.attachAntennaController({
      profileId: profile.profileId,
      profileRevision: profile.revision,
      targets: Object.entries(current.targets ?? {}).map(([antennaLabel, target]) => ({ antennaLabel, target })),
      armed: true,
    });
  });

  antennaControllerRun.addEventListener("click", () => controller.runAntennaController());
  antennaControllerRetry.addEventListener("click", () => controller.runAntennaController());
  antennaControllerSave.addEventListener("click", async () => {
    const profile = state.antennaControllerCatalog?.profiles?.find(
      (candidate) => candidate.profileId === state.antennaController?.profileId,
    );
    if (!profile) return;
    const field = (name) => root.querySelector(`[data-active-controller-field="${name}"]`).value;
    const lines = (name) => field(name) === "" ? [] : field(name).split(/\r?\n/);
    const structured = state.antennaControllerCatalog?.inputStyle === "structured";
    const switchCommand = structured
      ? { oneLine: "", program: field("switchProgram"), arguments: lines("switchArguments") }
      : { oneLine: field("switchCommand"), program: "", arguments: [] };
    const hasVerification = structured ? field("verificationProgram") : field("verificationCommand");
    const verificationCommand = !hasVerification
      ? null
      : structured
        ? { oneLine: "", program: field("verificationProgram"), arguments: lines("verificationArguments") }
        : { oneLine: field("verificationCommand"), program: "", arguments: [] };
    await controller.saveAntennaControllerProfile({
      profileId: profile.profileId,
      name: field("name"),
      timeoutSeconds: Number(field("timeoutSeconds")),
      switchCommand,
      verificationCommand,
    });
  });

  setupForm.addEventListener("input", (event) => {
    if (event.target.matches?.('[data-setup-field="callsign"], [data-setup-field="signalTransmittedCallsign"]')) {
      event.target.value = event.target.value.toUpperCase();
    } else if (event.target.matches?.('[data-setup-field="grid"]')) {
      event.target.value = normalizeMaidenheadGrid(event.target.value);
    }
    if (event.target.matches?.("[data-setup-question]")) {
      selectSetupQuestion(setupForm, event.target.value);
    } else if (event.target.matches?.('[data-setup-field="mode"]')) {
      syncSetupQuestionToMode(setupForm);
    }
    syncSignalPlanFields(setupForm);
    syncControllerSetupFields(setupForm, controllerSetupFields);
    if (event.target.matches?.('[data-antenna-field="label"]')) {
      syncControllerTargets(setupForm);
    }
    if (!setupBusyState(state)) {
      controller.editSetup();
    }
  });

  useCurrentLocationButton.addEventListener("click", async () => {
    useCurrentLocationButton.disabled = true;
    useCurrentLocationButton.textContent = "Requesting…";
    locationStatus.textContent = "Requesting macOS location permission or a one-time location…";
    try {
      const outcome = await controller.requestStationLocation();
      if (outcome.status !== "success") {
        locationStatus.textContent = locationLookupMessage(outcome);
        return;
      }
      stationGrid.value = maidenheadGrid(outcome.latitude, outcome.longitude);
      controller.editSetup();
      locationStatus.textContent = `Estimated ${stationGrid.value}; raw coordinates were not saved.`;
    } catch (error) {
      locationStatus.textContent = error?.message || locationLookupMessage(null);
    } finally {
      useCurrentLocationButton.disabled = false;
      useCurrentLocationButton.textContent = "Use current location";
    }
  });

  setupForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    const outcome = await controller.reviewSetup(readSetupDraft(setupForm));
    focusSetupOutcome(
      outcome,
      setupReviewPanel,
      setupDiagnostics,
      setupForm,
      preferredScrollBehavior,
    );
  });

  setupCreateButton.addEventListener("click", async () => {
    await controller.createSession();
  });

  setupAddAntennaButton.addEventListener("click", () => {
    const fragment = setupAntennaTemplate.content.cloneNode(true);
    setupAddAntennaButton.before(fragment);
    refreshAntennaRows(setupForm);
    syncControllerTargets(setupForm);
    controller.editSetup();
  });

  setupForm.addEventListener("click", (event) => {
    const removeButton = event.target.closest("[data-remove-antenna]");
    if (!removeButton) return;
    const rows = setupForm.querySelectorAll("[data-antenna-row]");
    if (rows.length <= 1) return;
    removeButton.closest("[data-antenna-row]").remove();
    refreshAntennaRows(setupForm);
    syncControllerTargets(setupForm);
    controller.editSetup();
  });

  importWsprLiveButton.addEventListener("click", async () => {
    await controller.importWsprLive();
  });
  importRbnButton.addEventListener("click", async () => {
    await controller.importRbn();
  });

  reportRefreshButton.addEventListener("click", () => controller.refreshReport());
  reportUpdateButton.addEventListener("click", () => { void controller.applyReportUpdate(); });
  reportWindowButton.addEventListener("click", () => { void controller.openReportWindow(); });
  reportSummaryModeButton.addEventListener("click", () => controller.selectReportMode("summary"));
  reportFullModeButton.addEventListener(
    "click",
    () => controller.selectReportMode("full_evidence"),
  );
  reportDiagnosticsButton.addEventListener("click", () => {
    if (typeof reportDiagnosticsDialog.showModal === "function") {
      reportDiagnosticsDialog.showModal();
    } else {
      reportDiagnosticsDialog.setAttribute("open", "");
    }
    Promise.resolve().then(() => reportDiagnosticsClose.focus());
  });
  const closeReportDiagnostics = () => {
    if (reportDiagnosticsDialog.open) reportDiagnosticsDialog.close?.();
    reportDiagnosticsDialog.removeAttribute("open");
    reportDiagnosticsButton.focus();
  };
  reportDiagnosticsClose.addEventListener("click", closeReportDiagnostics);
  reportDiagnosticsDialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    closeReportDiagnostics();
  });
  reportExportButton.addEventListener("click", () => {
    if (typeof reportExportDialog.showModal === "function") {
      reportExportDialog.showModal();
    } else {
      reportExportDialog.setAttribute("open", "");
    }
    Promise.resolve().then(() => reportExportClose.focus());
  });
  const closeReportExport = () => {
    if (reportExportDialog.open) reportExportDialog.close?.();
    reportExportDialog.removeAttribute("open");
    reportExportButton.focus();
  };
  reportExportClose.addEventListener("click", closeReportExport);
  reportExportDialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    closeReportExport();
  });
  reportSummaryExportButton.addEventListener("click", async () => {
    reportExportTrigger = reportSummaryExportButton;
    await controller.exportReport("summary_html");
    if (state.reportExportStatus === "confirming") {
      Promise.resolve().then(() => reportReplaceCancel.focus());
    } else {
      reportExportTrigger = null;
      closeReportExport();
    }
  });
  reportFullExportButton.addEventListener("click", async () => {
    reportExportTrigger = reportFullExportButton;
    await controller.exportReport(
      "full_evidence_html",
      reportControllerHandling.value,
      reportOperationalHandling.value,
    );
    if (state.reportExportStatus === "confirming") {
      Promise.resolve().then(() => reportReplaceCancel.focus());
    } else {
      reportExportTrigger = null;
      closeReportExport();
    }
  });
  const cancelReportReplacement = async () => {
    if (["replacing", "cancelling"].includes(state.reportExportStatus)) return;
    const trigger = reportExportTrigger;
    await controller.cancelReportReplacement();
    if (!state.reportExportPending) {
      reportExportTrigger = null;
      trigger?.focus();
    }
  };
  reportReplaceCancel.addEventListener("click", cancelReportReplacement);
  reportReplaceDialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    void cancelReportReplacement();
  });
  reportReplaceDialog.addEventListener("keydown", (event) => {
    if (event.key !== "Tab") return;
    const focusable = [reportReplaceCancel, reportReplaceConfirm]
      .filter((button) => !button.disabled);
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable.at(-1);
    if (event.shiftKey && rootDocument.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && rootDocument.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  });
  reportReplaceConfirm.addEventListener("click", async () => {
    if (["replacing", "cancelling"].includes(state.reportExportStatus)) return;
    await controller.confirmReportReplacement();
    if (!state.reportExportPending) {
      reportExportTrigger = null;
      closeReportExport();
    }
  });
  copySupportSummary.addEventListener("click", async () => {
    await controller.copySupportSummary();
  });

  syncSignalPlanFields(setupForm);
  refreshAntennaRows(setupForm);
  syncControllerTargets(setupForm);
  syncControllerSetupFields(setupForm, controllerSetupFields);
  syncSetupQuestionToMode(setupForm);
  render();
  if (typeof browserWindow.__TAURI__?.core?.invoke === "function") {
    void controller.loadStationPreferences()
      .then((preferences) => applyStationPreferences(setupForm, preferences))
      .catch(() => {});
    void controller.loadAntennaControllerProfiles();
    if (state.activeWorkflow === "saved") void controller.loadManagedSessions();
  }
  controller.start();
  return controller;
}

function focusActiveHeading(elements, workflow) {
  const panel = elements.panels.find((candidate) => candidate.dataset.panel === workflow);
  const heading = panel?.querySelector("h1");
  if (!heading) return;
  heading.tabIndex = -1;
  heading.focus({ preventScroll: true });
}

function canonicalCommandLine(command) {
  if (!command) return "";
  return [command.programTemplate, ...command.argumentTemplates]
    .map((token) => JSON.stringify(token))
    .join(" ");
}

function applyControllerProfile(form, profile) {
  const set = (field, value) => {
    form.querySelector(`[data-setup-field="${field}"]`).value = value ?? "";
  };
  set("controllerProfileName", profile?.name ?? "");
  set("controllerTimeoutSeconds", profile?.timeoutSeconds ?? 10);
  set("controllerSwitchCommand", canonicalCommandLine(profile?.switchCommand));
  set("controllerVerificationCommand", canonicalCommandLine(profile?.verificationCommand));
  set("controllerSwitchProgram", profile?.switchCommand?.programTemplate ?? "");
  set("controllerSwitchArguments", profile?.switchCommand?.argumentTemplates?.join("\n") ?? "");
  set("controllerVerificationProgram", profile?.verificationCommand?.programTemplate ?? "");
  set("controllerVerificationArguments", profile?.verificationCommand?.argumentTemplates?.join("\n") ?? "");
}

function controllerProfileKey(profile) {
  return profile ? `${profile.profileId}:${profile.revision}` : "new";
}

function syncControllerProfileDraft(form, catalog, selection, synchronizedProfile) {
  if (!catalog) return synchronizedProfile;
  const profile = catalog.profiles.find(
    (candidate) => candidate.profileId === selection?.profileId
      && candidate.revision === selection.revision,
  ) ?? null;
  const nextProfile = controllerProfileKey(profile);
  if (profile && nextProfile !== synchronizedProfile) {
    applyControllerProfile(form, profile);
    return nextProfile;
  }
  if (!profile && synchronizedProfile !== null && synchronizedProfile !== nextProfile) {
    applyControllerProfile(form, null);
    return nextProfile;
  }
  return synchronizedProfile ?? nextProfile;
}

function syncControllerSetupFields(form, fields) {
  const enabled = form.querySelector('[data-setup-field="antennaControllerEnabled"]').checked;
  fields.hidden = !enabled;
  for (const control of fields.querySelectorAll("input, select, textarea")) {
    control.disabled = !enabled;
  }
}

let controllerTargetKey = 0;

function syncControllerTargets(form) {
  const targetList = form.querySelector("[data-controller-targets]");
  if (!targetList) return;
  const values = new Map(
    [...targetList.querySelectorAll("[data-controller-target]")]
      .map((input) => [input.dataset.controllerTargetKey, input.value]),
  );
  const enabled = form.querySelector('[data-setup-field="antennaControllerEnabled"]').checked;
  const document = form.ownerDocument;
  const fields = [...form.querySelectorAll("[data-antenna-row]")].map((row, index) => {
    row.dataset.controllerTargetKey ||= `controller-target-${controllerTargetKey++}`;
    const antennaLabel = row.querySelector('[data-antenna-field="label"]').value.trim();
    const heading = antennaLabel || `Antenna ${String.fromCharCode(65 + index)}`;
    const field = document.createElement("div");
    field.className = "controller-target-field field-control";
    field.dataset.controllerTargetRow = "";
    const name = document.createElement("strong");
    name.textContent = heading;
    const label = document.createElement("label");
    label.textContent = "Controller value";
    const input = document.createElement("input");
    input.dataset.controllerTarget = "";
    input.dataset.controllerTargetKey = row.dataset.controllerTargetKey;
    input.dataset.antennaLabel = antennaLabel;
    input.value = values.get(row.dataset.controllerTargetKey) ?? "";
    input.disabled = !enabled;
    label.append(input);
    field.append(name, label);
    return field;
  });
  targetList.replaceChildren(...fields);
}

function setupBusyState(state) {
  return ["reviewing", "creating"].includes(state.setupStatus);
}

function syncSignalPlanFields(form) {
  const enabled = form.querySelector('[data-setup-field="signalPlanEnabled"]').checked;
  const fields = form.querySelector("[data-signal-plan-fields]");
  fields.hidden = !enabled;
  for (const control of fields.querySelectorAll("input, select, textarea")) {
    control.disabled = !enabled;
  }
  syncWsprLiveForSignalPlan(form, enabled);
}

function refreshAntennaRows(form) {
  const rows = [...form.querySelectorAll("[data-antenna-row]")];
  rows.forEach((row, index) => {
    row.querySelector("[data-antenna-title]").textContent = `Antenna ${String.fromCharCode(65 + index)}`;
    row.querySelector("[data-remove-antenna]").disabled = rows.length <= 1;
  });
}

if (typeof document !== "undefined") {
  mount(document, window);
}
