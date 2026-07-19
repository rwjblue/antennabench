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
  renderSavedSessions,
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

function reportDocumentHarness() {
  let sequence = 0;
  return {
    create() {
      sequence += 1;
      return `blob:report-${sequence}`;
    },
    revoke() {},
  };
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
  assert.equal(document.querySelector("[data-open-session]"), null);
  assert.doesNotMatch(DESKTOP_HTML, /Choose bundle|Edit session|>Tweak</);
  assert.match(document.querySelector('[data-panel="run"] [data-conductor-empty]').textContent, /Saved sessions/);
  assert.match(document.querySelector('[data-panel="report"] [data-report-placeholder]').textContent, /Saved sessions/);
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
  assert.deepEqual(e.navigation.map((node) => node.getAttribute("aria-current")), ["false", "false", "false", "page", "false"]);
  assert.deepEqual(e.panels.map((node) => node.hidden), [true, true, true, false, true]);
  assert.equal(e.navigation.filter((node) => node.classList.contains("active")).length, 1);
});

test("saved sessions render lifecycle actions, problems, duplicates, and distinct catalog states", () => {
  const e = loadDesktopDocument();
  const entry = (locatorId, lifecycle, overrides = {}) => ({
    locatorId,
    bundleName: `${locatorId}.session.antennabundle`,
    origin: "managed",
    originLabel: "Saved by AntennaBench",
    status: "available",
    callsign: "N1RWJ",
    createdAt: "2026-07-18T14:00:00Z",
    lifecycle,
    schemaVersion: 5,
    revision: 3,
    mode: "whole_station_ab",
    plannedRepetitions: 2,
    directionCoverage: "transmit_and_receive",
    plannedCycleCount: 8,
    observationCounts: { total: 3, localDecodes: 1, publicSpots: 1, importedSpots: 1 },
    bands: ["20m", "70cm"],
    antennaLabels: ["DXC", "Attic EFHW"],
    antennaCount: 2,
    sameSessionIdCount: 1,
    problems: [],
    ...overrides,
  });
  const state = initialState("saved");
  state.catalogStatus = "ready";
  state.managedCatalog = {
    status: "complete",
    diagnostics: [],
    entries: [
      entry("ready", "ready"),
      entry("running", "running"),
      entry("interrupted", "interrupted"),
      entry("ended", "ended", { sameSessionIdCount: 2 }),
      entry("abandoned", "abandoned", { sameSessionIdCount: 2 }),
      entry("legacy", null, { revision: null }),
      entry("draft", "draft"),
      entry(null, null, {
        bundleName: "broken.session.antennabundle",
        status: "invalid",
        callsign: null,
        createdAt: null,
        schemaVersion: null,
        revision: null,
        mode: null,
        plannedRepetitions: null,
        directionCoverage: null,
        plannedCycleCount: null,
        observationCounts: null,
        bands: [],
        antennaLabels: [],
        antennaCount: null,
        problems: [{ code: "bundle.invalid", severity: "error", message: "Manifest is invalid." }],
      }),
    ],
  };
  state.activeManagedLocatorId = "running";
  state.catalogRowError = {
    locatorId: "ended",
    error: { message: "Could not open.", detail: "The bundle moved." },
  };
  renderSavedSessions(e, state, document);

  const rows = [...e.savedCatalog.querySelectorAll(".saved-row")];
  const primaryLabels = rows.map((row) => row.querySelector(".primary-action")?.textContent ?? null);
  assert.deepEqual(primaryLabels, [
    "Start session", "Continue session", "Resume session", "View report",
    "View report", "View report", "View details", "View details",
  ]);
  assert.equal(rows[0].querySelector(".saved-plan-summary").textContent, "2 planned repetitions · TX + RX · 8 planned cycles · 3 recorded observations");
  assert.match(rows[0].querySelector(".saved-row-details").textContent, /Local decodes1 recorded observation/);
  assert.match(rows[0].querySelector(".saved-technical-details").textContent, /Schema5.*Committed revision3/s);
  assert.match(rows[3].querySelector(".saved-warning").textContent, /same session identity/);
  assert.equal(rows[3].querySelector(".saved-warning").hidden, false);
  assert.equal(rows[4].querySelector(".saved-warning").hidden, false);
  assert.match(rows[3].querySelector(".saved-row-error").textContent, /bundle moved/);
  assert.equal(rows[1].querySelector(".saved-open-now").textContent, "Open now");
  assert.equal(rows[7].querySelector(".saved-lifecycle").textContent, "Invalid session bundle");
  assert.match(rows[7].querySelector("details").textContent, /Manifest is invalid/);
  assert.equal(rows[0].querySelector('[data-saved-action="delete"]').textContent, "Delete…");
  assert.equal(rows[0].querySelector('[data-saved-action="delete"]').disabled, false);
  assert.equal(rows[1].querySelector('[data-saved-action="delete"]').disabled, true);
  assert.match(rows[1].querySelector('[data-saved-action="delete"]').title, /Close this session/);
  assert.equal(rows[7].querySelector('[data-saved-action="delete"]'), null);
  assert.equal(e.savedEmpty.hidden, true);

  state.catalogDeleteStatus = "confirming";
  state.catalogDeleteTarget = {
    locatorId: "ready",
    callsign: "N1RWJ",
    bundleName: "ready.session.antennabundle",
  };
  renderSavedSessions(e, state, document);
  assert.equal(e.savedDeleteDialog.getAttribute("aria-labelledby"), "saved-delete-title");
  assert.equal(e.savedDeleteDialog.getAttribute("aria-describedby"), "saved-delete-description");
  assert.match(e.savedDeleteIdentity.textContent, /N1RWJ.*ready\.session\.antennabundle/);
  assert.equal(e.savedDeleteCancel.disabled, false);
  assert.equal(e.savedDeleteConfirm.textContent, "Move to Trash");

  state.catalogDeleteStatus = "deleting";
  renderSavedSessions(e, state, document);
  assert.equal(e.savedDeleteCancel.disabled, true);
  assert.equal(e.savedDeleteConfirm.disabled, true);
  assert.equal(e.savedDeleteConfirm.textContent, "Moving to Trash…");
  assert.equal(e.savedCatalog.querySelector('[data-locator-id="ready"] [data-saved-action="delete"]').textContent, "Moving…");
  assert.equal(e.savedCatalog.querySelector('[data-locator-id="ended"] button').disabled, false, "unaffected rows stay interactive");

  state.catalogDeleteStatus = "idle";
  state.catalogDeleteTarget = null;

  state.managedCatalog.status = "incomplete";
  state.managedCatalog.diagnostics = [{ message: "Inspection budget reached." }];
  renderSavedSessions(e, state, document);
  assert.equal(e.savedStatus.textContent, "Partial list");
  assert.match(e.savedFeedback.textContent, /Only part.*Inspection budget reached/s);

  state.catalogStatus = "error";
  state.catalogError = { message: "Could not refresh.", detail: "Try again." };
  renderSavedSessions(e, state, document);
  assert.equal(e.savedCatalog.children.length, 8, "a failed refresh keeps stale rows visible");
  assert.match(e.savedFeedback.textContent, /Could not refresh/);

  state.catalogStatus = "ready";
  state.catalogError = null;
  state.managedCatalog = { status: "complete", entries: [], diagnostics: [] };
  renderSavedSessions(e, state, document);
  assert.equal(e.savedEmpty.hidden, false);
  assert.equal(e.savedCatalog.children.length, 0);

  state.catalogStatus = "loading";
  state.managedCatalog = null;
  renderSavedSessions(e, state, document);
  assert.equal(e.savedStatus.textContent, "Loading");
  assert.equal(e.savedCatalog.getAttribute("aria-busy"), "true");
  assert.equal(e.savedEmpty.hidden, true);

  state.catalogStatus = "error";
  state.catalogError = { message: "Saved sessions are unavailable.", detail: "Try again." };
  renderSavedSessions(e, state, document);
  assert.equal(e.savedStatus.textContent, "Unavailable");
  assert.equal(e.savedCatalog.children.length, 0);
  assert.match(e.savedFeedback.textContent, /Saved sessions are unavailable/);
});

