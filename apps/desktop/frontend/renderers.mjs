import {
  conductorActionAvailable,
  createCountdownAnchor,
  formatActiveRunTime,
  projectCountdown,
  setupPlanEstimate,
  updateReportFrame,
  viewModel,
  wsprLiveAcquisitionModel,
  wsjtxReadinessModel,
} from "./models.mjs";

export function renderNavigation(elements, state) {
  for (const item of viewModel(state)) {
    const button = elements.navigation.find(
      (candidate) => candidate.dataset.workflow === item.workflow,
    );
    const panel = elements.panels.find(
      (candidate) => candidate.dataset.panel === item.workflow,
    );
    button.classList.toggle("active", item.active);
    button.setAttribute("aria-current", item.active ? "page" : "false");
    panel.hidden = !item.active;
  }
}

export function renderSetup(elements, state, root) {
  const {
    setupForm, setupReviewButton, setupCreateButton, setupStatus, setupFeedback,
    setupFeedbackMessage, setupFeedbackDetail, setupDiagnostics, setupReviewPanel,
    setupReviewStation, setupReviewAntennas, setupReviewShape, setupReviewSlots,
    setupReviewSchedule, setupReviewCounterbalance, setupReviewTransitions,
    setupReviewSequence, setupReviewCanDescribe, setupReviewCannotEstablish,
    setupPlanSummary,
    controllerOneLine, controllerStructured, controllerProfileSelect,
    controllerProfileSave, controllerProfileDelete, controllerProfileStatus,
  } = elements;
  const setupBusy = ["reviewing", "creating"].includes(state.setupStatus);
  setupForm.setAttribute("aria-busy", String(setupBusy));
  setupReviewButton.disabled = setupBusy;
  setupReviewButton.textContent = state.setupStatus === "reviewing"
    ? "Validating…"
    : "Review plan";
  setupCreateButton.disabled = state.setupStatus !== "reviewed";
  setupCreateButton.textContent = state.setupStatus === "creating" ? "Creating…" : "Create session";
  setupStatus.textContent = setupStatusText(state);
  setupStatus.classList.toggle("muted", ["editing", "invalid", "error"].includes(state.setupStatus));
  setupPlanSummary.textContent = setupPlanEstimate({
    mode: setupForm.querySelector('[data-setup-field="mode"]').value,
    rounds: setupForm.querySelector('[data-setup-field="rounds"]').value,
    antennaCount: setupForm.querySelectorAll("[data-antenna-row]").length,
    signalPlanEnabled: setupForm.querySelector('[data-setup-field="signalPlanEnabled"]').checked,
    frequenciesHz: setupForm.querySelector('[data-setup-field="signalFrequenciesHz"]').value,
  });
  const catalog = state.antennaControllerCatalog;
  if (catalog) {
    const selected = state.antennaControllerProfileNotice?.profileId
      ?? controllerProfileSelect.value;
    const signature = catalog.profiles.map((profile) => `${profile.profileId}:${profile.revision}`).join("|");
    if (controllerProfileSelect.dataset.catalogSignature !== signature) {
      replaceSelectOptions(controllerProfileSelect, [
        { value: "", label: "Create a new profile" },
        ...catalog.profiles.map((profile) => ({ value: profile.profileId, label: profile.name })),
      ]);
      controllerProfileSelect.value = catalog.profiles.some((profile) => profile.profileId === selected)
        ? selected
        : "";
      controllerProfileSelect.dataset.catalogSignature = signature;
    }
    const structured = catalog.inputStyle === "structured";
    controllerOneLine.hidden = structured;
    controllerStructured.hidden = !structured;
  }
  const profileBusy = ["saving", "deleting"].includes(state.antennaControllerStatus);
  controllerProfileSave.disabled = profileBusy;
  controllerProfileSave.textContent = state.antennaControllerStatus === "saving"
    ? "Saving…"
    : controllerProfileSelect.value === ""
      ? "Save profile"
      : "Update profile";
  controllerProfileDelete.disabled = profileBusy || controllerProfileSelect.value === "";
  controllerProfileDelete.textContent = state.antennaControllerStatus === "deleting"
    ? "Deleting…"
    : "Delete profile";
  const profileNotice = state.antennaControllerProfileNotice;
  controllerProfileStatus.dataset.kind = state.antennaControllerProfileError ? "error" : "ready";
  controllerProfileStatus.textContent = state.antennaControllerProfileError
    ? state.antennaControllerProfileError.detail ?? state.antennaControllerProfileError.message
    : profileNotice?.kind === "deleted"
      ? "Profile deleted from this computer."
      : profileNotice?.kind === "saved"
        ? "Profile saved on this computer."
        : "";

  renderFeedback(
    setupFeedback,
    setupFeedbackMessage,
    setupFeedbackDetail,
    setupFeedbackModel(state),
  );
  const diagnostics = [
    ...(state.setupReview?.diagnostics ?? []),
    ...(state.antennaControllerProfileError ? [{
      field: "antennaController",
      message: state.antennaControllerProfileError.detail ?? state.antennaControllerProfileError.message,
      code: "setup.antenna_controller.profile",
      severity: "error",
    }] : []),
  ];
  renderSetupFieldDiagnostics(setupForm, diagnostics, root);
  setupDiagnostics.replaceChildren(...diagnostics.map((diagnostic) => {
    const item = root.createElement("li");
    const field = root.createElement("strong");
    field.textContent = diagnostic.field;
    const message = root.createElement("span");
    message.textContent = diagnostic.message;
    item.append(field, message);
    return item;
  }));
  setupDiagnostics.hidden = diagnostics.length === 0;

  const plan = state.setupReview?.plan ?? null;
  setupReviewPanel.hidden = plan === null;
  if (!plan) return;
  setupReviewStation.textContent = `${plan.station.callsign} · ${plan.station.grid} · ${plan.station.powerWatts ?? "power not set"}${plan.station.powerWatts === null ? "" : " W"}`;
  setupReviewAntennas.textContent = plan.antennas
    .map((antenna, index) => `${String.fromCharCode(65 + index)}: ${antenna.label}${antenna.context ? ` — ${antenna.context}` : ""}`)
    .join("\n");
  const signalSummary = plan.signalPlan
    ? `${humanizeIdentifier(plan.signalPlan.mode)} · ${humanizeIdentifier(plan.signalPlan.collectionProfile)} · ${plan.signalPlan.frequenciesHz.length} frequencies`
    : `WSPR.live ${plan.wsprLiveAcquisitionEnabled ? "enabled" : "off"}`;
  const controllerSummary = plan.antennaController
    ? ` · controller ${plan.antennaController.profileName} · ${humanizeIdentifier(plan.antennaController.invocation)} · ${plan.antennaController.manualReviewRequired ? "operator ready required" : "command-verified readiness"}`
    : " · manual antenna control";
  setupReviewShape.textContent = `${humanizeIdentifier(plan.mode)} · ${humanizeIdentifier(plan.goal)} · ${signalSummary}${controllerSummary}`;
  setupReviewSchedule.textContent = plan.scheduleReview.summary;
  setupReviewCounterbalance.textContent = plan.scheduleReview.counterbalanceExplanation;
  setupReviewTransitions.textContent = plan.scheduleReview.transitionSummary;
  setupReviewSequence.replaceChildren(...plan.slots.map((slot, index) => {
    const item = root.createElement("li");
    const transition = plan.scheduleReview.transitions[index - 1];
    if (transition) {
      const change = root.createElement("span");
      change.className = "cycle-transition";
      change.textContent = transition.summary;
      item.append(change);
    }
    const cycle = root.createElement("strong");
    cycle.textContent = `${slot.sequenceNumber}. ${slot.direction ? humanizeIdentifier(slot.direction) : "Signal"} · ${slot.antennaLabel}`;
    const context = root.createElement("small");
    context.textContent = slot.signal
      ? `${slot.band} · ${slot.signal.frequencyHz} Hz`
      : slot.band;
    item.append(cycle, context);
    return item;
  }));
  setupReviewCanDescribe.replaceChildren(...plan.capabilities.canDescribe.map((statement) => {
    const item = root.createElement("li");
    item.textContent = statement;
    return item;
  }));
  setupReviewCannotEstablish.replaceChildren(...plan.capabilities.cannotEstablish.map((statement) => {
    const item = root.createElement("li");
    item.textContent = statement;
    return item;
  }));
  setupReviewSlots.replaceChildren(...plan.slots.map((slot) => {
    const row = root.createElement("tr");
    for (const value of [
      slot.sequenceNumber,
      slot.antennaLabel,
      slot.direction ? humanizeIdentifier(slot.direction) : "—",
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
  }));
}

function renderSetupFieldDiagnostics(form, diagnostics, root) {
  for (const diagnostic of form.querySelectorAll("[data-field-diagnostic]")) {
    diagnostic.remove();
  }
  for (const control of form.querySelectorAll("[data-setup-invalid]")) {
    control.removeAttribute("aria-invalid");
    control.removeAttribute("data-setup-invalid");
    const describedBy = (control.getAttribute("aria-describedby") ?? "")
      .split(/\s+/)
      .filter((id) => id && !id.startsWith("setup-field-error-"));
    if (describedBy.length > 0) control.setAttribute("aria-describedby", describedBy.join(" "));
    else control.removeAttribute("aria-describedby");
  }
  diagnostics.forEach((diagnostic, index) => {
    const control = setupControlForDiagnostic(form, diagnostic);
    if (!control) return;
    const id = `setup-field-error-${index + 1}`;
    const message = root.createElement("small");
    message.id = id;
    message.className = "field-error";
    message.dataset.fieldDiagnostic = "";
    message.textContent = diagnostic.message;
    const container = control.closest("label, .field-control") ?? control.parentElement;
    container?.append(message);
    control.dataset.setupInvalid = "";
    if (diagnostic.severity !== "warning") control.setAttribute("aria-invalid", "true");
    const describedBy = new Set((control.getAttribute("aria-describedby") ?? "").split(/\s+/).filter(Boolean));
    describedBy.add(id);
    control.setAttribute("aria-describedby", [...describedBy].join(" "));
  });
}

function setupControlForDiagnostic(form, diagnostic) {
  const directFields = {
    "station.callsign": "callsign",
    "station.grid": "grid",
    "station.powerWatts": "powerWatts",
    "schedule.mode": "mode",
    "schedule.goal": "goal",
    "schedule.band": "band",
    "schedule.rounds": "rounds",
    wsprLiveAcquisitionEnabled: "wsprLiveAcquisitionEnabled",
    "signalPlan.plannedPowerWatts": "signalPlannedPowerWatts",
    "signalPlan.transmittedCallsign": "signalTransmittedCallsign",
    "signalPlan.differingIdentityValidated": "signalDifferingIdentityValidated",
    "signalPlan.message": "signalMessage",
    "signalPlan.repetitionCount": "signalRepetitionCount",
    "signalPlan.keySpeedWpm": "signalKeySpeedWpm",
    "signalPlan.transmitSeconds": "signalTransmitSeconds",
    "signalPlan.intervalSeconds": "signalIntervalSeconds",
    "signalPlan.frequenciesHz": "signalFrequenciesHz",
  };
  if (directFields[diagnostic.field]) {
    return form.querySelector(`[data-setup-field="${directFields[diagnostic.field]}"]`);
  }
  const antenna = diagnostic.field?.match(/^antennas\.(\d+)\.(\w+)$/);
  if (antenna) {
    return form.querySelectorAll("[data-antenna-row]")[Number(antenna[1])]
      ?.querySelector(`[data-antenna-field="${antenna[2]}"]`) ?? null;
  }
  if (diagnostic.field === "antennas") {
    return form.querySelector('[data-antenna-row] [data-antenna-field="label"]');
  }
  if (diagnostic.field === "station") {
    return form.querySelector('[data-setup-field="callsign"]');
  }
  if (diagnostic.field === "schedule") {
    return form.querySelector('[data-setup-field="mode"]');
  }
  if (diagnostic.field === "signalPlan") {
    return form.querySelector('[data-setup-field="signalTransmittedCallsign"]');
  }
  if (diagnostic.field === "antennaController") {
    return controllerControlForMessage(form, diagnostic.message);
  }
  return null;
}

function controllerControlForMessage(form, message = "") {
  const normalized = message.toLowerCase();
  if (normalized.includes("profile name")) {
    return form.querySelector('[data-setup-field="controllerProfileName"]');
  }
  if (normalized.includes("timeout")) {
    return form.querySelector('[data-setup-field="controllerTimeoutSeconds"]');
  }
  if (normalized.includes("verification")) {
    return visibleControllerControl(form, "controllerVerificationCommand", "controllerVerificationProgram");
  }
  if (normalized.includes("target") || normalized.includes("antenna")) {
    const quotedAntenna = message.match(/antenna ["“]([^"”]+)["”]/u)?.[1];
    const targets = [...form.querySelectorAll("[data-controller-target]")];
    return targets.find((target) => target.dataset.antennaLabel === quotedAntenna)
      ?? targets.find((target) => target.value.trim() === "")
      ?? targets[0]
      ?? null;
  }
  if (normalized.includes("saved") || normalized.includes("profile")) {
    return form.querySelector('[data-setup-field="controllerProfileId"]');
  }
  return visibleControllerControl(form, "controllerSwitchCommand", "controllerSwitchProgram");
}

function visibleControllerControl(form, oneLine, structured) {
  const oneLineControl = form.querySelector(`[data-setup-field="${oneLine}"]`);
  return oneLineControl?.closest("[hidden]")
    ? form.querySelector(`[data-setup-field="${structured}"]`)
    : oneLineControl;
}

export function renderRun(elements, state, root, options = {}) {
  const {
    conductorPanel, conductorEmpty, conductorStatus, conductorRefreshButtons,
    evidenceForm, conductorFeedback, conductorFeedbackMessage, conductorFeedbackDetail,
    conductorLifecycle, conductorAntennaInUse, conductorPhase, conductorGuidance,
    conductorCountdown, skipCycleControl, currentSlot, nextSlot, evidenceSlot, evidenceAntenna,
    wsjtxReadiness, wsjtxReadinessItems, wsjtxReadinessAcknowledge,
    lifecycleButtons, conductorDiagnostics, conductorEvents, wsjtxForm, wsjtxStart,
    wsjtxStop, wsjtxRequirement, wsjtxPhase, wsjtxCounts, wsjtxSetupWarnings, wsjtxDiagnostic,
    wsprLivePhase, wsprLiveCompact, wsprLiveDetail, wsprLiveDiagnostic, wsprLiveRetry,
    wsprLiveEndWithout,
    antennaControllerStatus, antennaControllerDetail, antennaControllerDiagnostic,
    antennaControllerAttach,
    antennaControllerRun, antennaControllerRetry, antennaControllerEditor,
    antennaControllerOneLine, antennaControllerStructured,
  } = elements;
  const conductorBusy = ["loading", "refreshing", "mutating"].includes(state.conductorStatus);
  const hasConductor = state.conductor !== null;
  conductorPanel.hidden = !hasConductor;
  conductorEmpty.hidden = hasConductor;
  conductorStatus.textContent = conductorStatusText(state);
  conductorStatus.classList.toggle("muted", !hasConductor || state.conductorStatus === "error");
  conductorRefreshButtons.forEach((button) => { button.disabled = conductorBusy; });
  evidenceForm.setAttribute("aria-busy", String(conductorBusy));
  evidenceForm.querySelector("button[type=submit]").disabled = conductorBusy || !hasConductor;
  renderFeedback(
    conductorFeedback,
    conductorFeedbackMessage,
    conductorFeedbackDetail,
    conductorFeedbackModel(state),
  );

  if (!hasConductor) {
    wsjtxReadiness.hidden = true;
    lifecycleButtons.forEach((button) => { button.disabled = true; });
    conductorDiagnostics.replaceChildren();
    conductorEvents.replaceChildren();
    wsjtxStart.disabled = true;
    wsjtxStop.disabled = true;
    antennaControllerAttach.hidden = true;
    antennaControllerRun.hidden = true;
    antennaControllerRetry.hidden = true;
    antennaControllerDiagnostic.hidden = true;
    return { anchor: null, key: null };
  }

  const view = state.conductor;
  const readiness = wsjtxReadinessModel(state);
  wsjtxReadiness.hidden = !readiness.visible;
  wsjtxReadinessItems.replaceChildren(...readiness.items.map((text) => {
    const item = root.createElement("li");
    item.textContent = text;
    return item;
  }));
  wsjtxReadinessAcknowledge.checked = readiness.acknowledged;
  wsjtxReadinessAcknowledge.disabled = conductorBusy;
  const controller = state.antennaController;
  const controllerBusy = ["loading", "attaching", "saving", "running"].includes(state.antennaControllerStatus);
  antennaControllerStatus.textContent = controller?.armed
    ? `${controller.profileName ?? "Local profile"} ready · ${humanizeIdentifier(controller.automationStatus ?? "idle")}`
    : controller?.profileId
      ? `${controller.profileName ?? "Saved profile"} not allowed to run`
      : controller?.policy === "command_controlled"
        ? "No local profile attached"
        : "Manual only";
  antennaControllerDetail.textContent = state.antennaControllerError?.message
    ?? state.antennaControllerOutcome?.detail
    ?? controller?.lastAttempt?.detail
    ?? (controller?.staleProfile
      ? "The saved profile changed. Review it and allow its current revision to run before retrying."
      : controller?.manualReviewRequired === false
        ? "Successful switch and independent verification commands authorize the next eligible WSPR boundary. Manual ready remains available as fallback."
        : "Successful commands wait for the named operator ready action; manual operation remains available.");
  const controllerDiagnostic = state.antennaControllerOutcome?.diagnostic
    ?? controller?.lastAttempt?.diagnostic
    ?? "";
  antennaControllerDiagnostic.textContent = controllerDiagnostic;
  antennaControllerDiagnostic.hidden = !controllerDiagnostic;
  const hasSavedAssociation = Boolean(controller?.profileId);
  antennaControllerAttach.hidden = controller?.policy !== "command_controlled" || !hasSavedAssociation || controller?.armed;
  antennaControllerAttach.disabled = controllerBusy;
  const automaticBusy = ["waiting", "running"].includes(controller?.automationStatus);
  const canRunController = controller?.armed && !automaticBusy && view.lifecycle === "running" && Boolean(view.nextIntent);
  antennaControllerRun.hidden = !canRunController;
  antennaControllerRun.disabled = controllerBusy;
  antennaControllerRetry.hidden = !canRunController || !controller?.lastAttempt;
  antennaControllerRetry.disabled = controllerBusy;
  antennaControllerEditor.hidden = !hasSavedAssociation;
  const activeProfile = state.antennaControllerCatalog?.profiles?.find(
    (profile) => profile.profileId === controller?.profileId,
  );
  const structuredControllerInput = state.antennaControllerCatalog?.inputStyle === "structured";
  antennaControllerOneLine.hidden = structuredControllerInput;
  antennaControllerStructured.hidden = !structuredControllerInput;
  if (activeProfile && antennaControllerEditor.dataset.profileRevision !== activeProfile.revision) {
    const set = (field, value) => {
      antennaControllerEditor.querySelector(`[data-active-controller-field="${field}"]`).value = value ?? "";
    };
    const commandLine = (command) => command
      ? [command.programTemplate, ...command.argumentTemplates].map((token) => JSON.stringify(token)).join(" ")
      : "";
    set("name", activeProfile.name);
    set("timeoutSeconds", activeProfile.timeoutSeconds);
    set("switchCommand", commandLine(activeProfile.switchCommand));
    set("verificationCommand", commandLine(activeProfile.verificationCommand));
    set("switchProgram", activeProfile.switchCommand.programTemplate);
    set("switchArguments", activeProfile.switchCommand.argumentTemplates.join("\n"));
    set("verificationProgram", activeProfile.verificationCommand?.programTemplate);
    set("verificationArguments", activeProfile.verificationCommand?.argumentTemplates?.join("\n"));
    antennaControllerEditor.dataset.profileRevision = activeProfile.revision;
  }
  const monotonicNow = options.monotonicNow ?? (() => Date.now());
  const nextAnchor = createCountdownAnchor(view, monotonicNow());
  const anchor = nextAnchor?.key === options.countdownKey ? options.countdownAnchor : nextAnchor;
  conductorLifecycle.textContent = humanizeIdentifier(view.lifecycle);
  conductorAntennaInUse.textContent = view.antennaInUse ?? "None";
  conductorPhase.textContent = humanizeIdentifier(view.phase);
  conductorGuidance.textContent = view.guidance;
  const projectedSeconds = state.conductorStatus === "ready"
    ? projectCountdown(anchor, monotonicNow())
    : view.secondsToTransition;
  conductorCountdown.textContent = projectedSeconds === null ? "" : formatCountdown(projectedSeconds);
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
  replaceSelectOptions(evidenceAntenna, view.antennas.map((antenna) => ({ value: antenna, label: antenna })));

  const evidenceAllowed = ["running", "interrupted"].includes(view.lifecycle);
  evidenceForm.querySelector("button[type=submit]").disabled = conductorBusy || !evidenceAllowed;
  const wsjtxBusy = ["refreshing", "starting", "stopping"].includes(state.wsjtxStatus);
  const wsjtxRunning = state.wsjtx?.phase === "running";
  lifecycleButtons.forEach((button) => {
    const action = button.dataset.conductorAction;
    if (action === "arm_wspr_cycle" && view.nextIntent) {
      const direction = view.nextIntent.direction ? humanizeIdentifier(view.nextIntent.direction) : null;
      button.textContent = direction
        ? `${direction} on ${view.nextIntent.antennaLabel} ready`
        : `${view.nextIntent.antennaLabel} ready`;
    }
    const available = conductorActionAvailable(view, action);
    button.hidden = !available;
    button.disabled = conductorBusy || !available
      || (["start", "resume"].includes(action) && readiness.visible && !readiness.acknowledged)
      || (action === "start" && view.wsjtxRequired && !wsjtxRunning);
  });
  skipCycleControl.hidden = !lifecycleButtons.some((button) =>
    button.dataset.conductorAction === "skip_wspr_cycle" && !button.hidden);
  conductorDiagnostics.replaceChildren(...view.diagnostics.map((diagnostic) => {
    const item = root.createElement("li");
    const code = root.createElement("strong");
    code.textContent = diagnostic.slotId ? `${diagnostic.code} · ${diagnostic.slotId}` : diagnostic.code;
    const message = root.createElement("span");
    message.textContent = diagnostic.message;
    item.append(code, message);
    return item;
  }));
  conductorDiagnostics.hidden = view.diagnostics.length === 0;
  conductorEvents.replaceChildren(...view.effectiveEvents.map((event) =>
    conductorEventElement(root, event, conductorBusy || !evidenceAllowed)));
  wsjtxForm.setAttribute("aria-busy", String(wsjtxBusy));
  wsjtxStart.disabled = conductorBusy || wsjtxBusy || wsjtxRunning || !["ready", "running"].includes(view.lifecycle);
  wsjtxStop.disabled = conductorBusy || wsjtxBusy || !wsjtxRunning;
  wsjtxRequirement.textContent = view.wsjtxRequired
    ? "Local/offline receive collection · required"
    : "Local/offline receive collection · optional";
  wsjtxPhase.textContent = state.wsjtx
    ? `${humanizeIdentifier(state.wsjtx.phase)}${state.wsjtx.bindAddress ? ` · ${state.wsjtx.bindAddress}` : ""}`
    : "Not started";
  wsjtxCounts.textContent = state.wsjtx
    ? `Direct/local active · ${state.wsjtx.receivedDatagrams} received · ${state.wsjtx.committedMutations} committed · ${state.wsjtx.ignoredDatagrams} explicit non-observation disposition(s)`
    : view.wsjtxRequired
      ? "Direct/local inactive · start this UDP receiver before the receive-capable session."
      : "Direct/local inactive · optional when delayed/public WSPR.live is enabled or the run has no receive periods.";
  const setupWarnings = state.wsjtx?.setupWarnings ?? [];
  wsjtxSetupWarnings.replaceChildren(...setupWarnings.map((warning) => {
    const item = root.createElement("li");
    item.dataset.code = warning.code;
    const title = root.createElement("strong");
    title.textContent = "Check WSJT-X setup";
    const message = root.createElement("span");
    message.textContent = warning.message;
    item.append(title, message);
    return item;
  }));
  wsjtxSetupWarnings.hidden = setupWarnings.length === 0;
  const adapterDiagnostic = state.wsjtxError ?? state.wsjtx?.diagnostic ?? null;
  wsjtxDiagnostic.hidden = adapterDiagnostic === null;
  if (adapterDiagnostic) {
    wsjtxDiagnostic.textContent = adapterDiagnostic.message ?? adapterDiagnostic.detail;
    if (adapterDiagnostic.code) wsjtxDiagnostic.textContent += ` (${adapterDiagnostic.code})`;
  }
  const acquisition = wsprLiveAcquisitionModel(state);
  wsprLiveCompact.textContent = acquisition.compact.text;
  wsprLiveCompact.dataset.kind = acquisition.compact.kind;
  wsprLivePhase.textContent = acquisition.phase;
  wsprLiveDetail.textContent = acquisition.detail;
  wsprLiveDiagnostic.hidden = acquisition.diagnostic.length === 0;
  wsprLiveDiagnostic.textContent = acquisition.diagnostic;
  wsprLiveRetry.hidden = !acquisition.retry;
  wsprLiveRetry.disabled = conductorBusy || state.wsprLiveAcquisitionStatus === "fetching";
  wsprLiveEndWithout.hidden = !acquisition.endWithout;
  wsprLiveEndWithout.disabled = conductorBusy || state.wsprLiveAcquisitionStatus === "fetching";
  return { anchor, key: anchor?.key ?? null };
}

export function renderTransfer(elements, state) {
  const {
    openButton, exportButton, importWsprLiveButton, importRbnButton, transferStatus,
    openFeedback, feedbackMessage, feedbackDetail, exportFeedback,
    exportFeedbackMessage, exportFeedbackDetail, importFeedback,
    importFeedbackMessage, importFeedbackDetail,
  } = elements;
  openButton.disabled = state.openStatus === "loading";
  openButton.textContent = state.openStatus === "loading" ? "Opening…" : "Choose bundle";
  const exportLoading = state.exportStatus === "loading";
  const importLoading = state.importStatus === "loading";
  exportButton.disabled = state.session === null || state.openStatus === "loading" || exportLoading;
  exportButton.textContent = state.session === null ? "Open a bundle first" : exportLoading ? "Exporting…" : "Export lossless bundle";
  importWsprLiveButton.disabled = state.session?.lifecycle !== "running" || importLoading;
  importWsprLiveButton.textContent = state.session?.lifecycle !== "running"
    ? "Open a running session first" : importLoading ? "Importing…" : "Choose WSPR.live JSON";
  const rbnEligible = state.session?.schemaVersion === 3
    && !["draft", "ready"].includes(state.session?.lifecycle);
  importRbnButton.disabled = !rbnEligible || importLoading;
  importRbnButton.textContent = state.session === null
    ? "Open a session first"
    : state.session.schemaVersion !== 3
      ? "This older session cannot import RBN evidence"
      : !rbnEligible ? "Start the session first" : importLoading ? "Importing…" : "Choose RBN ZIP";
  transferStatus.textContent = transferStatusText(state);
  transferStatus.classList.toggle("muted", state.openStatus !== "ready");
  renderFeedback(openFeedback, feedbackMessage, feedbackDetail, openFeedbackModel(state));
  renderFeedback(exportFeedback, exportFeedbackMessage, exportFeedbackDetail, exportFeedbackModel(state));
  renderFeedback(importFeedback, importFeedbackMessage, importFeedbackDetail, importFeedbackModel(state));
}

export function renderReport(elements, state, reportDocuments) {
  const {
    reportStatus, reportPlaceholder, reportViewer, reportFrame, reportRefreshButton,
    reportCompactExportButton, reportFullExportButton, reportFeedback, reportFeedbackMessage, reportFeedbackDetail,
    reportBundleName, reportRevision, reportSummary, reportControllerOptions,
    reportControllerHandling,
  } = elements;
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
  reportCompactExportButton.disabled = reportBusy || !hasReport;
  reportFullExportButton.disabled = reportBusy || !hasReport;
  const hasControllerEvidence = hasReport && state.session.hasControllerEvidence === true;
  reportControllerOptions.hidden = !hasControllerEvidence;
  if (!hasControllerEvidence) {
    reportControllerHandling.value = "complete";
    delete reportControllerOptions.dataset.presentationId;
  } else if (reportControllerOptions.dataset.presentationId !== String(state.reportPresentationId)) {
    reportControllerHandling.value = "complete";
    reportControllerOptions.dataset.presentationId = String(state.reportPresentationId);
  }
  reportRefreshButton.textContent = state.reportStatus === "refreshing" ? "Refreshing…" : "Refresh committed snapshot";
  reportCompactExportButton.textContent = state.reportExportStatus === "loading" ? "Exporting…" : "Export compact summary HTML";
  reportFullExportButton.textContent = state.reportExportStatus === "loading" ? "Exporting…" : "Export full evidence HTML";
  renderFeedback(reportFeedback, reportFeedbackMessage, reportFeedbackDetail, reportFeedbackModel(state));
  if (!hasSession) return;
  reportBundleName.textContent = state.session.bundleName;
  reportRevision.textContent = `Revision ${state.session.revision ?? "legacy"} · ${humanizeIdentifier(state.session.lifecycle ?? "static")}`;
  reportSummary.textContent = `${state.session.callsign} · ${state.session.grid} · ${state.session.antennaCount} antennas · ${state.session.slotCount} slots · ${state.session.observationCount} observations`;
  if (hasReport) updateReportFrame(reportFrame, state, reportDocuments);
}

function renderFeedback(container, message, detail, model) {
  container.hidden = model === null;
  if (!model) return;
  container.dataset.kind = model.kind;
  message.textContent = model.message;
  detail.textContent = model.detail;
  detail.hidden = model.detail.length === 0;
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
  if (["loading", "refreshing"].includes(state.conductorStatus)) return {
    kind: "loading", message: "Recovering the latest committed run state…", detail: "Rust owns timing, revision, and action eligibility.",
  };
  if (state.conductorStatus === "mutating") return {
    kind: "loading", message: `${conductorActionLabel(state.conductorPendingAction)}…`, detail: "The event and checkpoint are committed before the screen advances.",
  };
  if (state.conductorError) return { kind: "error", ...state.conductorError };
  if (state.conductorNotice) return { kind: "ready", message: state.conductorNotice, detail: "The active view reflects the committed checkpoint." };
  if (state.conductor?.phase === "interrupted") return { kind: "ready", message: "Session paused.", detail: "Resume when the station is ready, or end with the evidence already recorded." };
  if (state.conductor?.phase === "finalizing") return { kind: "loading", message: "Waiting for the final public reports…", detail: "Retry collection, or explicitly end without the final public spots." };
  if (["complete", "ended", "abandoned"].includes(state.conductor?.phase)) return { kind: "ready", message: "Session is terminal.", detail: "Existing evidence remains exportable and reportable." };
  return null;
}

function conductorActionLabel(action) {
  switch (action) {
    case "start": return "Starting session";
    case "resume": return "Resuming session";
    case "interrupt": return "Pausing session";
    case "end": return "Ending session";
    case "abandon": return "Abandoning session";
    case "arm_wspr_cycle": return "Starting WSPR cycle";
    case "skip_wspr_cycle": return "Skipping WSPR cycle";
    case "replace_event": return "Saving correction";
    case "retract_event": return "Saving retraction";
    default: return "Saving evidence";
  }
}

export function formatCountdown(seconds) {
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
    slot.direction ? humanizeIdentifier(slot.direction) : null,
    slot.band,
    slot.plannedAntenna,
    formatActiveRunTime(slot.startsAt, { now }),
  ]) {
    const item = root.createElement("span");
    if (value !== null) { item.textContent = value; timing.append(item); }
  }
  const actual = root.createElement("p");
  actual.className = "slot-evidence";
  actual.textContent = slot.actualAntenna
    ? `Actual: ${slot.actualAntenna}`
    : `Actual: not confirmed · ${humanizeIdentifier(slot.evidenceStatus)}`;
  container.append(title, timing, actual);
  if (slot.plannedSignal) {
    const planned = root.createElement("p");
    planned.textContent = `Planned signal: ${slot.plannedSignal.mode.toUpperCase()} · ${slot.plannedSignal.frequencyHz} Hz · ${slot.plannedSignal.transmittedCallsign}`;
    const actualSignal = root.createElement("p");
    actualSignal.textContent = slot.actualSignal
      ? `Actual signal: ${slot.actualSignal.mode?.toUpperCase() ?? "mode unconfirmed"} · ${slot.actualSignal.frequencyHz ?? "frequency unconfirmed"} Hz · ${slot.actualSignal.transmittedCallsign ?? "callsign unconfirmed"} · ${humanizeIdentifier(slot.signalStatus)}`
      : `Actual signal: not confirmed · ${humanizeIdentifier(slot.signalStatus)}`;
    container.append(planned, actualSignal);
  }
}

function renderIntent(container, intent, root) {
  container.replaceChildren();
  if (!intent) {
    const empty = root.createElement("p"); empty.className = "muted-copy"; empty.textContent = "None"; container.append(empty); return;
  }
  const title = root.createElement("strong");
  title.textContent = `#${intent.sequenceNumber} · ${intent.direction ? `${humanizeIdentifier(intent.direction)} on ` : ""}${intent.antennaLabel}`;
  const band = root.createElement("p"); band.textContent = intent.band;
  const timing = root.createElement("p"); timing.textContent = "Timing will be set after you confirm the antenna is ready.";
  container.append(title, band, timing);
}

function replaceSelectOptions(select, options) {
  const signature = JSON.stringify(options);
  if (select.dataset.options === signature) return;
  const selected = select.value;
  select.replaceChildren(...options.map(({ value, label }) => {
    const option = select.ownerDocument.createElement("option");
    option.value = value; option.textContent = label; return option;
  }));
  select.dataset.options = signature;
  if (options.some(({ value }) => value === selected)) select.value = selected;
}

function conductorEventElement(root, event, disabled) {
  const article = root.createElement("article");
  const context = root.createElement("div");
  const kind = root.createElement("span");
  kind.textContent = event.slotId ? `${humanizeIdentifier(event.kind)} · ${event.slotId}` : humanizeIdentifier(event.kind);
  const summary = root.createElement("strong"); summary.textContent = event.summary;
  const time = root.createElement("small"); time.textContent = formatReviewTime(event.occurredAt);
  context.append(kind, summary, time);
  const actions = root.createElement("div");
  for (const [action, label] of [["replace", "Replace"], ["retract", "Retract"]]) {
    const button = root.createElement("button");
    button.type = "button"; button.dataset.eventAction = action; button.dataset.eventId = event.sourceEventId;
    button.textContent = label; button.disabled = disabled; actions.append(button);
  }
  article.append(context, actions);
  return article;
}

function transferStatusText(state) {
  if (state.importStatus === "loading") return "Importing evidence";
  if (state.importStatus === "error") return "Import failed";
  if (state.openStatus === "loading") {
    return state.openSource === "managed" ? "Opening saved session" : "Opening bundle";
  }
  if (state.openStatus === "ready") return "Bundle open";
  if (state.openStatus === "error") return "Open failed";
  return "No bundle open";
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
  if (state.setupStatus === "reviewing") return { kind: "loading", message: "Checking the plan…", detail: "No destination is created during review." };
  if (state.setupStatus === "creating") return { kind: "loading", message: "Creating and reopening the checkpointed session…", detail: "The destination is published only after complete verification." };
  if (state.setupError) return { kind: "error", ...state.setupError };
  if (state.setupStatus === "invalid") return { kind: "error", message: "The plan needs changes before it can be created.", detail: "Correct the highlighted fields, then review again." };
  if (state.setupNotice === "cancelled") return { kind: "cancelled", message: "Creation cancelled.", detail: "The reviewed plan remains ready and no destination was changed." };
  if (state.setupNotice === "created" && state.session) return { kind: "ready", message: `${state.session.bundleName} is the active session.`, detail: `Checkpoint revision 0 is ready with ${state.session.slotCount} planned slots.` };
  if (state.setupStatus === "reviewed") return { kind: "ready", message: "The plan passed strict creation validation.", detail: "Review the exact UTC-backed schedule, then create the session." };
  return null;
}

function openFeedbackModel(state) {
  if (state.openStatus === "loading") return state.openSource === "managed"
    ? { kind: "loading", message: "Opening the saved session…", detail: "Its current lifecycle will be checked before choosing the destination." }
    : { kind: "loading", message: "Reading and validating the selected bundle…", detail: "The source directory will not be changed." };
  if (state.error) return { kind: "error", ...state.error };
  if (state.notice === "cancelled") return { kind: "cancelled", message: "Open cancelled.", detail: "No session was changed." };
  if (state.notice === "work_redirected" && state.session) return { kind: "ready", message: `${state.session.bundleName} opened in Reports.`, detail: "Its current lifecycle is terminal or read-only, so run services were not loaded." };
  if (state.session) return { kind: "ready", message: `${state.session.bundleName} is ready.`, detail: "Its local report was rebuilt in memory from the source bundle." };
  return null;
}

function exportFeedbackModel(state) {
  if (state.exportStatus === "loading") return { kind: "loading", message: "Copying and verifying the active bundle…", detail: "Original durable files and attachments are preserved byte-for-byte." };
  if (state.exportError) return { kind: "error", ...state.exportError };
  if (state.exportNotice === "cancelled") return { kind: "cancelled", message: "Export cancelled.", detail: "The active session was not changed." };
  if (state.exportedBundleName) return { kind: "ready", message: `${state.exportedBundleName} was exported and verified.`, detail: "The original bundle remains the active session." };
  return null;
}

function importFeedbackModel(state) {
  if (state.importStatus === "loading") {
    const rbn = state.importKind === "rbn";
    return { kind: "loading", message: rbn ? "Validating and committing RBN archive evidence…" : "Validating and committing WSPR.live evidence…", detail: rbn ? "The exact ZIP, bounded row dispositions, and public reports commit under one checkpoint." : "The exact response and its bounded row dispositions commit under one checkpoint." };
  }
  if (state.importError) return { kind: "error", ...state.importError };
  if (state.importNotice === "cancelled") return { kind: "cancelled", message: `${state.importKind === "rbn" ? "RBN archive" : "WSPR.live"} import cancelled.`, detail: "The active session was not changed." };
  if (!state.importNotice) return null;
  const result = state.importNotice;
  const omitted = result.omitted ? `, ${result.omitted} omitted by the retention bound` : "";
  return { kind: "ready", message: `${result.observationsCreated} imported spot observation(s) committed at revision ${result.revision}.`, detail: `${result.total} rows: ${result.accepted} accepted, ${result.filtered} filtered, ${result.malformed} malformed, ${result.unsupported} unsupported, ${result.duplicate} duplicate, ${result.conflict} conflict${omitted}. AntennaBench retained the rows returned by this WSPR.live response; the upstream mirror does not provide an independent completeness guarantee.` };
}

function reportFeedbackModel(state) {
  if (state.reportStatus === "refreshing") return { kind: "loading", message: "Building one verified committed snapshot…", detail: "The prior coherent report remains visible until the new revision is verified." };
  if (state.reportExportStatus === "loading") return { kind: "loading", message: "Exporting the committed HTML snapshot…", detail: "Choose compact summary HTML for printing/lightweight sharing or full evidence HTML for the complete audit report. The destination is created without overwriting an existing file." };
  if (state.reportExportError) return { kind: "error", ...state.reportExportError };
  if (state.reportError) return { kind: "error", ...state.reportError };
  if (state.reportExportNotice === "cancelled") return { kind: "cancelled", message: "Report export cancelled.", detail: "The visible coherent report was retained." };
  if (state.reportExportNotice) return { kind: "ready", message: "The committed report artifact was exported.", detail: state.reportExportNotice };
  return null;
}

function humanizeIdentifier(value) {
  return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function formatReviewTime(value) {
  const instant = new Date(value);
  return `${new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "medium" }).format(instant)} · ${instant.toISOString()}`;
}
