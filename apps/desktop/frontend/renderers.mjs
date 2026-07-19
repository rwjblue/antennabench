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

export function renderSavedSessions(elements, state, root) {
  const {
    savedStatus, savedRefresh, savedRevealFolder, savedFeedback, savedFeedbackMessage,
    savedFeedbackDetail, savedEmpty, savedCatalog,
  } = elements;
  const loading = ["loading", "refreshing"].includes(state.catalogStatus);
  const entries = state.managedCatalog?.entries ?? [];
  savedStatus.textContent = state.catalogStatus === "loading"
    ? "Loading"
    : state.catalogStatus === "refreshing"
      ? "Refreshing"
      : state.managedCatalog?.status === "incomplete"
        ? "Partial list"
        : state.catalogStatus === "error" && !state.managedCatalog
          ? "Unavailable"
          : `${entries.length} saved`;
  savedStatus.classList.toggle("muted", state.catalogStatus !== "ready");
  savedRefresh.disabled = loading;
  savedRefresh.textContent = loading ? "Refreshing…" : "Refresh";
  savedRevealFolder.disabled = state.catalogRowOperation !== null;
  savedEmpty.hidden = !(
    state.catalogStatus === "ready"
    && state.managedCatalog?.status === "complete"
    && entries.length === 0
  );

  const catalogMessage = state.catalogError
    ? { kind: "error", ...state.catalogError }
    : state.managedCatalog?.status === "incomplete"
      ? {
        kind: "error",
        message: "Only part of Saved sessions could be inspected.",
        detail: state.managedCatalog.diagnostics?.map((item) => item.message).join(" ")
          || "Refresh after reducing the number or size of saved session entries.",
      }
      : openFeedbackModel(state);
  renderFeedback(savedFeedback, savedFeedbackMessage, savedFeedbackDetail, catalogMessage);

  savedCatalog.setAttribute("aria-busy", String(loading));
  savedCatalog.replaceChildren(...entries.map((entry) => renderSavedRow(root, state, entry)));
}

