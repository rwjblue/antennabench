import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test, vi } from "vitest";

import {
  REQUIRED_ELEMENT_LIST_SELECTORS,
  REQUIRED_ELEMENT_SELECTORS,
  collectDesktopElements,
} from "../frontend/elements.mjs";
import { createDesktopController } from "../frontend/controller.mjs";
import { CONTEXT_HELP, installContextualHelp } from "../frontend/models.mjs";

import {
  renderNavigation,
  renderReport,
  renderRun,
  renderSetup,
  renderTransfer,
} from "../frontend/renderers.mjs";
import { initialState } from "../frontend/state.mjs";

const DESKTOP_HTML = readFileSync(
  path.join(process.cwd(), "frontend", "index.html"),
  "utf8",
);

function loadDesktopDocument() {
  document.open();
  document.write(DESKTOP_HTML);
  document.close();
  return collectDesktopElements(document);
}

test("the checked-in HTML satisfies the fail-fast renderer element inventory", () => {
  const elements = loadDesktopDocument();
  assert.ok(elements.mainContent instanceof HTMLElement);
  for (const selector of [
    ...Object.values(REQUIRED_ELEMENT_SELECTORS),
    ...Object.values(REQUIRED_ELEMENT_LIST_SELECTORS),
  ]) {
    assert.ok(document.querySelector(selector), `missing ${selector}`);
  }

  const emptyDocument = document.implementation.createHTMLDocument("empty");
  assert.throws(
    () => collectDesktopElements(emptyDocument),
    /Missing required desktop element mainContent/,
  );
});

test("contextual help is centralized, keyboard accessible, and fully inventoried", () => {
  loadDesktopDocument();
  const root = document.documentElement;
  const trigger = root.querySelector('[data-help-trigger="countdown"]');
  installContextualHelp(root);
  const popover = document.getElementById(trigger.getAttribute("aria-controls"));

  assert.equal(trigger.getAttribute("aria-label"), "Help: Countdown");
  assert.equal(trigger.getAttribute("aria-expanded"), "false");
  assert.equal(popover.getAttribute("role"), "note");
  assert.match(popover.textContent, /time until the current transmission ends/);
  assert.equal(popover.hidden, true);

  trigger.click();
  assert.equal(trigger.getAttribute("aria-expanded"), "true");
  assert.equal(trigger.getAttribute("aria-describedby"), popover.id);
  assert.equal(popover.hidden, false);
  assert.equal(popover.parentElement, document.body);
  assert.match(popover.style.left, /px$/);
  assert.match(popover.style.top, /px$/);

  document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  assert.equal(trigger.getAttribute("aria-expanded"), "false");
  assert.equal(document.activeElement, trigger);

  trigger.click();
  document.body.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  assert.equal(popover.hidden, true);
  assert.equal(trigger.getAttribute("aria-expanded"), "false");

  for (const help of Object.values(CONTEXT_HELP)) {
    const sentences = help.text.match(/[.!?](?:\s|$)/g) ?? [];
    assert.ok(sentences.length <= 2, `${help.title} is longer than two sentences`);
  }

  const inventory = new Set(
    [...document.querySelectorAll("[data-help-trigger]")]
      .map((element) => element.dataset.helpTrigger),
  );
  assert.deepEqual([...inventory].sort(), Object.keys(CONTEXT_HELP).sort());
  assert.equal(
    document.querySelectorAll("button[data-help-trigger]").length,
    Object.keys(CONTEXT_HELP).length,
  );
  assert.equal(document.querySelector('[title*="help" i]'), null);
});

test("contextual help accepts the document root used by the desktop mount", () => {
  loadDesktopDocument();
  assert.doesNotThrow(() => installContextualHelp(document));
  const trigger = document.querySelector('[data-help-trigger="setup_question"]');
  assert.ok(document.getElementById(trigger.getAttribute("aria-controls")));
});

test("navigation renders exactly one active accessible workflow", () => {
  const e = loadDesktopDocument();
  renderNavigation(e, { activeWorkflow: "transfer" });
  assert.deepEqual(e.navigation.map((node) => node.getAttribute("aria-current")), ["false", "false", "page", "false"]);
  assert.deepEqual(e.panels.map((node) => node.hidden), [true, true, false, true]);
  assert.equal(e.navigation.filter((node) => node.classList.contains("active")).length, 1);
});

