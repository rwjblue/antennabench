import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test, vi } from "vitest";

import { collectDesktopElements } from "../frontend/elements.mjs";

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

test("the headless desktop completes location, review, and creation through mounted DOM events", async () => {
  const elements = loadDesktopDocument();
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
        summary: "2 directed WSPR cycles; ideal minimum 4 minutes.",
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
    reportHtml: "<p>headless report</p>",
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
    active_session_conductor: conductorView(),
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

    const controllerEnabled = elements.setupForm.querySelector(
      '[data-setup-field="antennaControllerEnabled"]',
    );
    controllerEnabled.checked = true;
    controllerEnabled.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.equal(elements.controllerSetupFields.hidden, false);
    assert.equal(
      elements.setupForm.querySelector("[data-controller-target-field]").hidden,
      false,
    );
    elements.setupForm.querySelector('[data-setup-field="controllerProfileName"]').value = "Elecraft";
    elements.setupForm.querySelector('[data-setup-field="controllerSwitchCommand"]').value = "switch {target}";
    elements.setupForm.querySelector('[data-setup-field="controllerVerificationCommand"]').value = "true";
    const antennaRows = [...elements.setupForm.querySelectorAll("[data-antenna-row]")];
    antennaRows[0].querySelector('[data-antenna-field="label"]').value = "DXC";
    antennaRows[0].querySelector('[data-antenna-field="controllerTarget"]').value = "2";
    antennaRows[1].querySelector('[data-antenna-field="label"]').value = "Attic EFHW";
    antennaRows[1].querySelector('[data-antenna-field="controllerTarget"]').value = "1";

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
    assert.deepEqual(uncaught, []);
  } finally {
    delete window.__TAURI__;
    window.removeEventListener("error", recordError);
    window.removeEventListener("unhandledrejection", recordError);
    vi.clearAllTimers();
    vi.useRealTimers();
  }
});