function renderSavedRow(root, state, entry) {
  const row = root.createElement("article");
  row.className = `saved-row saved-row-${entry.status}`;
  row.dataset.locatorId = entry.locatorId ?? "";
  const heading = root.createElement("div");
  heading.className = "saved-row-heading";
  const identity = root.createElement("div");
  const callsign = root.createElement("h2");
  callsign.textContent = entry.callsign || entry.bundleName;
  const created = root.createElement("p");
  created.textContent = entry.createdAt
    ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(entry.createdAt))
    : "Creation time unavailable";
  identity.append(callsign, created);
  const lifecycle = root.createElement("span");
  lifecycle.className = "status-chip saved-lifecycle";
  lifecycle.textContent = managedStatusLabel(entry);
  heading.append(identity, lifecycle);

  const summary = root.createElement("p");
  summary.className = "saved-plan-summary";
  const bands = (entry.bands ?? []).map(formatBand).join(", ") || "Band unavailable";
  const mode = entry.mode ? humanizeIdentifier(entry.mode).replace("Ab", "A/B") : "Plan unavailable";
  const repetitions = formatCount(entry.plannedRepetitions, "planned repetition");
  const direction = formatDirectionCoverage(entry.directionCoverage);
  const cycles = formatCount(entry.plannedCycleCount, "planned cycle");
  const observations = formatCount(entry.observationCounts?.total, "recorded observation");
  summary.textContent = `${repetitions} · ${direction} · ${cycles} · ${observations}`;
  const meta = root.createElement("p");
  meta.className = "saved-row-meta";
  meta.textContent = `${entry.originLabel ?? "Saved by AntennaBench"} · ${entry.bundleName}`;

  const duplicate = root.createElement("p");
  duplicate.className = "saved-warning";
  duplicate.textContent = "Another saved bundle has the same session identity.";
  duplicate.hidden = !(entry.sameSessionIdCount > 1);

  const details = root.createElement("details");
  details.className = "saved-row-details";
  const detailsSummary = root.createElement("summary");
  detailsSummary.textContent = "Experiment and evidence";
  const detailList = root.createElement("dl");
  for (const [label, value] of [
    ["Plan", mode],
    ["Antennas", (entry.antennaLabels ?? []).join(", ") || "Unavailable"],
    ["Bands", bands],
    ["Local decodes", formatCount(entry.observationCounts?.localDecodes, "recorded observation")],
    ["Public spots", formatCount(entry.observationCounts?.publicSpots, "recorded observation")],
    ["Imported spots", formatCount(entry.observationCounts?.importedSpots, "recorded observation")],
  ]) {
    const group = root.createElement("div");
    const term = root.createElement("dt"); term.textContent = label;
    const description = root.createElement("dd"); description.textContent = String(value);
    group.append(term, description); detailList.append(group);
  }
  const warning = root.createElement("p");
  warning.textContent = "This portable directory is the session record. Use AntennaBench actions instead of editing its files by hand.";
  const problems = root.createElement("ul");
  for (const problem of entry.problems ?? []) {
    const item = root.createElement("li"); item.textContent = problem.message; problems.append(item);
  }
  problems.hidden = problems.childElementCount === 0;
  const technical = root.createElement("details");
  technical.className = "saved-technical-details";
  const technicalSummary = root.createElement("summary");
  technicalSummary.textContent = "Technical details";
  const technicalList = root.createElement("dl");
  for (const [label, value] of [
    ["Schema", entry.schemaVersion ?? "Unavailable"],
    ["Committed revision", entry.revision ?? "Legacy / unavailable"],
  ]) {
    const group = root.createElement("div");
    const term = root.createElement("dt"); term.textContent = label;
    const description = root.createElement("dd"); description.textContent = String(value);
    group.append(term, description); technicalList.append(group);
  }
  technical.append(technicalSummary, technicalList);
  details.append(detailsSummary, detailList, technical, problems, warning);

  const actions = root.createElement("div");
  actions.className = "saved-row-actions";
  const action = managedOpenAction(entry);
  if (action) actions.append(savedActionButton(root, action.label, "open", entry.locatorId, action.intent, true));
  if (action?.intent === "work") {
    actions.append(savedActionButton(root, "View report", "open", entry.locatorId, "report"));
  }
  if (!action) actions.append(savedActionButton(root, "View details", "details", entry.locatorId, null, true));
  if (entry.locatorId) actions.append(savedActionButton(root, "Reveal in Finder", "reveal", entry.locatorId));
  for (const button of actions.querySelectorAll("button")) {
    button.disabled = state.catalogRowOperation !== null;
  }
  if (state.activeManagedLocatorId === entry.locatorId) {
    const active = root.createElement("span"); active.className = "saved-open-now"; active.textContent = "Open now";
    actions.prepend(active);
  }
  if (state.catalogRowError?.locatorId === entry.locatorId) {
    const error = root.createElement("p"); error.className = "saved-row-error";
    error.textContent = `${state.catalogRowError.error.message} ${state.catalogRowError.error.detail}`.trim();
    row.append(heading, summary, meta, duplicate, details, actions, error);
  } else {
    row.append(heading, summary, meta, duplicate, details, actions);
  }
  return row;
}

function savedActionButton(root, label, action, locatorId, intent = null, primary = false) {
  const button = root.createElement("button");
  button.type = "button"; button.textContent = label; button.dataset.savedAction = action;
  button.dataset.locatorId = locatorId ?? "";
  if (intent) button.dataset.intent = intent;
  if (primary) button.className = "primary-action";
  return button;
}

function managedOpenAction(entry) {
  if (entry.status !== "available" || entry.lifecycle === "draft" || !entry.locatorId) return null;
  switch (entry.lifecycle) {
    case "ready": return { label: "Start session", intent: "work" };
    case "running": return { label: "Continue session", intent: "work" };
    case "interrupted": return { label: "Resume session", intent: "work" };
    default: return { label: "View report", intent: "report" };
  }
}