test("setup renderer covers editing, review, diagnostics, creating, invalid, and created states", () => {
  const e = loadDesktopDocument();
  const state = initialState();
  renderSetup(e, state, document);
  assert.equal(e.setupStatus.textContent, "Draft");
  assert.match(e.setupPlanSummary.textContent, /16 planned WSPR cycles · about 32 minutes/);
  assert.equal(e.setupCreateButton.disabled, true);
  assert.equal(e.setupForm.getAttribute("aria-busy"), "false");

  state.setupStatus = "reviewing";
  renderSetup(e, state, document);
  assert.equal(e.setupReviewButton.textContent, "Validating…");
  assert.equal(e.setupForm.getAttribute("aria-busy"), "true");

  state.setupStatus = "invalid";
  state.setupReview = {
    diagnostics: [
      { field: "station.grid", message: "Grid required", code: "grid.required" },
      {
        field: "antennaController",
        message: "The controller profile name is required",
        code: "setup.antenna_controller.invalid",
      },
    ],
    plan: null,
  };
  renderSetup(e, state, document);
  assert.equal(e.setupStatus.textContent, "Needs changes");
  assert.equal(e.setupDiagnostics.hidden, false);
  assert.equal(e.setupDiagnostics.children[0].children[1].textContent, "Grid required");
  const grid = e.setupForm.querySelector('[data-setup-field="grid"]');
  assert.equal(grid.getAttribute("aria-invalid"), "true");
  assert.equal(grid.closest("label").querySelector(".field-error").textContent, "Grid required");
  const profileName = e.setupForm.querySelector('[data-setup-field="controllerProfileName"]');
  assert.equal(profileName.getAttribute("aria-invalid"), "true");
  assert.match(profileName.closest("label").querySelector(".field-error").textContent, /profile name/);

  state.setupStatus = "reviewed";
  state.setupReview = {
    diagnostics: [],
    plan: {
      station: { callsign: "N1RWJ", grid: "FN42", powerWatts: 5 },
      antennas: [{ label: "Dipole", context: "north" }],
      mode: "whole_station_ab",
      goal: "general_coverage",
      wsprLiveAcquisitionEnabled: true,
      signalPlan: null,
      scheduleReview: {
        periodKind: "wspr_cycle",
        periodCount: 2,
        wsprCycleCount: 2,
        idealMinimumMinutes: 4,
        summary: "2 directed WSPR cycles; ideal minimum 4 minutes.",
        counterbalanceExplanation: "Successive repetitions reverse the antenna order.",
        transitionSummary: "1 transition: 1 antenna change, 1 direction change, 1 requiring both.",
        transitions: [{
          fromSequenceNumber: 1,
          toSequenceNumber: 2,
          antennaChange: true,
          directionChange: true,
          summary: "Change antenna and TX/RX direction",
        }],
      },
      capabilities: {
        canDescribe: ["Transmit-path same-path differences."],
        cannotEstablish: ["A winner."],
      },
      slots: [
        { sequenceNumber: 1, antennaLabel: "Dipole", direction: "transmit", band: "20m", signal: null },
        { sequenceNumber: 2, antennaLabel: "Vertical", direction: "receive", band: "20m", signal: null },
      ],
    },
  };
  renderSetup(e, state, document);
  assert.equal(e.setupCreateButton.disabled, false);
  assert.equal(e.setupReviewPanel.hidden, false);
  assert.match(e.setupReviewShape.textContent, /Whole Station Ab/);
  assert.match(e.setupReviewSchedule.textContent, /ideal minimum 4 minutes/);
  assert.equal(e.setupReviewSequence.children[0].children[0].textContent, "1. Transmit · Dipole");
  assert.equal(e.setupReviewSequence.children[1].children[0].textContent, "Change antenna and TX/RX direction");
  assert.equal(e.setupReviewSequence.children[1].children[1].textContent, "2. Receive · Vertical");
  assert.equal(e.setupReviewCanDescribe.children[0].textContent, "Transmit-path same-path differences.");
  assert.equal(e.setupReviewCannotEstablish.children[0].textContent, "A winner.");
  assert.equal(e.setupReviewSlots.children.length, 2);

  state.setupStatus = "creating";
  renderSetup(e, state, document);
  assert.equal(e.setupCreateButton.textContent, "Creating…");
  state.setupStatus = "created";
  state.setupNotice = "created";
  state.session = { bundleName: "field.session.antennabundle", slotCount: 1 };
  renderSetup(e, state, document);
  assert.equal(e.setupStatus.textContent, "Session ready");
  assert.match(e.setupFeedbackMessage.textContent, /field\.session/);
});

