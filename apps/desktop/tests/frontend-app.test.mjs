import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test, vi } from "vitest";

import { collectDesktopElements } from "../frontend/elements.mjs";

const DESKTOP_HTML = readFileSync(
  path.join(process.cwd(), "frontend", "index.html"),
  "utf8",
);
const DESKTOP_CSS = readFileSync(
  path.join(process.cwd(), "frontend", "styles.css"),
  "utf8",
);

function loadDesktopDocument() {
  document.open();
  document.write(DESKTOP_HTML);
  document.close();
  return collectDesktopElements(document);
}

function conductorView(overrides = {}) {
  return {
    sessionId: "session-1",
    revision: 1,
    actionToken: "token-1",
    lifecycle: "running",
    phase: "between_slots",
    antennaInUse: null,
    guidance: "Switch and confirm",
    secondsToTransition: null,
    now: "2026-07-18T14:00:00Z",
    currentSlot: null,
    nextSlot: null,
    nextIntent: {
      intentId: "intent-1",
      sequenceNumber: 1,
      direction: "transmit",
      antennaLabel: "DXC",
      band: "20m",
    },
    slots: [
      { slotId: "slot-1", sequenceNumber: 1, plannedAntenna: "DXC", band: "20m" },
    ],
    antennas: ["DXC", "Attic EFHW"],
    diagnostics: [],
    effectiveEvents: [],
    wsjtxRequired: false,
    ...overrides,
  };
}

test("controller manual review keeps native checkbox, label, and help semantics", () => {
  const elements = loadDesktopDocument();
  const checkbox = elements.setupForm.querySelector(
    '[data-setup-field="controllerManualReviewRequired"]',
  );
  const label = checkbox.closest("label");
  const help = label.nextElementSibling;

  assert.equal(label.className, "authority-confirmation");
  assert.match(label.textContent, /After each switch, wait for me to confirm the antenna is ready/);
  assert.equal(help.tagName, "SMALL");
  assert.equal(checkbox.getAttribute("aria-describedby"), help.id);
  assert.match(help.textContent, /Keep this checked for manual review/);
  assert.equal(checkbox.checked, true);
  label.click();
  assert.equal(checkbox.checked, false);
  checkbox.disabled = true;
  label.click();
  assert.equal(checkbox.checked, false);

  assert.match(DESKTOP_CSS, /\.field-grid label:not\(\.authority-confirmation\)/);
  assert.match(
    DESKTOP_CSS,
    /\.field-grid input:not\(\[type="checkbox"\]\):not\(\[type="radio"\]\)/,
  );
  assert.doesNotMatch(DESKTOP_CSS, /\.field-grid input, \.field-grid select/);
  assert.match(DESKTOP_CSS, /\.authority-confirmation:has\(input:focus-visible\)/);
});