function managedStatusLabel(entry) {
  if (entry.status !== "available") return ({
    invalid: "Invalid session bundle",
    unsupported: "Unsupported version",
    unreadable: "Could not read",
    unsafe: "Unsafe filesystem entry",
  })[entry.status] ?? "Session problem";
  return humanizeIdentifier(entry.lifecycle ?? "legacy report");
}

function formatBand(band) {
  return String(band).replace(/^(\d+)(m|cm)$/i, "$1 $2");
}

function formatCount(value, singular) {
  if (value === null || value === undefined) return "Unavailable";
  return `${value} ${value === 1 ? singular : `${singular}s`}`;
}

function formatDirectionCoverage(value) {
  return ({
    transmit_only: "TX only",
    receive_only: "RX only",
    transmit_and_receive: "TX + RX",
    unknown: "Direction unknown",
  })[value] ?? "Direction unavailable";
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
    wsprLiveAcquisitionEnabled: setupForm.querySelector('[data-setup-field="wsprLiveAcquisitionEnabled"]').checked,
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
    managedLocationNotice, managedLocationMessage, managedLocationDetail,
    managedLocationReveal,
    runHistoricalDiagnostic, runHistoricalTitle, runHistoricalSummary, runHistoricalMeta,
  } = elements;
  managedLocationNotice.hidden = state.managedLocationNotice === null;
  if (state.managedLocationNotice) {
    managedLocationMessage.textContent = "Session saved in AntennaBench Sessions.";
    managedLocationDetail.textContent = state.managedLocationNotice.bundleName;
    managedLocationReveal.disabled = state.catalogRowOperation !== null;
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
  renderFeedback(
    conductorFeedback,
    conductorFeedbackMessage,
    conductorFeedbackDetail,
    conductorFeedbackModel(state),
  );
  renderReopenedHistoricalDiagnostic(
    { runHistoricalDiagnostic, runHistoricalTitle, runHistoricalSummary, runHistoricalMeta },
    state,
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
    exportButton, importWsprLiveButton, importRbnButton, transferStatus, exportFeedback,
    exportFeedbackMessage, exportFeedbackDetail, importFeedback,
    importFeedbackMessage, importFeedbackDetail,
  } = elements;
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
  transferStatus.classList.toggle("muted", state.session === null);
  renderFeedback(exportFeedback, exportFeedbackMessage, exportFeedbackDetail, exportFeedbackModel(state));
  renderFeedback(importFeedback, importFeedbackMessage, importFeedbackDetail, importFeedbackModel(state));
}