function conductorView(overrides = {}) {
  return {
    sessionId: "session-1", revision: 4, actionToken: "token-4", lifecycle: "running",
    phase: "between_slots", antennaInUse: "Dipole", guidance: "Switch and confirm",
    secondsToTransition: 30, now: "2026-07-16T23:00:00Z", currentSlot: null,
    nextSlot: null,
    nextIntent: { intentId: "intent-1", sequenceNumber: 1, direction: "transmit", antennaLabel: "Dipole", band: "20m" },
    slots: [{ slotId: "slot-1", sequenceNumber: 1, plannedAntenna: "Dipole", band: "20m" }],
    antennas: ["Dipole", "Vertical"], diagnostics: [], effectiveEvents: [],
    wsjtxRequired: true,
    ...overrides,
  };
}

test("run renderer covers lifecycle actions, cycles, evidence controls, and adapter states", () => {
  const e = loadDesktopDocument();
  const state = initialState("run");
  state.conductorStatus = "ready";
  state.conductor = conductorView();
  state.wsjtx = { phase: "stopped", receivedDatagrams: 0, committedMutations: 0, ignoredDatagrams: 0 };
  state.antennaController = { policy: "manual", attached: false, armed: false, targets: {} };
  let result = renderRun(e, state, document, { monotonicNow: () => 1000 });
  assert.equal(e.conductorPanel.hidden, false);
  assert.equal(e.conductorCountdown.textContent, "00:30");
  assert.match(e.nextSlot.children[0].textContent, /Transmit on Dipole/);
  assert.equal(e.lifecycleButtons[0].disabled, true, "start waits for required WSJT-X");
  assert.equal(e.evidenceSlot.children.length, 2);
  state.conductorStatus = "refreshing";
  renderRun(e, state, document, { monotonicNow: () => 1000 });
  assert.equal(e.conductorStatus.textContent, "Refreshing");
  assert.match(e.conductorFeedbackMessage.textContent, /Recovering/);
  state.conductorStatus = "ready";
  state.antennaControllerOutcome = {
    detail: "Switch failed; manual operation remains available.",
    diagnostic: "Switch program: /opt/controller\ndisposition: exit 7\nstderr: failure",
  };
  renderRun(e, state, document, { monotonicNow: () => 1000 });
  assert.equal(e.antennaControllerDiagnostic.hidden, false);
  assert.match(e.antennaControllerDiagnostic.textContent, /stderr: failure/);

  state.wsjtx = {
    phase: "running", bindAddress: "127.0.0.1", receivedDatagrams: 2,
    committedMutations: 1, ignoredDatagrams: 1,
    setupWarnings: [{ code: "wsjtx.setup.tx_disabled_during_transmit", message: "Turn Enable Tx on." }],
  };
  state.wsprLiveAcquisitionStatus = "fetching";
  result = renderRun(e, state, document, {
    monotonicNow: () => 1000,
    countdownAnchor: result.anchor,
    countdownKey: result.key,
  });
  assert.equal(e.wsjtxPhase.textContent, "Running · 127.0.0.1");
  assert.equal(e.wsprLivePhase.textContent, "Collecting best-effort public spots…");
  assert.match(e.wsjtxCounts.textContent, /Direct\/local active/);
  assert.equal(e.wsjtxSetupWarnings.hidden, false);
  assert.equal(e.wsjtxSetupWarnings.children[0].dataset.code, "wsjtx.setup.tx_disabled_during_transmit");
  assert.equal(e.wsjtxSetupWarnings.children[0].children[1].textContent, "Turn Enable Tx on.");
  assert.equal(e.lifecycleButtons[2].disabled, false, "advisory warnings do not disable conductor actions");
  assert.equal(e.wsprLiveRetry.disabled, true);
  assert.equal(e.wsprLiveCompact.dataset.kind, "checking");
  assert.match(e.wsprLiveCompact.textContent, /Checking/);
  assert.equal(e.skipCycleControl.hidden, false);

  const slot = {
    slotId: "slot-1", sequenceNumber: 1, direction: "transmit", band: "20m",
    plannedAntenna: "Dipole", startsAt: "2026-07-16T23:02:00Z",
    actualAntenna: "Dipole", evidenceStatus: "confirmed", plannedSignal: null,
  };
  state.conductor = conductorView({
    phase: "active",
    currentSlot: slot,
    nextSlot: { ...slot, slotId: "slot-2", sequenceNumber: 2, plannedAntenna: "Vertical", actualAntenna: null },
    nextIntent: null,
  });
  renderRun(e, state, document, { monotonicNow: () => 1000 });
  assert.equal(e.currentSlot.children[0].textContent, "Cycle 1");
  assert.equal(e.currentSlot.children[2].textContent, "Actual: Dipole");
  assert.equal(e.nextSlot.children[0].textContent, "Cycle 2");
  assert.equal(e.skipCycleControl.hidden, true);

  state.wsprLiveAcquisitionStatus = "error";
  state.wsprLiveAcquisitionError = { message: "mirror down" };
  state.conductor = conductorView({
    phase: "finalizing",
    effectiveEvents: [{ kind: "add_note", sourceEventId: "event-1", summary: "Wind", occurredAt: "2026-07-16T23:00:00Z", slotId: null }],
  });
  renderRun(e, state, document, { monotonicNow: () => 1000 });
  assert.equal(e.wsprLiveRetry.hidden, false);
  assert.equal(e.wsprLiveEndWithout.hidden, false);
  assert.equal(e.conductorEvents.children[0].children[1].children.length, 2);
  assert.equal(e.conductorEvents.children[0].children[1].children[0].textContent, "Replace");
});

