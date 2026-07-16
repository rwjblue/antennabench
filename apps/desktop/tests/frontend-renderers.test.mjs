import assert from "node:assert/strict";
import test from "node:test";

import {
  renderNavigation,
  renderReport,
  renderRun,
  renderSetup,
  renderTransfer,
} from "../frontend/renderers.mjs";
import { initialState } from "../frontend/state.mjs";

class FakeElement {
  constructor(tag = "div", document = null) {
    this.tagName = tag.toUpperCase();
    this.ownerDocument = document;
    this.dataset = {};
    this.attributes = new Map();
    this.children = [];
    this.className = "";
    this.classList = {
      values: new Set(),
      toggle: (name, enabled) => enabled
        ? this.classList.values.add(name)
        : this.classList.values.delete(name),
    };
    this.hidden = false;
    this.disabled = false;
    this.textContent = "";
    this.value = "";
    this.srcdoc = "";
    this.submit = null;
  }

  setAttribute(name, value) { this.attributes.set(name, String(value)); }
  getAttribute(name) { return this.attributes.get(name); }
  append(...children) { this.children.push(...children); }
  replaceChildren(...children) { this.children = children; }
  querySelector(selector) { return selector === "button[type=submit]" ? this.submit : null; }
}

function fakeDocument() {
  const document = { createElement: (tag) => new FakeElement(tag, document) };
  return document;
}

function elements(names, document = fakeDocument()) {
  return Object.fromEntries(names.map((name) => [name, new FakeElement("div", document)]));
}

test("navigation renders exactly one active accessible workflow", () => {
  const navigation = ["setup", "run", "transfer", "report"].map((workflow) => {
    const node = new FakeElement("button"); node.dataset.workflow = workflow; return node;
  });
  const panels = ["setup", "run", "transfer", "report"].map((workflow) => {
    const node = new FakeElement("section"); node.dataset.panel = workflow; return node;
  });
  renderNavigation({ navigation, panels }, { activeWorkflow: "transfer" });
  assert.deepEqual(navigation.map((node) => node.getAttribute("aria-current")), ["false", "false", "page", "false"]);
  assert.deepEqual(panels.map((node) => node.hidden), [true, true, false, true]);
  assert.equal(navigation.filter((node) => node.classList.values.has("active")).length, 1);
});

test("setup renderer covers editing, review, diagnostics, creating, invalid, and created states", () => {
  const document = fakeDocument();
  const e = elements([
    "setupForm", "setupReviewButton", "setupCreateButton", "setupStatus",
    "setupFeedback", "setupFeedbackMessage", "setupFeedbackDetail",
    "setupDiagnostics", "setupReviewPanel", "setupReviewStation",
    "setupReviewAntennas", "setupReviewShape", "setupReviewSlots",
  ], document);
  const state = initialState();
  renderSetup(e, state, document);
  assert.equal(e.setupStatus.textContent, "Draft");
  assert.equal(e.setupCreateButton.disabled, true);
  assert.equal(e.setupForm.getAttribute("aria-busy"), "false");

  state.setupStatus = "reviewing";
  renderSetup(e, state, document);
  assert.equal(e.setupReviewButton.textContent, "Validating…");
  assert.equal(e.setupForm.getAttribute("aria-busy"), "true");

  state.setupStatus = "invalid";
  state.setupReview = {
    diagnostics: [{ field: "station.grid", message: "Grid required", code: "grid.required" }],
    plan: null,
  };
  renderSetup(e, state, document);
  assert.equal(e.setupStatus.textContent, "Needs changes");
  assert.equal(e.setupDiagnostics.hidden, false);
  assert.equal(e.setupDiagnostics.children[0].children[1].textContent, "Grid required (grid.required)");

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
      slots: [{ sequenceNumber: 1, antennaLabel: "Dipole", direction: "transmit", band: "20m", signal: null }],
    },
  };
  renderSetup(e, state, document);
  assert.equal(e.setupCreateButton.disabled, false);
  assert.equal(e.setupReviewPanel.hidden, false);
  assert.match(e.setupReviewShape.textContent, /Whole Station Ab/);
  assert.equal(e.setupReviewSlots.children.length, 1);

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

