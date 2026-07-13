export const WORKFLOWS = Object.freeze(["setup", "run", "transfer", "report"]);

export function initialState(workflow = "setup") {
  return selectWorkflow(
    {
      activeWorkflow: "setup",
      openStatus: "idle",
      session: null,
      error: null,
      notice: null,
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

export function openSessionSucceeded(state, session) {
  return {
    ...state,
    activeWorkflow: "report",
    openStatus: "ready",
    session,
    error: null,
    notice: null,
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

export function invokeActiveSessionReport(invoke) {
  return invoke("active_session_report");
}

function mount(root, browserWindow) {
  let state = initialState(workflowFromHash(browserWindow.location.hash));
  const navigation = [...root.querySelectorAll("[data-workflow]")];
  const panels = [...root.querySelectorAll("[data-panel]")];
  const openButton = root.querySelector("[data-open-session]");
  const transferStatus = root.querySelector("[data-transfer-status]");
  const openFeedback = root.querySelector("[data-open-feedback]");
  const feedbackMessage = root.querySelector("[data-feedback-message]");
  const feedbackDetail = root.querySelector("[data-feedback-detail]");
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

    openButton.disabled = state.openStatus === "loading";
    openButton.textContent = state.openStatus === "loading" ? "Opening…" : "Choose bundle";
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

    const hasSession = state.session !== null;
    reportStatus.textContent = hasSession ? "Ready offline" : "Unavailable";
    reportStatus.classList.toggle("muted", !hasSession);
    reportPlaceholder.hidden = hasSession;
    reportViewer.hidden = !hasSession;
    if (hasSession) {
      reportBundleName.textContent = state.session.bundleName;
      reportSummary.textContent = `${state.session.callsign} · ${state.session.grid} · ${state.session.antennaCount} antennas · ${state.session.slotCount} slots · ${state.session.observationCount} observations`;
      if (reportFrame.dataset.sessionId !== state.session.sessionId) {
        reportFrame.srcdoc = state.session.reportHtml;
        reportFrame.dataset.sessionId = state.session.sessionId;
      }
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

  render();
}

function transferStatusText(state) {
  if (state.openStatus === "loading") return "Opening bundle";
  if (state.openStatus === "ready") return "Bundle open";
  if (state.openStatus === "error") return "Open failed";
  return "No bundle open";
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

if (typeof document !== "undefined") {
  mount(document, window);
}