test("setup renderer covers editing, review, diagnostics, creating, invalid, and created states", () => {
  const e = loadDesktopDocument();
  const state = initialState();
  renderSetup(e, state, document);
  assert.equal(e.setupStatus.textContent, "Draft");
  assert.match(e.setupPlanSummary.textContent, /16 planned WSPR cycles · about 32 minutes/);
  assert.match(e.setupPlanSummary.textContent, /then a 5-minute WSPR\.live ingestion grace/);
  assert.equal(e.setupCreateButton.disabled, true);
  assert.equal(e.setupForm.getAttribute("aria-busy"), "false");

  state.antennaControllerCatalog = {
    inputStyle: "one_line",
    profiles: [{ profileId: "profile-1", revision: "revision-1", name: "Bench switch" }],
  };
  state.antennaControllerProfileNotice = { kind: "saved", profileId: "profile-1" };
  renderSetup(e, state, document);
  assert.equal(e.controllerProfileSave.textContent, "Update profile");
  assert.equal(e.controllerProfileDelete.textContent, "Delete profile");
  state.antennaControllerProfileNotice = null;

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

  const targetList = e.setupForm.querySelector("[data-controller-targets]");
  const targetInputs = ["Vertical", "Dipole"].map((antennaLabel) => {
    const label = document.createElement("label");
    const input = document.createElement("input");
    input.dataset.controllerTarget = "";
    input.dataset.antennaLabel = antennaLabel;
    label.append(input);
    targetList.append(label);
    return input;
  });
  state.setupReview.diagnostics = [{
    field: "antennaController",
    message: 'antenna "Dipole" requires a bounded nonempty target',
    code: "setup.antenna_controller.invalid",
  }];
  renderSetup(e, state, document);
  assert.equal(targetInputs[0].hasAttribute("aria-invalid"), false);
  assert.equal(targetInputs[1].getAttribute("aria-invalid"), "true");

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
        requiredCycleMinutes: 4,
        finalizationGraceMinutes: 5,
        summary: "2 directed WSPR cycles; about 4 minutes of required cycle time; then a 5-minute WSPR.live ingestion grace.",
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
  assert.match(e.setupReviewSchedule.textContent, /about 4 minutes of required cycle time/);
  assert.match(e.setupReviewSchedule.textContent, /then a 5-minute WSPR\.live ingestion grace/);
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

test("ready and interrupted runs require the persistent WSJT-X acknowledgement", () => {
  const e = loadDesktopDocument();
  const state = initialState("run");
  state.conductorStatus = "ready";
  state.conductor = conductorView({
    revision: 1,
    lifecycle: "ready",
    wsjtxRequired: false,
    wsjtxReadiness: {
      band: "20m",
      powerWatts: 5,
      wsprLiveAcquisitionEnabled: true,
      hasReceivePeriods: true,
      nextDirection: "transmit",
    },
  });
  state.wsjtx = null;
  state.antennaController = {
    policy: "command_controlled",
    armed: true,
    invocation: "automatic",
    automationStatus: "waiting",
    profileId: "profile-1",
  };

  renderRun(e, state, document, { monotonicNow: () => 1000 });
  assert.equal(e.wsjtxReadiness.hidden, false);
  assert.match(e.wsjtxReadinessItems.textContent, /Band: 20 m/);
  assert.match(e.wsjtxReadinessItems.textContent, /Enable Tx: On/);
  assert.match(e.wsjtxReadinessItems.textContent, /Upload spots/);
  assert.equal(e.lifecycleButtons[0].disabled, true, "automatic control cannot bypass acknowledgement");
  assert.equal(e.wsjtxSetupWarnings.hidden, true, "missing UDP status does not hide the checklist");

  state.wsjtxReadinessAcknowledgement = "session-1:1:ready";
  renderRun(e, state, document, { monotonicNow: () => 1000 });
  assert.equal(e.wsjtxReadinessAcknowledge.checked, true);
  assert.equal(e.lifecycleButtons[0].disabled, false);

  state.conductor = conductorView({
    revision: 7,
    lifecycle: "interrupted",
    wsjtxRequired: false,
    wsjtxReadiness: {
      band: "20m",
      powerWatts: 5,
      wsprLiveAcquisitionEnabled: false,
      hasReceivePeriods: true,
      nextDirection: "receive",
    },
  });
  renderRun(e, state, document, { monotonicNow: () => 1000 });
  assert.equal(e.lifecycleButtons[1].disabled, true);
  assert.match(e.wsjtxReadinessItems.textContent, /Enable Tx: Off/);
  assert.doesNotMatch(e.wsjtxReadinessItems.textContent, /Upload spots/);
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

  assert.equal(e.transferStatus.textContent, "Session active");
});

test("report renderer covers unavailable, refreshing, ready, exporting, error, and frame identity", () => {
  const e = loadDesktopDocument();
  const state = initialState("report");
  const reportDocuments = reportDocumentHarness();
  renderReport(e, state, reportDocuments);
  assert.equal(e.reportStatus.textContent, "Unavailable");
  assert.equal(e.reportViewer.hidden, true);

  state.session = {
    bundleName: "test.session.antennabundle", callsign: "N1RWJ", grid: "FN42",
    antennaCount: 2, slotCount: 4, observationCount: 8, revision: 3,
    lifecycle: "running", completeness: "full_detail", reportHtml: "<p>three</p>",
  };
  state.reportStatus = "ready";
  state.reportPresentationId = 3;
  renderReport(e, state, reportDocuments);
  assert.equal(e.reportFrame.getAttribute("src"), "blob:report-1");
  assert.equal(e.reportControllerOptions.hidden, true);
  assert.equal(e.reportControllerHandling.value, "complete");

  state.session.hasControllerEvidence = true;
  renderReport(e, state, reportDocuments);
  assert.equal(e.reportControllerOptions.hidden, false);
  assert.equal(e.reportControllerHandling.value, "complete", "controller details are included by default");
  e.reportControllerHandling.value = "omitted_at_export";
  renderReport(e, state, reportDocuments);
  assert.equal(e.reportControllerHandling.value, "omitted_at_export", "choice persists for one presentation");
  state.reportPresentationId = 4;
  renderReport(e, state, reportDocuments);
  assert.equal(e.reportControllerHandling.value, "complete", "a new snapshot defaults to include");
  state.session.hasControllerEvidence = false;
  renderReport(e, state, reportDocuments);
  assert.equal(e.reportControllerOptions.hidden, true);
  e.reportFrame.src = "sentinel";
  renderReport(e, state, reportDocuments);
  assert.match(e.reportFrame.src, /sentinel$/, "same presentation does not replace the blob document");

  state.reportStatus = "refreshing";
  renderReport(e, state, reportDocuments);
  assert.equal(e.reportStatus.textContent, "Refreshing");
  assert.equal(e.reportRefreshButton.disabled, true);
  state.reportStatus = "ready";
  state.reportExportStatus = "loading";
  renderReport(e, state, reportDocuments);
  assert.equal(e.reportCompactExportButton.textContent, "Exporting…");
  assert.equal(e.reportFullExportButton.textContent, "Exporting…");
  state.reportExportStatus = "error";
  state.reportExportError = { message: "cannot export", detail: "destination exists" };
  renderReport(e, state, reportDocuments);
  assert.equal(e.reportFeedback.dataset.kind, "error");
  assert.equal(e.reportFeedbackDetail.textContent, "destination exists");
});

test("operational history renders every honest accessible state and reopened-session alert", () => {
  const e = loadDesktopDocument();
  const reportDocuments = reportDocumentHarness();
  const state = initialState("report");
  state.reportStatus = "ready";
  state.reportPresentationId = 9;
  state.session = {
    bundleName: "diagnostic.session.antennabundle",
    callsign: "N1RWJ",
    grid: "FN42",
    antennaCount: 2,
    slotCount: 4,
    observationCount: 8,
    revision: 12,
    lifecycle: "interrupted",
    completeness: "full_detail",
    reportHtml: "<p>report excludes operational history</p>",
    operationalHistory: {
      historyState: "complete",
      retainedCount: 2,
      retentionOmittedCount: 0,
      presentationOmittedCount: 0,
      contextOmittedCount: 0,
      supportSummaryOmittedCount: 0,
      recordLimit: 2048,
      byteLimit: 16 * 1024 * 1024,
      supportSummary: "{\"schema\":\"antennabench_support_summary.v1\"}",
      contexts: [
        {
          contextId: "ctx_creator", creator: true, firstRecordedAt: "2026-07-18T12:00:00Z",
          appVersion: "0.1.0", sourceCommit: "abc123", sourceState: "clean",
          buildChannel: "official_release", targetTriple: "aarch64-apple-darwin",
          osFamily: "macos", osVersion: "15.5", runtimeArchitecture: "aarch64",
        },
        {
          contextId: "ctx_resume", creator: false, firstRecordedAt: "2026-07-19T12:00:00Z",
          appVersion: "0.2.0-dev", sourceCommit: "def456", sourceState: "dirty",
          buildChannel: "development", targetTriple: "x86_64-apple-darwin",
          osFamily: "macos", osVersion: "15.6", runtimeArchitecture: "x86_64",
        },
      ],
      diagnostics: [
        {
          diagnosticId: "diag-1", runtimeContextId: "ctx_resume",
          occurredAt: "2026-07-19T12:05:00Z", operation: "wspr_live_acquisition",
          phase: "preflight", code: "resource.jsonl_line_bytes",
          summary: "The WSPR.live response could not be committed.", outcome: "failed",
          severity: "error", revisionBefore: 10, revisionAfter: 10,
          evidenceEffect: "none_committed", retryDisposition: "requires_input_change",
          retryGuidanceCode: "wspr_live.reduce_input", targets: ["window: start to end"],
          causes: ["resource.jsonl_line_bytes (preflight) · observed_bytes=301337, limit_bytes=262144"],
          detailTruncated: false,
        },
        {
          diagnosticId: "diag-2", runtimeContextId: "ctx_resume",
          occurredAt: "2026-07-19T12:06:00Z", operation: "wspr_live_acquisition",
          phase: "finalize", code: "wspr_live.finalization_failed",
          summary: "Capture committed before finalization failed.", outcome: "partial",
          severity: "error", revisionBefore: 11, revisionAfter: 12,
          evidenceEffect: "primary_evidence_committed", retryDisposition: "requires_state_change",
          retryGuidanceCode: "session.resolve_state", targets: [], causes: [],
          detailTruncated: true,
        },
      ],
    },
  };

  renderReport(e, state, reportDocuments);
  assert.equal(e.operationalHistory.getAttribute("aria-labelledby"), "operational-history-title");
  assert.equal(e.operationalHistory.open, false, "support history is collapsed by default");
  assert.equal(e.operationalHistoryAlert.hidden, false, "material diagnostics stay visible");
  assert.match(e.operationalHistoryAlertTitle.textContent, /partial result/i);
  assert.match(e.operationalHistoryAlertMessage.textContent, /supporting detail/i);
  assert.equal(e.operationalHistoryContexts.children.length, 2);
  assert.equal(e.operationalHistoryDiagnostics.children.length, 2);
  assert.match(e.operationalHistoryDiagnostics.textContent, /observed_bytes=301337/);
  assert.match(e.operationalHistoryDiagnostics.textContent, /Primary evidence committed/i);
  assert.equal(e.reportOperationalHandling.value, "omitted", "full export is private by default");
  assert.match(e.operationalHistoryBounds.textContent, /2048 records/);
  e.operationalHistory.querySelector("summary").click();
  assert.equal(e.operationalHistory.open, true, "native summary expands the support detail");
  renderReport(e, state, reportDocuments);
  assert.equal(e.operationalHistory.open, true, "rerender preserves the operator's disclosure state");
  state.reportPresentationId = 10;
  renderReport(e, state, reportDocuments);
  assert.equal(e.operationalHistory.open, false, "a new report presentation starts collapsed");

  const bothDiagnostics = state.session.operationalHistory.diagnostics;
  state.session.operationalHistory.diagnostics = bothDiagnostics.slice(0, 1);
  renderReport(e, state, reportDocuments);
  assert.equal(e.operationalHistoryDiagnostics.children.length, 1, "one-failure state");
  state.session.operationalHistory.diagnostics = bothDiagnostics;

  renderRun(e, state, document);
  assert.equal(e.runHistoricalDiagnostic.hidden, false);
  assert.match(e.runHistoricalTitle.textContent, /retained only part/i);
  assert.match(e.runHistoricalMeta.textContent, /primary evidence committed/i);

  const messages = new Map();
  const alerts = new Map();
  for (const historyState of [
    "complete", "legacy_unknown", "retention_capped", "persistence_gap", "unavailable",
  ]) {
    state.session.operationalHistory = {
      ...state.session.operationalHistory,
      historyState,
      diagnostics: [],
      reasonCode: historyState === "persistence_gap" ? "diagnostic.persistence_failed" : null,
    };
    renderReport(e, state, reportDocuments);
    messages.set(historyState, e.operationalHistoryMessage.textContent);
    alerts.set(historyState, e.operationalHistoryAlert.hidden);
  }
  assert.equal(new Set(messages.values()).size, 5);
  assert.match(messages.get("legacy_unknown"), /unknown, not clean/);
  assert.match(messages.get("retention_capped"), /later outcomes may be absent/i);
  assert.match(messages.get("persistence_gap"), /known persistence gap/i);
  assert.match(messages.get("unavailable"), /cannot infer/);
  assert.match(messages.get("complete"), /storage or process loss/);
  assert.equal(alerts.get("complete"), true, "clean complete history has no alert");
  assert.equal(alerts.get("persistence_gap"), false, "a persistence gap has a compact alert");

  state.session.operationalHistory.historyState = "persistence_gap";
  renderReport(e, state, reportDocuments);
  assert.equal(e.operationalHistory.open, false);
  assert.equal(e.operationalHistoryAlert.hidden, false);
  assert.match(e.operationalHistoryAlertTitle.textContent, /known persistence gap/i);
  e.operationalHistory.querySelector("summary").click();
  assert.equal(e.operationalHistory.open, true);
  assert.equal(e.copySupportSummary.disabled, false);
  assert.match(e.copySupportSummary.parentElement.nextElementSibling.textContent, /redacted by default/i);
});
