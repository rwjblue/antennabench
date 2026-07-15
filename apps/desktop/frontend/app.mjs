export const WORKFLOWS = Object.freeze(["setup", "run", "transfer", "report"]);

export function initialState(workflow = "setup") {
  return selectWorkflow(
    {
      activeWorkflow: "setup",
      openStatus: "idle",
      session: null,
      reportPresentationId: 0,
      error: null,
      notice: null,
      exportStatus: "idle",
      exportError: null,
      exportNotice: null,
      exportedBundleName: null,
      setupStatus: "editing",
      setupReview: null,
      setupError: null,
      setupNotice: null,
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
    setupStatus: "created",
    setupError: null,
    setupNotice: "created",
    openStatus: "ready",
    session,
    reportPresentationId: state.reportPresentationId + 1,
    error: null,
    notice: null,
    exportStatus: "idle",
    exportError: null,
    exportNotice: null,
    exportedBundleName: null,
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
    reportPresentationId: state.reportPresentationId + 1,
    error: null,
    notice: null,
    exportStatus: "idle",
    exportError: null,
    exportNotice: null,
    exportedBundleName: null,
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

export function invokeOpenSession(invoke) {
  return invoke("open_session_bundle");
}

export function invokeReviewSessionSetup(invoke, draft) {
  return invoke("review_session_setup", { draft });
}

export function invokeCreateSessionFromReview(invoke, reviewId) {
  return invoke("create_session_from_review", { reviewId });
}

export function invokeActiveSessionReport(invoke) {
  return invoke("active_session_report");
}

export function invokeExportSession(invoke) {
  return invoke("export_active_session");
}

export function updateReportFrame(reportFrame, state) {
  if (state.session === null) return false;

  const presentationId = String(state.reportPresentationId);
  if (reportFrame.dataset.presentationId === presentationId) return false;

  reportFrame.srcdoc = state.session.reportHtml;
  reportFrame.dataset.presentationId = presentationId;
  return true;
}

function mount(root, browserWindow) {
  let state = initialState(workflowFromHash(browserWindow.location.hash));
  const navigation = [...root.querySelectorAll("[data-workflow]")];
  const panels = [...root.querySelectorAll("[data-panel]")];
  const setupForm = root.querySelector("[data-setup-form]");
  const setupStatus = root.querySelector("[data-setup-status]");
  const setupReviewButton = root.querySelector("[data-review-setup]");
  const setupCreateButton = root.querySelector("[data-create-session]");
  const setupAddAntennaButton = root.querySelector("[data-add-antenna]");
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
  const openButton = root.querySelector("[data-open-session]");
  const exportButton = root.querySelector("[data-export-session]");
  const transferStatus = root.querySelector("[data-transfer-status]");
  const openFeedback = root.querySelector("[data-open-feedback]");
  const feedbackMessage = root.querySelector("[data-feedback-message]");
  const feedbackDetail = root.querySelector("[data-feedback-detail]");
  const exportFeedback = root.querySelector("[data-export-feedback]");
  const exportFeedbackMessage = root.querySelector("[data-export-feedback-message]");
  const exportFeedbackDetail = root.querySelector("[data-export-feedback-detail]");
  const reportStatus = root.querySelector("[data-report-status]");
  const reportPlaceholder = root.querySelector("[data-report-placeholder]");
  const reportViewer = root.querySelector("[data-report-viewer]");
  const reportFrame = root.querySelector("[data-report-frame]");
  const reportBundleName = root.querySelector("[data-report-bundle]");
  const reportSummary = root.querySelector("[data-report-summary]");

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
      : "Choose destination and create";
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
      setupReviewShape.textContent = `${humanizeIdentifier(plan.mode)} · ${humanizeIdentifier(plan.goal)} · ${plan.slots.length} slots`;
      setupReviewSlots.replaceChildren(
        ...plan.slots.map((slot) => {
          const row = root.createElement("tr");
          for (const value of [
            slot.sequenceNumber,
            slot.antennaLabel,
            slot.band,
            formatReviewTime(slot.startsAt),
            `${slot.durationSeconds}s + ${slot.guardSeconds}s guard`,
          ]) {
            const cell = root.createElement("td");
            cell.textContent = String(value);
            row.append(cell);
          }
          return row;
        }),
      );
    }

    openButton.disabled = state.openStatus === "loading";
    openButton.textContent = state.openStatus === "loading" ? "Opening…" : "Choose bundle";
    const exportLoading = state.exportStatus === "loading";
    exportButton.disabled = state.session === null || state.openStatus === "loading" || exportLoading;
    exportButton.textContent = state.session === null
      ? "Open a bundle first"
      : exportLoading
        ? "Exporting…"
        : "Export copy";
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

    const hasSession = state.session !== null;
    reportStatus.textContent = hasSession ? "Ready offline" : "Unavailable";
    reportStatus.classList.toggle("muted", !hasSession);
    reportPlaceholder.hidden = hasSession;
    reportViewer.hidden = !hasSession;
    if (hasSession) {
      reportBundleName.textContent = state.session.bundleName;
      reportSummary.textContent = `${state.session.callsign} · ${state.session.grid} · ${state.session.antennaCount} antennas · ${state.session.slotCount} slots · ${state.session.observationCount} observations`;
      updateReportFrame(reportFrame, state);
    }
  };

  for (const button of navigation) {
    button.addEventListener("click", () => {
      state = selectWorkflow(state, button.dataset.workflow);
      browserWindow.history.replaceState(null, "", `#${state.activeWorkflow}`);
      render();
      root.querySelector("main").focus({ preventScroll: true });
    });
  }

  browserWindow.addEventListener("hashchange", () => {
    state = selectWorkflow(
      state,
      workflowFromHash(browserWindow.location.hash),
    );
    render();
  });

  setupForm.addEventListener("input", () => {
    if (!setupBusyState(state)) {
      state = editSessionSetup(state);
      render();
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
        const reportHtml = await invokeActiveSessionReport(invoke);
        state = setupCreationSucceeded(state, { ...outcome.session, reportHtml });
      } else {
        throw new Error("The desktop command returned an unexpected response.");
      }
    } catch (error) {
      state = setupCreationFailed(state, error);
    }
    render();
  });

  setupAddAntennaButton.addEventListener("click", () => {
    const fragment = setupAntennaTemplate.content.cloneNode(true);
    setupAddAntennaButton.before(fragment);
    refreshAntennaRows(setupForm);
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
        const reportHtml = await invokeActiveSessionReport(invoke);
        state = openSessionSucceeded(state, { ...outcome.session, reportHtml });
        browserWindow.history.replaceState(null, "", "#report");
      } else {
        throw new Error("The desktop command returned an unexpected response.");
      }
    } catch (error) {
      state = openSessionFailed(state, error);
    }

    render();
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

  render();
}

function transferStatusText(state) {
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
      detail: "Review the exact UTC-backed schedule, then choose a destination.",
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

function readSetupDraft(form) {
  const value = (field) => form.querySelector(`[data-setup-field="${field}"]`).value;
  const localStart = value("startsAt");
  const startsAt = localStart ? new Date(localStart).toISOString() : "";
  return {
    station: {
      callsign: value("callsign"),
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
      startsAt,
      band: value("band"),
      durationSeconds: value("durationSeconds"),
      guardSeconds: value("guardSeconds"),
      rounds: value("rounds"),
    },
  };
}

function refreshAntennaRows(form) {
  const rows = [...form.querySelectorAll("[data-antenna-row]")];
  rows.forEach((row, index) => {
    row.querySelector("[data-antenna-title]").textContent = `Antenna ${String.fromCharCode(65 + index)}`;
    row.querySelector("[data-remove-antenna]").disabled = rows.length <= 1;
  });
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

if (typeof document !== "undefined") {
  mount(document, window);
}