test("a full polling interval preserves the mounted Active Run text until atomic success", async () => {
  const e = loadDesktopDocument();
  const view = conductorView({ guidance: "Switch and confirm", secondsToTransition: 30 });
  const state = initialState("run");
  state.session = { lifecycle: "running", schemaVersion: 5 };
  state.conductorStatus = "ready";
  state.conductor = view;
  state.conductorNotice = "Session started.";
  state.wsprLiveAcquisitionStatus = "ready";
  state.wsprLiveAcquisition = { status: "disabled" };
  state.wsjtx = { phase: "stopped", receivedDatagrams: 0, committedMutations: 0, ignoredDatagrams: 0 };
  state.antennaController = { policy: "manual", attached: false, armed: false, targets: {} };
  const intervals = [];
  let resolvePoll;
  const calls = [];
  const controller = createDesktopController({
    state,
    invoke: async (command) => {
      calls.push(command);
      if (command === "active_session_conductor") {
        return new Promise((resolve) => { resolvePoll = resolve; });
      }
      if (command === "active_session_antenna_controller") return state.antennaController;
      if (command === "active_session_wsjtx_status") return state.wsjtx;
      if (command === "advance_active_session_wspr_live") return { status: "disabled" };
      throw new Error(`unexpected command ${command}`);
    },
    render: (next) => renderRun(e, next, document, { monotonicNow: () => 1000 }),
    setInterval: (callback, milliseconds) => {
      intervals.push({ callback, milliseconds });
      return callback;
    },
    getCountdownAnchor: () => null,
  });
  controller.render();
  const visibleBefore = {
    status: e.conductorStatus.textContent,
    guidance: e.conductorGuidance.textContent,
    feedback: e.conductorFeedbackMessage.textContent,
    action: e.lifecycleButtons.find((button) => !button.hidden)?.textContent,
    compact: e.wsprLiveCompact.textContent,
    countdown: e.conductorCountdown.textContent,
  };

  controller.start();
  intervals.find(({ milliseconds }) => milliseconds === 5000).callback();
  await vi.waitFor(() => assert.equal(calls[0], "active_session_conductor"));
  assert.deepEqual({
    status: e.conductorStatus.textContent,
    guidance: e.conductorGuidance.textContent,
    feedback: e.conductorFeedbackMessage.textContent,
    action: e.lifecycleButtons.find((button) => !button.hidden)?.textContent,
    compact: e.wsprLiveCompact.textContent,
    countdown: e.conductorCountdown.textContent,
  }, visibleBefore);

  resolvePoll({ ...view });
  await vi.waitFor(() => assert.ok(calls.includes("advance_active_session_wspr_live")));
  assert.equal(e.conductorStatus.textContent, visibleBefore.status);
  assert.equal(e.conductorGuidance.textContent, visibleBefore.guidance);
  assert.equal(e.conductorFeedbackMessage.textContent, visibleBefore.feedback);
  controller.dispose();
});