export function renderReport(elements, state, reportDocuments) {
  const {
    reportStatus, reportPlaceholder, reportViewer, reportFrame, reportRefreshButton,
    reportCompactExportButton, reportFullExportButton, reportFeedback, reportFeedbackMessage, reportFeedbackDetail,
    reportBundleName, reportRevision, reportSummary, reportControllerOptions,
    reportControllerHandling,
    reportOperationalOptions, reportOperationalHandling,
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
  reportOperationalOptions.hidden = !hasSession;
  if (reportOperationalOptions.dataset.presentationId !== String(state.reportPresentationId)) {
    reportOperationalHandling.value = "omitted";
    reportOperationalOptions.dataset.presentationId = String(state.reportPresentationId);
  }
  reportRefreshButton.textContent = state.reportStatus === "refreshing" ? "Refreshing…" : "Refresh committed snapshot";
  reportCompactExportButton.textContent = state.reportExportStatus === "loading" ? "Exporting…" : "Export compact summary HTML";
  reportFullExportButton.textContent = state.reportExportStatus === "loading" ? "Exporting…" : "Export full evidence HTML";
  renderFeedback(reportFeedback, reportFeedbackMessage, reportFeedbackDetail, reportFeedbackModel(state));
  renderOperationalHistory(elements, state);
  if (!hasSession) return;
  reportBundleName.textContent = state.session.bundleName;
  reportRevision.textContent = `Revision ${state.session.revision ?? "legacy"} · ${humanizeIdentifier(state.session.lifecycle ?? "static")}`;
  reportSummary.textContent = `${state.session.callsign} · ${state.session.grid} · ${state.session.antennaCount} antennas · ${state.session.slotCount} slots · ${state.session.observationCount} observations`;
  if (hasReport) updateReportFrame(reportFrame, state, reportDocuments);
}

function renderReopenedHistoricalDiagnostic(elements, state) {
  const { runHistoricalDiagnostic, runHistoricalTitle, runHistoricalSummary, runHistoricalMeta } = elements;
  const running = ["running", "interrupted"].includes(state.session?.lifecycle);
  const diagnostics = state.session?.operationalHistory?.diagnostics ?? [];
  const relevant = diagnostics.findLast?.((diagnostic) => (
    ["failed", "partial", "unknown"].includes(diagnostic.outcome)
    || diagnostic.severity === "error"
  )) ?? [...diagnostics].reverse().find((diagnostic) => (
    ["failed", "partial", "unknown"].includes(diagnostic.outcome)
    || diagnostic.severity === "error"
  ));
  runHistoricalDiagnostic.hidden = !running || !relevant;
  if (!running || !relevant) return;
  runHistoricalTitle.textContent = relevant.outcome === "partial"
    ? "Earlier operation retained only part of its intended result"
    : "Earlier operation needs attention";
  runHistoricalSummary.textContent = relevant.summary;
  runHistoricalMeta.textContent = [
    relevant.code,
    `${humanizeIdentifier(relevant.operation)} / ${humanizeIdentifier(relevant.phase)}`,
    humanizeIdentifier(relevant.evidenceEffect),
    relevant.occurredAt,
  ].join(" · ");
}

function renderOperationalHistory(elements, state) {
  const {
    operationalHistory, operationalHistoryStatus, operationalHistoryMessage,
    operationalHistoryContexts, operationalHistoryDiagnostics, operationalHistoryBounds,
    copySupportSummary, copySupportStatus,
  } = elements;
  const history = state.session?.operationalHistory;
  operationalHistory.hidden = state.session === null;
  if (!state.session) return;
  const historyState = history?.historyState ?? "unavailable";
  operationalHistory.dataset.state = historyState;
  operationalHistoryStatus.textContent = humanizeIdentifier(historyState);
  operationalHistoryStatus.classList.toggle("muted", historyState !== "complete");
  operationalHistoryMessage.textContent = operationalHistoryMessageFor(historyState, history);

  const document = operationalHistory.ownerDocument;
  operationalHistoryContexts.replaceChildren(...(history?.contexts ?? []).map((context) => {
    const article = document.createElement("article");
    const title = document.createElement("h3");
    title.textContent = context.creator ? "Creator runtime" : "Subsequent runtime";
    const build = document.createElement("p");
    build.textContent = [
      context.appVersion ?? "version unknown",
      context.sourceCommit ? `SHA ${context.sourceCommit}` : "SHA unknown",
      humanizeIdentifier(context.buildChannel ?? "unknown"),
      humanizeIdentifier(context.sourceState ?? "unknown"),
    ].join(" · ");
    const platform = document.createElement("p");
    platform.textContent = [
      context.targetTriple,
      context.osFamily,
      context.osVersion,
      context.runtimeArchitecture,
    ].filter(Boolean).join(" · ") || "Platform unknown";
    const recorded = document.createElement("p");
    recorded.textContent = `${context.firstRecordedAt} · ${context.contextId}`;
    article.append(title, build, platform, recorded);
    return article;
  }));

  operationalHistoryDiagnostics.replaceChildren(...(history?.diagnostics ?? []).map((diagnostic) => {
    const item = document.createElement("li");
    item.dataset.outcome = diagnostic.outcome;
    const title = document.createElement("h3");
    title.textContent = `${diagnostic.code} · ${humanizeIdentifier(diagnostic.outcome)}`;
    const summary = document.createElement("p");
    summary.textContent = diagnostic.summary;
    const operation = document.createElement("p");
    operation.textContent = `${diagnostic.occurredAt} · ${humanizeIdentifier(diagnostic.operation)} / ${humanizeIdentifier(diagnostic.phase)} · context ${diagnostic.runtimeContextId}`;
    const effect = document.createElement("p");
    effect.textContent = `Evidence: ${humanizeIdentifier(diagnostic.evidenceEffect)} · revisions ${diagnostic.revisionBefore ?? "unknown"} → ${diagnostic.revisionAfter ?? "unchanged/unknown"} · retry: ${humanizeIdentifier(diagnostic.retryDisposition)} (${diagnostic.retryGuidanceCode})`;
    item.append(title, summary, operation, effect);
    for (const detail of [...(diagnostic.targets ?? []), ...(diagnostic.causes ?? [])]) {
      const line = document.createElement("p");
      const code = document.createElement("code");
      code.textContent = detail;
      line.append(code);
      item.append(line);
    }
    if (diagnostic.detailTruncated) {
      const truncated = document.createElement("p");
      truncated.textContent = "This diagnostic reached its detail bound; some optional facts were omitted.";
      item.append(truncated);
    }
    return item;
  }));

  if ((history?.diagnostics ?? []).length === 0) {
    const empty = document.createElement("li");
    empty.dataset.outcome = "none";
    empty.textContent = historyState === "complete"
      ? "No material operational diagnostics were recorded within this format's storage and process guarantees."
      : "No diagnostic records are available for this history state.";
    operationalHistoryDiagnostics.replaceChildren(empty);
  }
  const omitted = (history?.retentionOmittedCount ?? 0)
    + (history?.presentationOmittedCount ?? 0)
    + (history?.supportSummaryOmittedCount ?? 0);
  operationalHistoryBounds.textContent = `Retained ${history?.retainedCount ?? 0} diagnostic records. Format bounds: ${history?.recordLimit ?? "unknown"} records / ${formatBytes(history?.byteLimit)}. This view shows the latest ${history?.diagnostics?.length ?? 0}; ${omitted} later, earlier, or support-summary records are explicitly reported as omitted. ${history?.contextOmittedCount ?? 0} runtime contexts are omitted from this view.`;
  copySupportSummary.disabled = typeof history?.supportSummary !== "string"
    || state.supportCopyStatus === "copying";
  copySupportSummary.textContent = state.supportCopyStatus === "copying"
    ? "Copying…"
    : "Copy support summary";
  copySupportStatus.textContent = state.supportCopyStatus === "copied"
    ? "Copied redacted JSON."
    : state.supportCopyStatus === "error"
      ? state.supportCopyError?.message ?? "Copy failed."
      : "";
}

function operationalHistoryMessageFor(state, history) {
  switch (state) {
    case "complete": return history?.diagnostics?.length
      ? "Chronological material failures, partial outcomes, and recoveries retained in this bundle are shown below."
      : "This schema-v6 checkpoint has no recorded material diagnostics within the format's guarantees; storage or process loss can still prevent a diagnostic write.";
    case "legacy_unknown": return "This legacy bundle predates durable runtime and diagnostic streams. Earlier operational history is unknown, not clean.";
    case "retention_capped": return "The append-only diagnostic stream reached its retention bound. Later outcomes may be absent.";
    case "persistence_gap": return `A diagnostic write could not be durably recorded. History has a known gap${history?.reasonCode ? ` (${history.reasonCode})` : ""}.`;
    default: return "Operational history is unavailable. AntennaBench cannot infer whether material failures occurred.";
  }
}

function formatBytes(value) {
  if (!Number.isFinite(value)) return "unknown bytes";
  return `${value} bytes`;
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
  return state.session ? "Session active" : "No active session";
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