function runElements(document) {
  const e = elements([
    "conductorPanel", "conductorEmpty", "conductorStatus", "evidenceForm",
    "conductorFeedback", "conductorFeedbackMessage", "conductorFeedbackDetail",
    "conductorLifecycle", "conductorAntennaInUse", "conductorPhase",
    "conductorGuidance", "conductorCountdown", "currentSlot", "nextSlot",
    "evidenceSlot", "evidenceAntenna", "conductorDiagnostics", "conductorEvents",
    "wsjtxForm", "wsjtxStart", "wsjtxStop", "wsjtxRequirement", "wsjtxPhase",
    "wsjtxCounts", "wsjtxDiagnostic", "wsprLivePhase", "wsprLiveDetail",
    "wsprLiveDiagnostic", "wsprLiveRetry", "wsprLiveEndWithout",
  ], document);
  e.evidenceForm.submit = new FakeElement("button", document);
  e.evidenceSlot.ownerDocument = document;
  e.evidenceAntenna.ownerDocument = document;
  e.conductorRefreshButtons = [new FakeElement("button", document)];
  e.lifecycleButtons = ["start", "arm_wspr_cycle", "end"].map((action) => {
    const button = new FakeElement("button", document); button.dataset.conductorAction = action; return button;
  });
  return e;
}

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
  const document = fakeDocument();
  const e = runElements(document);
  const state = initialState("run");
  state.conductorStatus = "ready";
  state.conductor = conductorView();
  state.wsjtx = { phase: "stopped", receivedDatagrams: 0, committedMutations: 0, ignoredDatagrams: 0 };
  let result = renderRun(e, state, document, { monotonicNow: () => 1000 });
  assert.equal(e.conductorPanel.hidden, false);
  assert.equal(e.conductorCountdown.textContent, "00:30");
  assert.match(e.nextSlot.children[0].textContent, /Transmit on Dipole/);
  assert.equal(e.lifecycleButtons[0].disabled, true, "start waits for required WSJT-X");
  assert.equal(e.evidenceSlot.children.length, 2);

  state.wsjtx = { phase: "running", bindAddress: "127.0.0.1", receivedDatagrams: 2, committedMutations: 1, ignoredDatagrams: 1 };
  state.wsprLiveAcquisitionStatus = "fetching";
  result = renderRun(e, state, document, {
    monotonicNow: () => 1000,
    countdownAnchor: result.anchor,
    countdownKey: result.key,
  });
  assert.equal(e.wsjtxPhase.textContent, "Running · 127.0.0.1");
  assert.equal(e.wsprLivePhase.textContent, "Collecting public spots…");
  assert.equal(e.wsprLiveRetry.disabled, true);

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

test("transfer renderer covers lifecycle/schema eligibility and feedback outcomes", () => {
  const e = elements([
    "openButton", "exportButton", "importWsprLiveButton", "importRbnButton",
    "transferStatus", "openFeedback", "feedbackMessage", "feedbackDetail",
    "exportFeedback", "exportFeedbackMessage", "exportFeedbackDetail",
    "importFeedback", "importFeedbackMessage", "importFeedbackDetail",
  ]);
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
  const e = elements([
    "reportStatus", "reportPlaceholder", "reportViewer", "reportFrame",
    "reportRefreshButton", "reportExportButton", "reportFeedback",
    "reportFeedbackMessage", "reportFeedbackDetail", "reportBundleName",
    "reportRevision", "reportSummary",
  ]);
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
  assert.equal(e.reportExportButton.textContent, "Exporting…");
  state.reportExportStatus = "error";
  state.reportExportError = { message: "cannot export", detail: "destination exists" };
  renderReport(e, state);
  assert.equal(e.reportFeedback.dataset.kind, "error");
  assert.equal(e.reportFeedbackDetail.textContent, "destination exists");
});
