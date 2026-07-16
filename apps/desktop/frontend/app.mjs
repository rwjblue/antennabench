import { createDesktopController } from "./controller.mjs";
import {
  applyStationPreferences,
  readEvidenceAction,
  readEvidenceReplacement,
  readSetupDraft,
  readSignalEvidenceFields,
} from "./forms.mjs";
import {
  conductorActionAvailable,
  createCountdownAnchor,
  formatActiveRunTime,
  installContextualHelp,
  locationLookupMessage,
  maidenheadGrid,
  recommendedNoteTarget,
  updateReportFrame,
  viewModel,
  workflowFromHash,
  wsprLiveAcquisitionModel,
  wsprRunPlanSummary,
} from "./models.mjs";
import { initialState } from "./state.mjs";

function mount(root, browserWindow) {
  let state = initialState(workflowFromHash(browserWindow.location.hash));
  let countdownAnchor = null;
  let countdownAnchorKey = null;
  let noteShortcutInitialized = false;
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
  const conductorLifecycle = root.querySelector("[data-conductor-lifecycle]");
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
  const wsjtxRequirement = root.querySelector("[data-wsjtx-requirement]");
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
  installContextualHelp(root);

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
      }
      conductorLifecycle.textContent = humanizeIdentifier(view.lifecycle);
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
      const wsjtxBusy = ["refreshing", "starting", "stopping"].includes(state.wsjtxStatus);
      const wsjtxRunning = state.wsjtx?.phase === "running";
      lifecycleButtons.forEach((button) => {
        const action = button.dataset.conductorAction;
        const isArmAction = action === "arm_wspr_cycle";
        if (isArmAction && view.nextIntent) {
          const direction = view.nextIntent.direction
            ? humanizeIdentifier(view.nextIntent.direction)
            : null;
          button.textContent = direction
            ? `${direction} on ${view.nextIntent.antennaLabel} ready`
            : `${view.nextIntent.antennaLabel} ready`;
        }
        const available = conductorActionAvailable(view, action);
        button.hidden = !available;
        button.disabled = conductorBusy
          || !available
          || (action === "start" && view.wsjtxRequired && !wsjtxRunning);
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
      wsjtxForm.setAttribute("aria-busy", String(wsjtxBusy));
      wsjtxStart.disabled = conductorBusy || wsjtxBusy || wsjtxRunning || !["ready", "running"].includes(view.lifecycle);
      wsjtxStop.disabled = conductorBusy || wsjtxBusy || !wsjtxRunning;
      wsjtxRequirement.textContent = view.wsjtxRequired
        ? "Required WSJT-X receiver"
        : "Optional WSJT-X receiver";
      wsjtxPhase.textContent = state.wsjtx
        ? `${humanizeIdentifier(state.wsjtx.phase)}${state.wsjtx.bindAddress ? ` · ${state.wsjtx.bindAddress}` : ""}`
        : "Not started";
      wsjtxCounts.textContent = state.wsjtx
        ? `${state.wsjtx.receivedDatagrams} received · ${state.wsjtx.committedMutations} committed · ${state.wsjtx.ignoredDatagrams} explicit non-observation disposition(s)`
        : view.wsjtxRequired
          ? "Start this UDP receiver before starting the session."
          : "TX-only manual operation remains available without WSJT-X.";
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
      root.ownerDocument.addEventListener?.("visibilitychange", callback);
      return () => root.ownerDocument.removeEventListener?.("visibilitychange", callback);
    },
    onHashChange(callback) {
      const listener = () => callback(workflowFromHash(browserWindow.location.hash));
      browserWindow.addEventListener("hashchange", listener);
      return () => browserWindow.removeEventListener?.("hashchange", listener);
    },
    isVisible: () => root.ownerDocument.visibilityState !== "hidden",
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
      root.querySelector("main").focus({ preventScroll: true });
    });
  }

  conductorRefreshButtons.forEach((button) => {
    button.addEventListener("click", () => controller.refreshConductor());
  });

  wsprLiveRetry.addEventListener("click", () => controller.advanceWsprLive(true));
  wsprLiveEndWithout.addEventListener("click", async () => {
    if (!controller.confirm("End this session without the final automatic WSPR.live capture? Existing evidence will remain.")) return;
    await controller.submitConductorAction({
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

  setupForm.addEventListener("input", (event) => {
    if (event.target.matches?.('[data-setup-field="callsign"], [data-setup-field="signalTransmittedCallsign"]')) {
      event.target.value = event.target.value.toUpperCase();
    }
    syncSignalPlanFields(setupForm);
    updateWsprRunPlanSummary(setupForm, setupRunPlanSummary);
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
    await controller.reviewSetup(readSetupDraft(setupForm));
  });

  setupCreateButton.addEventListener("click", async () => {
    await controller.createSession();
  });

  setupAddAntennaButton.addEventListener("click", () => {
    const fragment = setupAntennaTemplate.content.cloneNode(true);
    setupAddAntennaButton.before(fragment);
    refreshAntennaRows(setupForm);
    updateWsprRunPlanSummary(setupForm, setupRunPlanSummary);
    controller.editSetup();
  });

  setupForm.addEventListener("click", (event) => {
    const removeButton = event.target.closest("[data-remove-antenna]");
    if (!removeButton) return;
    const rows = setupForm.querySelectorAll("[data-antenna-row]");
    if (rows.length <= 1) return;
    removeButton.closest("[data-antenna-row]").remove();
    refreshAntennaRows(setupForm);
    updateWsprRunPlanSummary(setupForm, setupRunPlanSummary);
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
  reportExportButton.addEventListener("click", async () => {
    await controller.exportReport();
  });

  syncSignalPlanFields(setupForm);
  updateWsprRunPlanSummary(setupForm, setupRunPlanSummary);
  render();
  if (typeof browserWindow.__TAURI__?.core?.invoke === "function") {
    void controller.loadStationPreferences()
      .then((preferences) => applyStationPreferences(setupForm, preferences))
      .catch(() => {});
  }
  controller.start();
  return controller;
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
      detail: `${recovery.artifactCount} recovery artifact(s) were preserved.`,
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
    slot.direction ? humanizeIdentifier(slot.direction) : null,
    slot.band,
    slot.plannedAntenna,
    formatActiveRunTime(slot.startsAt, { now }),
  ]) {
    const item = root.createElement("span");
    if (value !== null) {
      item.textContent = value;
      timing.append(item);
    }
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
  title.textContent = `#${intent.sequenceNumber} · ${intent.direction ? `${humanizeIdentifier(intent.direction)} on ` : ""}${intent.antennaLabel}`;
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

function syncSignalPlanFields(form) {
  const enabled = form.querySelector('[data-setup-field="signalPlanEnabled"]').checked;
  const receiveOnly = form.querySelector('[data-setup-field="mode"]').value === "rx_focused";
  const fields = form.querySelector("[data-signal-plan-fields]");
  fields.hidden = !enabled;
  for (const control of fields.querySelectorAll("input, select, textarea")) {
    control.disabled = !enabled;
  }
  const wsprLive = form.querySelector('[data-setup-field="wsprLiveAcquisitionEnabled"]');
  if (enabled || receiveOnly) wsprLive.checked = false;
  wsprLive.disabled = enabled || receiveOnly;
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
    form.querySelector('[data-setup-field="mode"]').value,
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
