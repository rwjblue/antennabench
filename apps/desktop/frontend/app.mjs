import { createDesktopController } from "./controller.mjs";
import { collectDesktopElements } from "./elements.mjs";
import {
  applyStationPreferences,
  normalizeMaidenheadGrid,
  readEvidenceAction,
  readEvidenceReplacement,
  readSetupDraft,
  readSignalEvidenceFields,
  selectSetupQuestion,
  syncSetupQuestionToMode,
  syncWsprLiveForSignalPlan,
} from "./forms.mjs";
import {
  focusSetupOutcome,
  installContextualHelp,
  locationLookupMessage,
  maidenheadGrid,
  recommendedNoteTarget,
  workflowFromHash,
} from "./models.mjs";
import { initialState } from "./state.mjs";
import {
  formatCountdown,
  renderNavigation,
  renderReport,
  renderRun,
  renderSetup,
  renderTransfer,
} from "./renderers.mjs";

export function mount(root, browserWindow) {
  const rootDocument = root.ownerDocument ?? root;
  let state = initialState(workflowFromHash(browserWindow.location.hash));
  let countdownAnchor = null;
  let countdownAnchorKey = null;
  let noteShortcutInitialized = false;
  const monotonicNow = () => browserWindow.performance?.now?.() ?? Date.now();
  const elements = collectDesktopElements(root);
  const {
    mainContent,
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
    openButton,
    exportButton,
    importWsprLiveButton,
    importRbnButton,
    transferStatus,
    openFeedback,
    feedbackMessage,
    feedbackDetail,
    exportFeedback,
    exportFeedbackMessage,
    exportFeedbackDetail,
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
    reportRefreshButton,
    reportCompactExportButton, reportFullExportButton,
    reportFeedback,
    reportFeedbackMessage,
    reportFeedbackDetail,
    navigation,
    panels,
    conductorRefreshButtons,
    lifecycleButtons,
  } = elements;
  installContextualHelp(root);

  const render = () => {
    renderNavigation(elements, state);
    renderSetup(elements, state, root);
    const countdown = renderRun(elements, state, root, {
      monotonicNow,
      countdownAnchor,
      countdownKey: countdownAnchorKey,
    });
    countdownAnchor = countdown.anchor;
    countdownAnchorKey = countdown.key;
    renderTransfer(elements, state);
    renderReport(elements, state);
  };


  const controller = createDesktopController({
    state,
    invoke: browserWindow.__TAURI__?.core?.invoke,
    render(nextState) {
      state = nextState;
      render();
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
      const listener = () => callback(workflowFromHash(browserWindow.location.hash));
      browserWindow.addEventListener("hashchange", listener);
      return () => browserWindow.removeEventListener?.("hashchange", listener);
    },
    isVisible: () => rootDocument.visibilityState !== "hidden",
    prompt: (message, initial) => browserWindow.prompt(message, initial),
    confirm: (message) => browserWindow.confirm(message),
    getCountdownAnchor: () => countdownAnchor,
    renderCountdown(seconds) {
      conductorCountdown.textContent = seconds === null ? "" : formatCountdown(seconds);
    },
  });


  for (const button of navigation) {
    button.addEventListener("click", async () => {
      await controller.selectWorkflow(button.dataset.workflow);
      mainContent.focus({ preventScroll: true });
    });
  }

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
        const intent = state.conductor?.nextIntent;
        if (!intent) return;
        const reason = controller.prompt(`Optional reason for skipping cycle ${intent.sequenceNumber}:`, "");
        if (reason === null) return;
        await controller.submitConductorAction({
          kind,
          intentId: intent.intentId,
          reason,
        });
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
    correctionsPanel.scrollIntoView?.({ behavior: "smooth", block: "start" });
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
    const profile = state.antennaControllerCatalog?.profiles?.find(
      (candidate) => candidate.profileId === controllerProfileSelect.value,
    );
    if (profile) applyControllerProfile(setupForm, profile);
    controller.editSetup();
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
    focusSetupOutcome(outcome, setupReviewPanel, setupDiagnostics);
  });

  setupCreateButton.addEventListener("click", async () => {
    await controller.createSession();
  });

  setupAddAntennaButton.addEventListener("click", () => {
    const fragment = setupAntennaTemplate.content.cloneNode(true);
    setupAddAntennaButton.before(fragment);
    refreshAntennaRows(setupForm);
    controller.editSetup();
  });

  setupForm.addEventListener("click", (event) => {
    const removeButton = event.target.closest("[data-remove-antenna]");
    if (!removeButton) return;
    const rows = setupForm.querySelectorAll("[data-antenna-row]");
    if (rows.length <= 1) return;
    removeButton.closest("[data-antenna-row]").remove();
    refreshAntennaRows(setupForm);
    controller.editSetup();
  });

  openButton.addEventListener("click", async () => {
    await controller.openSession();
  });

  exportButton.addEventListener("click", async () => {
    await controller.exportSession();
  });

  importWsprLiveButton.addEventListener("click", async () => {
    await controller.importWsprLive();
  });
  importRbnButton.addEventListener("click", async () => {
    await controller.importRbn();
  });

  reportRefreshButton.addEventListener("click", () => controller.refreshReport());
  reportCompactExportButton.addEventListener("click", async () => {
    await controller.exportReport("compact_summary_html");
  });
  reportFullExportButton.addEventListener("click", async () => {
    await controller.exportReport("full_evidence_html");
  });

  syncSignalPlanFields(setupForm);
  syncControllerSetupFields(setupForm, controllerSetupFields);
  syncSetupQuestionToMode(setupForm);
  render();
  if (typeof browserWindow.__TAURI__?.core?.invoke === "function") {
    void controller.loadStationPreferences()
      .then((preferences) => applyStationPreferences(setupForm, preferences))
      .catch(() => {});
    void controller.loadAntennaControllerProfiles();
  }
  controller.start();
  return controller;
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
  set("controllerProfileName", profile.name);
  set("controllerTimeoutSeconds", profile.timeoutSeconds);
  set("controllerSwitchCommand", canonicalCommandLine(profile.switchCommand));
  set("controllerVerificationCommand", canonicalCommandLine(profile.verificationCommand));
  set("controllerSwitchProgram", profile.switchCommand.programTemplate);
  set("controllerSwitchArguments", profile.switchCommand.argumentTemplates.join("\n"));
  set("controllerVerificationProgram", profile.verificationCommand?.programTemplate ?? "");
  set("controllerVerificationArguments", profile.verificationCommand?.argumentTemplates?.join("\n") ?? "");
}

function syncControllerSetupFields(form, fields) {
  const enabled = form.querySelector('[data-setup-field="antennaControllerEnabled"]').checked;
  fields.hidden = !enabled;
  for (const control of fields.querySelectorAll("input, select, textarea")) {
    control.disabled = !enabled;
  }
  for (const target of form.querySelectorAll("[data-controller-target-field]")) {
    target.hidden = !enabled;
    target.querySelector("input").disabled = !enabled;
  }
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