test("transfer renderer covers lifecycle/schema eligibility and feedback outcomes", () => {
  const e = loadDesktopDocument();
  const state = initialState("transfer");
  renderTransfer(e, state);
  assert.equal(e.exportButton.disabled, true);
  assert.equal(e.importRbnButton.textContent, "Open a session first");

  state.openStatus = "ready";
  state.session = { lifecycle: "running", schemaVersion: 3, bundleName: "test" };
  renderTransfer(e, state);
  assert.equal(e.importWsprLiveButton.disabled, false);
  assert.equal(e.importRbnButton.disabled, false);

  state.notice = "cancelled";
  state.openStatus = "idle";
  renderTransfer(e, state);
  assert.equal(e.openFeedback.dataset.kind, "cancelled");
  state.notice = null;
  state.error = { message: "bad bundle", detail: "invalid JSON" };
  state.openStatus = "error";
  renderTransfer(e, state);
  assert.equal(e.openFeedback.dataset.kind, "error");
  assert.equal(e.feedbackDetail.textContent, "invalid JSON");
});

test("report renderer covers unavailable, refreshing, ready, exporting, error, and frame identity", () => {
  const e = loadDesktopDocument();
  const state = initialState("report");
  renderReport(e, state);
  assert.equal(e.reportStatus.textContent, "Unavailable");
  assert.equal(e.reportViewer.hidden, true);

  state.session = {
    bundleName: "test.session.antennabundle", callsign: "N1RWJ", grid: "FN42",
    antennaCount: 2, slotCount: 4, observationCount: 8, revision: 3,
    lifecycle: "running", completeness: "full_detail", reportHtml: "<p>three</p>",
  };
  state.reportStatus = "ready";
  state.reportPresentationId = 3;
  renderReport(e, state);
  assert.equal(e.reportFrame.srcdoc, "<p>three</p>");
  assert.equal(e.reportControllerOptions.hidden, true);
  assert.equal(e.reportControllerHandling.value, "complete");

  state.session.hasControllerEvidence = true;
  renderReport(e, state);
  assert.equal(e.reportControllerOptions.hidden, false);
  assert.equal(e.reportControllerHandling.value, "complete", "controller details are included by default");
  e.reportControllerHandling.value = "omitted_at_export";
  renderReport(e, state);
  assert.equal(e.reportControllerHandling.value, "omitted_at_export", "choice persists for one presentation");
  state.reportPresentationId = 4;
  renderReport(e, state);
  assert.equal(e.reportControllerHandling.value, "complete", "a new snapshot defaults to include");
  state.session.hasControllerEvidence = false;
  renderReport(e, state);
  assert.equal(e.reportControllerOptions.hidden, true);
  e.reportFrame.srcdoc = "sentinel";
  renderReport(e, state);
  assert.equal(e.reportFrame.srcdoc, "sentinel", "same presentation does not replace srcdoc");

  state.reportStatus = "refreshing";
  renderReport(e, state);
  assert.equal(e.reportStatus.textContent, "Refreshing");
  assert.equal(e.reportRefreshButton.disabled, true);
  state.reportStatus = "ready";
  state.reportExportStatus = "loading";
  renderReport(e, state);
  assert.equal(e.reportCompactExportButton.textContent, "Exporting…");
  assert.equal(e.reportFullExportButton.textContent, "Exporting…");
  state.reportExportStatus = "error";
  state.reportExportError = { message: "cannot export", detail: "destination exists" };
  renderReport(e, state);
  assert.equal(e.reportFeedback.dataset.kind, "error");
  assert.equal(e.reportFeedbackDetail.textContent, "destination exists");
});