test("the headless desktop completes location, review, and creation through mounted DOM events", async () => {
  const elements = loadDesktopDocument();
  elements.setupReviewPanel.scrollIntoView = vi.fn();
  const reportDocumentUrls = [];
  Object.defineProperty(window.URL, "createObjectURL", {
    configurable: true,
    value: () => {
      const url = `blob:headless-report-${reportDocumentUrls.length + 1}`;
      reportDocumentUrls.push(url);
      return url;
    },
  });
  Object.defineProperty(window.URL, "revokeObjectURL", {
    configurable: true,
    value: vi.fn(),
  });
  vi.useFakeTimers();
  const calls = [];
  const review = {
    valid: true,
    reviewId: "review-headless",
    diagnostics: [],
    plan: {
      station: { callsign: "N1RWJ", grid: "FN42li", powerWatts: 5 },
      antennas: [
        { label: "DXC", context: "DX Commander" },
        { label: "Attic EFHW", context: "Attic end-fed half-wave" },
      ],
      mode: "tx_focused",
      goal: "general_coverage",
      wsprLiveAcquisitionEnabled: true,
      signalPlan: null,
      antennaController: {
        profileName: "Elecraft",
        invocation: "automatic",
        manualReviewRequired: true,
      },
      scheduleReview: {
        summary: "2 directed WSPR cycles; about 4 minutes of required cycle time.",
        counterbalanceExplanation: "Successive repetitions reverse the antenna order.",
        transitionSummary: "1 antenna transition.",
        transitions: [{ summary: "Change antenna" }],
      },
      capabilities: {
        canDescribe: ["Transmit coverage differences."],
        cannotEstablish: ["A universal winner."],
      },
      slots: [
        { sequenceNumber: 1, antennaLabel: "DXC", direction: "transmit", band: "20m", signal: null },
        { sequenceNumber: 2, antennaLabel: "Attic EFHW", direction: "transmit", band: "20m", signal: null },
      ],
    },
  };
  const session = {
    bundleName: "headless.session.antennabundle",
    lifecycle: "running",
    schemaVersion: 4,
    reportHtml: "<!doctype html><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><style>body{color:#172033}</style><p>headless report</p>",
    revision: 1,
  };
  const responses = {
    load_station_preferences: null,
    antenna_controller_profiles: { inputStyle: "one_line", profiles: [] },
    request_station_location: {
      status: "success",
      latitude: 42.3601,
      longitude: -71.0589,
    },
    review_session_setup: review,
    create_session_from_review: { status: "created", session },
    refresh_active_session_report: {
      presentationId: 1,
      reportHtml: session.reportHtml,
      revision: 1,
      lifecycle: "running",
      completeness: "full_detail",
    },
    active_session_conductor: conductorView({
      lifecycle: "ready",
      phase: "ready",
      wsjtxReadiness: {
        band: "20m",
        powerWatts: 5,
        wsprLiveAcquisitionEnabled: true,
        hasReceivePeriods: true,
        nextDirection: "transmit",
      },
    }),
    mutate_active_session_conductor: conductorView({ revision: 2 }),
    active_session_antenna_controller: {
      policy: "manual",
      attached: false,
      armed: false,
      targets: {},
    },
    active_session_wsjtx_status: {
      phase: "stopped",
      receivedDatagrams: 0,
      committedMutations: 0,
      ignoredDatagrams: 0,
      setupWarnings: [],
    },
    advance_active_session_wspr_live: { status: "disabled" },
  };
  const invoke = vi.fn(async (command, payload) => {
    calls.push([command, payload]);
    assert.ok(command in responses, `unexpected native command ${command}`);
    return responses[command];
  });
  window.__TAURI__ = { core: { invoke } };
  const uncaught = [];
  const recordError = (event) => uncaught.push(event.error ?? event.reason ?? event.message);
  window.addEventListener("error", recordError);
  window.addEventListener("unhandledrejection", recordError);
  await import("../frontend/app.mjs?headless-composition");
  try {
    const callsign = elements.setupForm.querySelector('[data-setup-field="callsign"]');
    const grid = elements.setupForm.querySelector('[data-setup-field="grid"]');
    callsign.value = "n1rwj";
    callsign.dispatchEvent(new InputEvent("input", { bubbles: true }));
    grid.value = "fn42AB";
    grid.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.equal(callsign.value, "N1RWJ");
    assert.equal(grid.value, "FN42ab");

    const mode = elements.setupForm.querySelector('[data-setup-field="mode"]');
    assert.match(elements.setupPlanSummary.textContent, /16 planned WSPR cycles · about 32 minutes/);
    mode.value = "tx_focused";
    mode.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.match(elements.setupPlanSummary.textContent, /8 planned WSPR cycles · about 16 minutes/);
    mode.value = "whole_station_ab";
    mode.dispatchEvent(new InputEvent("input", { bubbles: true }));

    const rounds = elements.setupForm.querySelector('[data-setup-field="rounds"]');
    rounds.value = "3";
    rounds.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.match(elements.setupPlanSummary.textContent, /12 planned WSPR cycles · about 24 minutes/);
    rounds.value = "4";
    rounds.dispatchEvent(new InputEvent("input", { bubbles: true }));

    elements.setupAddAntennaButton.click();
    assert.match(elements.setupPlanSummary.textContent, /24 planned WSPR cycles · about 48 minutes/);
    elements.setupForm.querySelectorAll("[data-remove-antenna]")[2].click();
    assert.match(elements.setupPlanSummary.textContent, /16 planned WSPR cycles · about 32 minutes/);

    const controllerEnabled = elements.setupForm.querySelector(
      '[data-setup-field="antennaControllerEnabled"]',
    );
    controllerEnabled.checked = true;
    controllerEnabled.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.equal(elements.controllerSetupFields.hidden, false);
    assert.equal(elements.setupForm.querySelectorAll("[data-controller-target]").length, 2);
    elements.setupForm.querySelector('[data-setup-field="controllerProfileName"]').value = "Elecraft";
    elements.setupForm.querySelector('[data-setup-field="controllerSwitchCommand"]').value = "switch {target}";
    elements.setupForm.querySelector('[data-setup-field="controllerVerificationCommand"]').value = "true";
    const antennaRows = [...elements.setupForm.querySelectorAll("[data-antenna-row]")];
    antennaRows[0].querySelector('[data-antenna-field="label"]').value = "DXC";
    antennaRows[0].querySelector('[data-antenna-field="label"]').dispatchEvent(new InputEvent("input", { bubbles: true }));
    antennaRows[1].querySelector('[data-antenna-field="label"]').value = "Attic EFHW";
    antennaRows[1].querySelector('[data-antenna-field="label"]').dispatchEvent(new InputEvent("input", { bubbles: true }));
    const controllerTargets = [...elements.setupForm.querySelectorAll("[data-controller-target]")];
    controllerTargets[0].value = "2";
    controllerTargets[1].value = "1";
    assert.equal(controllerTargets[0].closest("label").textContent, "Controller value");
    elements.setupAddAntennaButton.click();
    assert.deepEqual(
      [...elements.setupForm.querySelectorAll("[data-controller-target]")].slice(0, 2).map((input) => input.value),
      ["2", "1"],
    );
    elements.setupForm.querySelectorAll("[data-remove-antenna]")[2].click();

    elements.useCurrentLocationButton.click();
    await vi.waitFor(() => {
      assert.equal(grid.value, "FN42li");
      assert.match(elements.locationStatus.textContent, /Estimated FN42li/);
    });

    const submit = new Event("submit", { bubbles: true, cancelable: true });
    assert.equal(elements.setupForm.dispatchEvent(submit), false);
    assert.equal(submit.defaultPrevented, true);
    await vi.waitFor(() => {
      assert.equal(elements.setupCreateButton.disabled, false);
      assert.equal(elements.setupReviewPanel.hidden, false);
    });
    assert.deepEqual(elements.setupReviewPanel.scrollIntoView.mock.calls, [[{
      behavior: "smooth",
      block: "start",
    }]]);
    assert.equal(callsign.value, "N1RWJ", "review preserves entered station values");
    const reviewCall = calls.find(([command]) => command === "review_session_setup");
    assert.equal(reviewCall[1].draft.station.callsign, "N1RWJ");
    assert.equal(reviewCall[1].draft.station.grid, "FN42li");
    assert.equal(reviewCall[1].draft.antennaController.profile.switchCommand.oneLine, "switch {target}");
    assert.equal(reviewCall[1].draft.antennaController.profile.verificationCommand.oneLine, "true");
    assert.deepEqual(reviewCall[1].draft.antennaController.targets, [
      { antennaLabel: "DXC", target: "2" },
      { antennaLabel: "Attic EFHW", target: "1" },
    ]);

    elements.setupCreateButton.click();
    await vi.waitFor(() => {
      assert.equal(window.location.hash, "#run");
      assert.equal(document.querySelector('[data-panel="run"]').hidden, false);
      assert.match(elements.setupStatus.textContent, /Session ready/);
    });
    assert.ok(calls.some(([command]) => command === "create_session_from_review"));
    assert.deepEqual(reportDocumentUrls, ["blob:headless-report-1"]);
    assert.equal(elements.reportFrame.getAttribute("src"), "blob:headless-report-1");

    const start = elements.lifecycleButtons.find(
      (button) => button.dataset.conductorAction === "start",
    );
    assert.equal(elements.wsjtxReadiness.hidden, false);
    assert.equal(start.disabled, true);
    start.click();
    assert.equal(calls.some(([command]) => command === "mutate_active_session_conductor"), false);
    elements.wsjtxReadinessAcknowledge.click();
    assert.equal(start.disabled, false);
    start.click();
    await vi.waitFor(() => {
      assert.ok(calls.some(([command]) => command === "mutate_active_session_conductor"));
      assert.equal(elements.wsjtxReadiness.hidden, true);
    });
    assert.deepEqual(uncaught, []);
  } finally {
    delete window.__TAURI__;
    delete window.URL.createObjectURL;
    delete window.URL.revokeObjectURL;
    window.removeEventListener("error", recordError);
    window.removeEventListener("unhandledrejection", recordError);
    vi.clearAllTimers();
    vi.useRealTimers();
  }
});
