import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { test } from "vitest";

import {
  invokeActiveSessionAntennaController,
  invokeActiveSessionConductor,
  invokeActiveSessionReport,
  invokeActiveSessionWsjtxStatus,
  invokeAdvanceSessionWsprLive,
  invokeAntennaControllerProfiles,
  invokeAttachSessionAntennaController,
  invokeCreateSessionFromReview,
  invokeDeleteAntennaControllerProfile,
  invokeExportActiveSessionReport,
  invokeExportSession,
  invokeImportActiveSessionRbn,
  invokeImportActiveSessionWsprLive,
  invokeLoadStationPreferences,
  invokeMutateSessionConductor,
  invokeOpenSession,
  invokeRefreshActiveSessionReport,
  invokeReviewSessionSetup,
  invokeRunSessionAntennaController,
  invokeSaveAntennaControllerProfile,
  invokeStartSessionWsjtx,
  invokeStationLocation,
  invokeStopSessionWsjtx,
} from "../frontend/bridge.mjs";
import {
  SETUP_QUESTION_MODES,
  applyStationPreferences,
  goalForSetupQuestion,
  modeForSetupQuestion,
  normalizeMaidenheadGrid,
  readEvidenceAction,
  readEvidenceReplacement,
  readSetupDraft,
  selectSetupQuestion,
  syncSetupQuestionToMode,
  syncWsprLiveForSignalPlan,
} from "../frontend/forms.mjs";
import {
  WORKFLOWS,
  conductorActionAvailable,
  createCountdownAnchor,
  formatActiveRunTime,
  focusSetupOutcome,
  locationLookupMessage,
  maidenheadGrid,
  projectCountdown,
  recommendedNoteTarget,
  updateReportFrame,
  viewModel,
  workflowFromHash,
  wsprLiveAcquisitionModel,
} from "../frontend/models.mjs";
import {
  beginConductorLoad,
  beginConductorMutation,
  beginExportSession,
  beginOpenSession,
  beginReportExport,
  beginReportRefresh,
  beginRbnImport,
  beginSetupCreation,
  beginSetupReview,
  beginWsjtxAction,
  beginWsprLiveAcquisition,
  beginWsprLiveImport,
  conductorLoadSucceeded,
  conductorMutationFailed,
  conductorPollSucceeded,
  editSessionSetup,
  exportSessionCancelled,
  exportSessionFailed,
  exportSessionSucceeded,
  initialState,
  openSessionCancelled,
  openSessionFailed,
  openSessionSucceeded,
  reportExportCancelled,
  reportExportSucceeded,
  reportRefreshFailed,
  reportRefreshSucceeded,
  rbnImportCancelled,
  rbnImportFailed,
  rbnImportSucceeded,
  selectWorkflow,
  setupCreationCancelled,
  setupCreationFailed,
  setupCreationSucceeded,
  setupReviewFailed,
  setupReviewSucceeded,
  wsjtxActionFailed,
  wsjtxActionSucceeded,
  wsprLiveAcquisitionFailed,
  wsprLiveAcquisitionSucceeded,
  wsprLiveImportCancelled,
  wsprLiveImportFailed,
  wsprLiveImportSucceeded,
} from "../frontend/state.mjs";

test("the desktop serves checked-in native modules without frontend tooling", () => {
  const html = readFileSync(new URL("../frontend/index.html", import.meta.url), "utf8");
  const tauri = JSON.parse(readFileSync(
    new URL("../tauri.conf.json", import.meta.url),
    "utf8",
  ));
  const packageManifest = JSON.parse(readFileSync(
    new URL("../package.json", import.meta.url),
    "utf8",
  ));
  const lock = JSON.parse(readFileSync(
    new URL("../../../package-lock.json", import.meta.url),
    "utf8",
  ));
  assert.match(html, /<script type="module" src="app\.mjs"><\/script>/);
  assert.equal(tauri.build.frontendDist, "frontend");
  assert.equal(tauri.build.beforeDevCommand, undefined);
  assert.equal(tauri.build.beforeBuildCommand, undefined);
  assert.equal(packageManifest.private, true);
  assert.equal(packageManifest.dependencies, undefined);
  assert.deepEqual(Object.keys(packageManifest.scripts).sort(), ["coverage", "test"]);
  assert.equal(existsSync(new URL("../package-lock.json", import.meta.url)), false);
  assert.equal(existsSync(new URL("../../../package-lock.json", import.meta.url)), true);
  assert.equal(lock.packages["apps/desktop"].dependencies, undefined);
  assert.deepEqual(readdirSync(new URL("../frontend", import.meta.url)).sort(), [
    "app.mjs",
    "bridge.mjs",
    "controller.mjs",
    "elements.mjs",
    "forms.mjs",
    "index.html",
    "mark.svg",
    "models.mjs",
    "renderers.mjs",
    "state.mjs",
    "styles.css",
  ]);
});

test("the shell starts in session setup", () => {
  assert.deepEqual(initialState(), {
    activeWorkflow: "setup",
    openStatus: "idle",
    session: null,
    reportPresentationId: 0,
    reportStatus: "idle",
    reportError: null,
    reportExportStatus: "idle",
    reportExportError: null,
    reportExportNotice: null,
    error: null,
    notice: null,
    exportStatus: "idle",
    exportError: null,
    exportNotice: null,
    exportedBundleName: null,
    importStatus: "idle",
    importKind: null,
    importError: null,
    importNotice: null,
    setupStatus: "editing",
    setupReview: null,
    setupError: null,
    setupNotice: null,
    conductorStatus: "idle",
    conductor: null,
    conductorError: null,
    wsjtxStatus: "idle",
    wsjtx: null,
    wsjtxError: null,
    wsprLiveAcquisitionStatus: "idle",
    wsprLiveAcquisition: null,
    wsprLiveAcquisitionError: null,
    antennaControllerStatus: "idle",
    antennaControllerCatalog: null,
    antennaController: null,
    antennaControllerError: null,
    antennaControllerOutcome: null,
    antennaControllerProfileNotice: null,
    antennaControllerProfileError: null,
  });
});

test("setup review gates creation on a valid normalized Rust plan", () => {
  const reviewing = beginSetupReview(initialState());
  const invalid = setupReviewSucceeded(reviewing, {
    valid: false,
    reviewId: null,
    diagnostics: [{ code: "bundle.semantic.invalid_required_text" }],
    plan: null,
  });
  const reviewed = setupReviewSucceeded(beginSetupReview(invalid), {
    valid: true,
    reviewId: "review-1",
    diagnostics: [],
    plan: { sessionId: "session-1", slots: [] },
  });

  assert.equal(reviewing.setupStatus, "reviewing");
  assert.equal(invalid.setupStatus, "invalid");
  assert.equal(reviewed.setupStatus, "reviewed");
  assert.equal(reviewed.setupReview.reviewId, "review-1");
  assert.equal(editSessionSetup(reviewed).setupReview, null);
});

test("setup serializes the default-on WSPR.live choice and explicit opt-out", () => {
  const setupHtml = readFileSync(new URL("../frontend/index.html", import.meta.url), "utf8");
  assert.match(
    setupHtml,
    /data-setup-field="wsprLiveAcquisitionEnabled" checked/,
  );
  assert.doesNotMatch(setupHtml, /Optional public spots/);
  assert.match(setupHtml, /Delayed public TX and RX WSPR spots are collected automatically/);
  assert.match(setupHtml, /Upload spots/);
  assert.match(setupHtml, /best-effort basis/);
  assert.match(setupHtml, /independent completeness guarantee/);
  assert.doesNotMatch(setupHtml, /unknown completeness/);
  assert.doesNotMatch(setupHtml, /data-import-authority|Confirm source authority/);
  const setupPanel = setupHtml.match(/data-panel="setup"[\s\S]*?data-panel="run"/)?.[0] ?? "";
  assert.doesNotMatch(setupPanel, /Facets|placeholder=|Trusted boundary|trusted Rust/);
  assert.doesNotMatch(setupPanel, /Deterministic schedule|startsAt|durationSeconds|guardSeconds/);
  assert.match(setupPanel, /Actual WSPR cycle times are set during the run/);
  assert.match(setupPanel, /Optional antenna details/);
  assert.match(setupPanel, /Advanced: controlled CW or RTTY signal/);
  assert.ok(setupPanel.indexOf("WSPR Spots") < setupPanel.indexOf("Advanced: controlled CW or RTTY signal"));
  assert.match(setupPanel, /One repetition tests every antenna in each direction selected by the mode/);
  assert.match(setupPanel, /Rust-calculated directed sequence, exact cycle count, and ideal minimum time/);
  assert.match(setupPanel, /This plan can describe/);
  assert.match(setupPanel, /This plan cannot establish/);
  assert.doesNotMatch(setupPanel, /SWR|return loss|impedance|resonance|cable health|tuner health|analy[sz]er/i);

  const values = new Map([
    ["callsign", "n1rwj"],
    ["grid", "FN42"],
    ["powerWatts", "5"],
    ["operatorNotes", ""],
    ["mode", "whole_station_ab"],
    ["goal", "general_coverage"],
    ["band", "20m"],
    ["rounds", "2"],
  ]);
  const publicSpots = { checked: true };
  const signalPlan = { checked: false };
  const form = {
    querySelector(selector) {
      if (selector.includes("wsprLiveAcquisitionEnabled")) return publicSpots;
      if (selector.includes("signalPlanEnabled")) return signalPlan;
      const field = selector.match(/data-setup-field="([^"]+)"/)[1];
      return { value: values.get(field) };
    },
    querySelectorAll() { return []; },
  };

  assert.equal(readSetupDraft(form).wsprLiveAcquisitionEnabled, true);
  assert.equal(readSetupDraft(form).station.callsign, "N1RWJ");
  values.set("grid", "fn42AB");
  assert.equal(readSetupDraft(form).station.grid, "FN42ab");
  publicSpots.checked = false;
  assert.equal(readSetupDraft(form).wsprLiveAcquisitionEnabled, false);
});

test("every setup question maps to the exact existing mode without entering the draft", () => {
  const values = new Map([
    ["callsign", "N1RWJ"], ["grid", "FN42"], ["powerWatts", "5"],
    ["operatorNotes", ""], ["mode", "whole_station_ab"],
    ["goal", "general_coverage"], ["band", "20m"], ["rounds", "4"],
  ]);
  const questions = Object.keys(SETUP_QUESTION_MODES).map((value) => ({ value, checked: false }));
  const form = {
    querySelector(selector) {
      if (selector.includes("signalPlanEnabled")) return { checked: false };
      if (selector.includes("wsprLiveAcquisitionEnabled")) return { checked: true };
      if (selector.includes("antennaControllerEnabled")) return { checked: false };
      const field = selector.match(/data-setup-field="([^"]+)"/)?.[1];
      return {
        get value() { return values.get(field) ?? ""; },
        set value(value) { values.set(field, value); },
      };
    },
    querySelectorAll(selector) {
      if (selector === "[data-setup-question]") return questions;
      return [];
    },
  };

  for (const [question, mode] of Object.entries(SETUP_QUESTION_MODES)) {
    values.set("mode", mode);
    values.set("goal", goalForSetupQuestion(question));
    const direct = readSetupDraft(form);
    values.set("mode", "whole_station_ab");
    values.set("goal", "dx");
    assert.equal(selectSetupQuestion(form, question), mode);
    const questionFirst = readSetupDraft(form);
    assert.deepEqual(questionFirst, direct, `${question} produces the direct-mode RPC draft`);
    assert.equal(Object.hasOwn(questionFirst, "question"), false);
    assert.equal(questions.find((choice) => choice.value === question).checked, true);
  }
  values.set("mode", "rx_focused");
  values.set("goal", "single_antenna_profiling");
  assert.equal(syncSetupQuestionToMode(form), "rx_focused");
  assert.equal(questions.find((choice) => choice.value === "hear_better").checked, true);
  assert.equal(values.get("goal"), "general_coverage");
  assert.equal(modeForSetupQuestion("profile_one_antenna"), "single_antenna_profiling");
  assert.equal(goalForSetupQuestion("profile_one_antenna"), "single_antenna_profiling");
  assert.throws(() => modeForSetupQuestion("desired_winner"), /Unknown setup question/);

  const reviewed = setupReviewSucceeded(beginSetupReview(initialState()), {
    valid: true,
    reviewId: "review-before-question-edit",
    diagnostics: [],
    plan: {},
  });
  const edited = editSessionSetup(reviewed);
  assert.equal(edited.setupStatus, "editing");
  assert.equal(edited.setupReview, null, "question or direct-mode edits stale the prior review token");
});

test("RX-only questions retain default best-effort public collection and explicit offline opt-out", () => {
  const wsprLive = { checked: true, disabled: false };
  const form = { querySelector() { return wsprLive; } };
  assert.equal(syncWsprLiveForSignalPlan(form, false), true);
  assert.equal(wsprLive.disabled, false);
  wsprLive.checked = false;
  assert.equal(syncWsprLiveForSignalPlan(form, false), false, "offline opt-out remains selected");
  wsprLive.checked = true;
  assert.equal(syncWsprLiveForSignalPlan(form, true), false);
  assert.equal(wsprLive.disabled, true, "controlled CW/RTTY is not a WSPR.live plan");
});

test("setup review focus follows successful and invalid keyboard submissions", () => {
  const review = { focusCount: 0, focus() { this.focusCount += 1; } };
  const diagnostics = { focusCount: 0, focus() { this.focusCount += 1; } };
  const invalidField = { focusCount: 0, focus() { this.focusCount += 1; } };
  const form = { querySelector() { return invalidField; } };
  assert.equal(focusSetupOutcome({ setupStatus: "reviewed" }, review, diagnostics), "review");
  assert.equal(review.focusCount, 1);
  assert.equal(focusSetupOutcome({ setupStatus: "invalid" }, review, diagnostics, form), "field");
  assert.equal(invalidField.focusCount, 1);
  assert.equal(focusSetupOutcome({ setupStatus: "invalid" }, review, diagnostics), "diagnostics");
  assert.equal(diagnostics.focusCount, 1);
  assert.equal(focusSetupOutcome({ setupStatus: "error" }, review, diagnostics), null);
});

test("question cards preserve native keyboard controls and collapse on narrow screens", () => {
  const html = readFileSync(new URL("../frontend/index.html", import.meta.url), "utf8");
  const css = readFileSync(new URL("../frontend/styles.css", import.meta.url), "utf8");
  assert.equal([...html.matchAll(/type="radio" name="setup-question"/g)].length, 4);
  assert.match(html, /data-setup-field="mode"/);
  assert.match(html, /data-setup-field="goal"/);
  assert.match(html, /data-setup-review tabindex="-1"/);
  assert.match(html, /data-setup-diagnostics tabindex="-1"/);
  assert.match(css, /@media \(max-width: 620px\)[\s\S]*\.question-choices[\s\S]*grid-template-columns: 1fr/);
  assert.match(css, /question-choices label:has\(input:focus-visible\)/);
});

test("Active Run help stays grouped with its subject across responsive layouts", () => {
  const html = readFileSync(new URL("../frontend/index.html", import.meta.url), "utf8");
  const css = readFileSync(new URL("../frontend/styles.css", import.meta.url), "utf8");
  assert.match(
    html,
    /data-skip-cycle-control hidden><button[^>]+data-conductor-action="skip_wspr_cycle"[\s\S]*?data-help-trigger="skip_cycle"/,
  );
  assert.match(html, /class="action-help-row"[\s\S]*?data-help-trigger="notes_corrections"/);
  assert.match(html, /class="help-label"[\s\S]*?data-help-trigger="current_cycle"/);
  assert.match(html, /class="help-label"[\s\S]*?data-help-trigger="next_cycle"/);
  assert.match(css, /\.action-help-row \{[^}]*align-items: center[^}]*justify-content: center/);
  assert.match(
    css,
    /@media \(max-width: 620px\)[\s\S]*\.action-help-row \{[^}]*align-items: flex-start[^}]*flex-wrap: wrap/,
  );
});

test("setup serializes explicit local controller policy, profile, and target mappings", () => {
  const setupHtml = readFileSync(new URL("../frontend/index.html", import.meta.url), "utf8");
  assert.match(setupHtml, /Antenna switching assistant/);
  assert.match(setupHtml, /Automatically after Start or Resume/);
  assert.match(setupHtml, /wait for me to confirm the antenna is ready/);
  assert.match(setupHtml, /Save profile/);
  assert.match(setupHtml, /Delete selected profile/);
  assert.match(setupHtml, /may disclose paths, addresses, usernames, or credentials/);
  assert.match(setupHtml, /Switch arguments, one per line/);

  const values = new Map([
    ["callsign", "n1rwj"], ["grid", "FN42"], ["powerWatts", "5"], ["operatorNotes", ""],
    ["mode", "tx_focused"], ["goal", "general_coverage"], ["band", "20m"], ["rounds", "1"],
    ["controllerProfileId", "profile-1"], ["controllerProfileName", "Bench switch"],
    ["controllerTimeoutSeconds", "10"],
    ["controllerInvocation", "automatic"],
    ["controllerSwitchCommand", "switch --target {target} --mode {mode}"],
    ["controllerVerificationCommand", "verify --target {target}"],
    ["controllerSwitchProgram", ""], ["controllerSwitchArguments", ""],
    ["controllerVerificationProgram", ""], ["controllerVerificationArguments", ""],
  ]);
  const antennaRows = [
    { label: "A", controllerTarget: "relay A" },
    { label: "B", controllerTarget: "relay B" },
  ].map((fields) => ({
    querySelector(selector) {
      const field = selector.match(/data-antenna-field="([^"]+)"/)?.[1];
      return field ? { value: fields[field] ?? "" } : null;
    },
  }));
  const form = {
    querySelector(selector) {
      if (selector.includes("signalPlanEnabled")) return { checked: false };
      if (selector.includes("wsprLiveAcquisitionEnabled")) return { checked: false };
      if (selector.includes("antennaControllerEnabled")) return { checked: true };
      if (selector.includes("controllerArmForSession")) return { checked: true };
      if (selector.includes("controllerManualReviewRequired")) return { checked: false };
      const field = selector.match(/data-setup-field="([^"]+)"/)?.[1];
      return { value: values.get(field) ?? "" };
    },
    querySelectorAll(selector) { return selector === "[data-antenna-row]" ? antennaRows : []; },
  };

  const draft = readSetupDraft(form);
  assert.equal(draft.antennaController.profile.profileId, "profile-1");
  assert.equal(draft.antennaController.profile.timeoutSeconds, 10);
  assert.equal(draft.antennaController.armForSession, true);
  assert.equal(draft.antennaController.invocation, "automatic");
  assert.equal(draft.antennaController.manualReviewRequired, false);
  assert.deepEqual(draft.antennaController.targets, [
    { antennaLabel: "A", target: "relay A" },
    { antennaLabel: "B", target: "relay B" },
  ]);
});

test("active run leads with task actions and hides implementation-oriented tools", () => {
  const html = readFileSync(new URL("../frontend/index.html", import.meta.url), "utf8");
  const runPanel = html.match(/<section class="workflow-panel" data-panel="run"[\s\S]*?<section class="workflow-panel" data-panel="transfer"/u)?.[0] ?? "";
  assert.match(runPanel, /Start session/);
  assert.match(runPanel, /Antenna ready/);
  assert.doesNotMatch(runPanel, /Begin antenna switch|begin_antenna_switch/);
  assert.match(runPanel, /Skip this cycle/);
  assert.match(runPanel, /Add note/);
  assert.match(runPanel, /Correct last action/);
  assert.match(runPanel, /<details[^>]*>\s*<summary><span data-wsjtx-requirement>Local\/offline receive collection/u);
  assert.match(runPanel, /<details[^>]*data-corrections-panel/u);
  assert.doesNotMatch(runPanel, /Explicit operator evidence|Trusted boundary|Trusted time/);
});

test("readiness is the only normal antenna-change action", () => {
  const between = {
    lifecycle: "running",
    phase: "between_slots",
    nextIntent: { intentId: "intent-1", antennaLabel: "Dipole" },
  };
  assert.equal(conductorActionAvailable(between, "arm_wspr_cycle"), true);
  assert.equal(conductorActionAvailable({ ...between, phase: "switching" }, "arm_wspr_cycle"), true);
  assert.equal(conductorActionAvailable({ ...between, phase: "active" }, "arm_wspr_cycle"), false);
  assert.equal(conductorActionAvailable({ ...between, phase: "awaiting_slot" }, "arm_wspr_cycle"), false);
  assert.equal(conductorActionAvailable({ ...between, lifecycle: "interrupted" }, "arm_wspr_cycle"), false);
  assert.equal(conductorActionAvailable({ ...between, nextIntent: null }, "arm_wspr_cycle"), false);
});

test("saved station details fill only an untouched setup form", () => {
  const controls = new Map([
    ["callsign", { value: "" }],
    ["grid", { value: "" }],
    ["powerWatts", { value: "" }],
    ["operatorNotes", { value: "" }],
  ]);
  const form = {
    querySelector(selector) {
      return controls.get(selector.match(/data-setup-field="([^"]+)"/)[1]);
    },
  };

  assert.equal(applyStationPreferences(form, {
    callsign: "n1rwj",
    grid: "FN42",
    powerWatts: "5",
    operatorNotes: "backyard",
  }), true);
  assert.deepEqual([...controls.values()].map(({ value }) => value), [
    "N1RWJ", "FN42", "5", "backyard",
  ]);

  controls.get("grid").value = "EM10";
  assert.equal(applyStationPreferences(form, {
    callsign: "K1ABC",
    grid: "FN31",
  }), false);
  assert.equal(controls.get("grid").value, "EM10");
});

test("native coordinates produce a canonical six-character Maidenhead grid without retaining coordinates", async () => {
  assert.equal(normalizeMaidenheadGrid("fn42AB"), "FN42ab");
  assert.equal(normalizeMaidenheadGrid("f"), "F");
  assert.equal(maidenheadGrid(42.3601, -71.0589), "FN42li");
  assert.equal(maidenheadGrid(-33.8688, 151.2093), "QF56od");
  assert.equal(maidenheadGrid(-90, -180), "AA00aa");
  assert.equal(maidenheadGrid(90, 180), "RR99xx");
  assert.throws(() => maidenheadGrid(91, 0), RangeError);
  assert.throws(() => maidenheadGrid(Number.NaN, 0), TypeError);

  const calls = [];
  const outcome = await invokeStationLocation(async (...args) => {
    calls.push(args);
    return { status: "success", latitude: 42.3601, longitude: -71.0589 };
  });
  assert.deepEqual(calls, [["request_station_location"]]);
  assert.equal(maidenheadGrid(outcome.latitude, outcome.longitude), "FN42li");
  assert.match(locationLookupMessage({ status: "denied" }), /System Settings/);
  assert.match(locationLookupMessage({ status: "restricted" }), /restricted/);
  assert.match(locationLookupMessage({ status: "unavailable" }), /unavailable/);
  assert.match(locationLookupMessage({ status: "timeout" }), /timed out/);

  const setupHtml = readFileSync(new URL("../frontend/index.html", import.meta.url), "utf8");
  assert.match(setupHtml, /data-use-current-location/);
  assert.match(setupHtml, /data-location-status aria-live="polite"/);
  const appSource = readFileSync(new URL("../frontend/app.mjs", import.meta.url), "utf8");
  assert.doesNotMatch(appSource, /navigator\.geolocation|getCurrentPosition/);
  assert.match(appSource, /Requesting macOS location permission or a one-time location/);
});

test("setup serializes an explicit typed signal plan without WSPR.live", () => {
  const values = new Map([
    ["callsign", "N1RWJ"], ["grid", "FN42"], ["powerWatts", "5"],
    ["operatorNotes", ""], ["mode", "tx_focused"], ["goal", "general_coverage"],
    ["band", "20m"], ["rounds", "2"], ["signalMode", "cw"],
    ["signalCollectionProfile", "rbn_cw_v1"], ["signalPlannedPowerWatts", "5"],
    ["signalTransmittedCallsign", "n1rwj"], ["signalMessage", "CQ N1RWJ TEST"],
    ["signalRepetitionCount", "2"], ["signalKeySpeedWpm", "20"],
    ["signalTransmitSeconds", "20"], ["signalIntervalSeconds", "30"],
    ["signalFrequenciesHz", "14050000, 14050300"],
  ]);
  const form = {
    querySelector(selector) {
      if (selector.includes("signalPlanEnabled")) return { checked: true };
      if (selector.includes("signalDifferingIdentityValidated")) return { checked: false };
      if (selector.includes("wsprLiveAcquisitionEnabled")) return { checked: false };
      const field = selector.match(/data-setup-field="([^"]+)"/)[1];
      return { value: values.get(field) };
    },
    querySelectorAll() { return []; },
  };

  const draft = readSetupDraft(form);
  assert.equal(draft.wsprLiveAcquisitionEnabled, false);
  assert.deepEqual(draft.signalPlan, {
    mode: "cw",
    collectionProfile: "rbn_cw_v1",
    plannedPowerWatts: "5",
    transmittedCallsign: "N1RWJ",
    differingIdentityValidated: false,
    message: "CQ N1RWJ TEST",
    repetitionCount: "2",
    keySpeedWpm: "20",
    transmitSeconds: "20",
    intervalSeconds: "30",
    frequenciesHz: "14050000, 14050300",
  });
});

test("setup creation cancellation, failure, and success preserve coherent state", () => {
  const reviewed = setupReviewSucceeded(beginSetupReview(initialState()), {
    valid: true,
    reviewId: "review-1",
    diagnostics: [],
    plan: { sessionId: "session-1", slots: [] },
  });
  const creating = beginSetupCreation(reviewed);
  const cancelled = setupCreationCancelled(creating);
  const failed = setupCreationFailed(creating, {
    kind: "destination",
    message: "Destination exists.",
    detail: "/tmp/existing.session.antennabundle",
  });
  const session = {
    sessionId: "session-1",
    bundleName: "created.session.antennabundle",
    reportHtml: "<!doctype html>",
  };
  const created = setupCreationSucceeded(creating, session);

  assert.equal(cancelled.setupStatus, "reviewed");
  assert.equal(cancelled.setupReview.reviewId, "review-1");
  assert.equal(failed.setupStatus, "reviewed");
  assert.equal(failed.setupError.kind, "destination");
  assert.equal(created.setupStatus, "created");
  assert.equal(created.activeWorkflow, "run");
  assert.equal(created.session, session);
  assert.equal(created.openStatus, "ready");
  assert.equal(created.reportPresentationId, 1);
});

test("the conductor retains its coherent view through refresh, mutation, and typed failure", () => {
  const session = { sessionId: "session-1" };
  const opened = openSessionSucceeded(initialState(), session);
  const loading = beginConductorLoad(opened);
  const conductor = {
    sessionId: "session-1",
    revision: 4,
    lifecycle: "running",
    actionToken: "mutation-4",
  };
  const ready = conductorLoadSucceeded(loading, conductor);
  const silentlyPolled = conductorPollSucceeded({
    ...ready,
    conductorNotice: "Session started.",
  }, { ...conductor, now: "2026-07-15T20:00:05Z" });
  const mutating = beginConductorMutation(ready, "start");
  const saved = conductorLoadSucceeded(mutating, { ...conductor, revision: 5 });
  const failed = conductorMutationFailed(beginConductorMutation(saved, "arm_wspr_cycle"), {
    kind: "stale_revision",
    message: "The session changed.",
    detail: "expected 4, actual 5",
  });

  assert.equal(loading.conductorStatus, "loading");
  assert.equal(ready.conductorStatus, "ready");
  assert.equal(ready.conductor, conductor);
  assert.equal(silentlyPolled.conductorStatus, "ready");
  assert.equal(silentlyPolled.conductorNotice, "Session started.");
  assert.equal(mutating.conductorStatus, "mutating");
  assert.equal(mutating.conductorPendingAction, "start");
  assert.equal(saved.conductorStatus, "ready");
  assert.equal(saved.conductorNotice, "Session started.");
  assert.equal(failed.conductorStatus, "error");
  assert.equal(failed.conductor, saved.conductor);
  assert.equal(failed.conductorError.kind, "stale_revision");
});

test("active countdown projects from a disposable Rust anchor", () => {
  const anchor = createCountdownAnchor({
    sessionId: "session-1",
    revision: 4,
    actionToken: "action-4",
    lifecycle: "running",
    phase: "active",
    secondsToTransition: 5,
    currentSlot: { slotId: "intent-1" },
    nextSlot: null,
  }, 10_000);
  assert.equal(projectCountdown(anchor, 10_000), 5);
  assert.equal(projectCountdown(anchor, 10_999), 5);
  assert.equal(projectCountdown(anchor, 11_000), 4);
  assert.equal(projectCountdown(anchor, 15_900), 0);
  assert.equal(projectCountdown(anchor, 9_000), 5);
  assert.equal(projectCountdown(anchor, 99_000), 0);
  assert.equal(createCountdownAnchor({ secondsToTransition: null }, 0), null);
});

test("active cycle times are concise local labels without ISO repetition", () => {
  const today = formatActiveRunTime("2026-07-16T13:34:01Z", {
    now: "2026-07-16T15:00:00Z",
    locale: "en-US",
    timeZone: "UTC",
  });
  const priorDay = formatActiveRunTime("2026-07-15T13:34:01Z", {
    now: "2026-07-16T15:00:00Z",
    locale: "en-US",
    timeZone: "UTC",
  });
  assert.equal(today, "1:34 PM");
  assert.equal(priorDay, "Jul 15, 1:34 PM");
  assert.doesNotMatch(today, /2026|T|Z/);
});

test("run notes recommend the current or most recently completed cycle", () => {
  const slot1 = {
    slotId: "cycle-1",
    startsAt: "2026-07-16T13:34:01Z",
    endsAt: "2026-07-16T13:35:51.592Z",
  };
  const slot2 = {
    slotId: "cycle-2",
    startsAt: "2026-07-16T13:36:01Z",
    endsAt: "2026-07-16T13:37:51.592Z",
  };
  const base = {
    now: "2026-07-16T13:36:30Z",
    slots: [slot1, slot2],
    currentSlot: slot1,
    nextSlot: slot2,
  };
  assert.equal(recommendedNoteTarget({ ...base, phase: "ready", currentSlot: null, nextSlot: null, slots: [] }), "");
  assert.equal(recommendedNoteTarget({ ...base, phase: "awaiting_slot" }), "cycle-2");
  assert.equal(recommendedNoteTarget({ ...base, phase: "active", currentSlot: slot2 }), "cycle-2");
  for (const phase of ["between_slots", "switching", "interrupted", "finalizing", "complete", "ended", "abandoned"]) {
    assert.equal(recommendedNoteTarget({ ...base, phase }), "cycle-1");
  }
  assert.equal(recommendedNoteTarget({
    ...base,
    phase: "interrupted",
    now: "2026-07-16T13:34:30Z",
    slots: [slot1],
    currentSlot: slot1,
  }), "cycle-1");
  assert.equal(recommendedNoteTarget(null), "");
});

test("the conductor bridge exposes only bounded read and focused mutation commands", async () => {
  const calls = [];
  const invoke = async (...args) => {
    calls.push(args);
    return { revision: args.length === 1 ? 4 : 5 };
  };
  const request = {
    actionToken: "mutation-4",
    expectedRevision: 4,
    action: {
      kind: "confirm_antenna",
      slotId: "slot-1",
      antennaLabel: "Loop",
      note: "operator confirmed",
    },
  };

  const view = await invokeActiveSessionConductor(invoke);
  const updated = await invokeMutateSessionConductor(invoke, request);

  assert.deepEqual(calls, [
    ["active_session_conductor"],
    ["mutate_active_session_conductor", { request }],
  ]);
  assert.equal(view.revision, 4);
  assert.equal(updated.revision, 5);
});

test("antenna-controller bridge separates local profile writes from narrow active-run intent", async () => {
  const calls = [];
  const invoke = async (...args) => {
    calls.push(args);
    return {};
  };
  const draft = {
    profileId: null,
    name: "Bench switch",
    timeoutSeconds: 10,
    switchCommand: { oneLine: "switch {target}", program: "", arguments: [] },
    verificationCommand: null,
  };
  const attach = {
    profileId: "profile-1",
    profileRevision: "revision-1",
    targets: [{ antennaLabel: "A", target: "relay-a" }],
    armed: true,
  };
  const run = { actionToken: "token-4", expectedRevision: 4, intentId: "intent-1" };

  await invokeAntennaControllerProfiles(invoke);
  await invokeSaveAntennaControllerProfile(invoke, draft);
  await invokeDeleteAntennaControllerProfile(invoke, "profile-1");
  await invokeActiveSessionAntennaController(invoke);
  await invokeAttachSessionAntennaController(invoke, attach);
  await invokeRunSessionAntennaController(invoke, run);

  assert.deepEqual(calls, [
    ["antenna_controller_profiles"],
    ["save_antenna_controller_profile", { draft }],
    ["delete_antenna_controller_profile", { profileId: "profile-1" }],
    ["active_session_antenna_controller"],
    ["attach_active_session_antenna_controller", { request: attach }],
    ["run_active_session_antenna_controller", { request: run }],
  ]);
  assert.deepEqual(Object.keys(run).sort(), ["actionToken", "expectedRevision", "intentId"]);
});

test("signal confirmations preserve explicit actual facts for append and correction", () => {
  const signal = {
    frequencyHz: 14050000,
    mode: "cw",
    powerWatts: 5,
    transmittedCallsign: "k1abc/b",
    cadenceFollowed: false,
  };

  assert.deepEqual(
    readEvidenceAction("confirm_signal", "slot-3", "", "slower than planned", signal),
    {
      kind: "confirm_signal",
      slotId: "slot-3",
      ...signal,
      transmittedCallsign: "K1ABC/B",
      note: "slower than planned",
    },
  );
  assert.deepEqual(
    readEvidenceReplacement("confirm_signal", "", "corrected", signal),
    {
      kind: "confirm_signal",
      ...signal,
      transmittedCallsign: "K1ABC/B",
      note: "corrected",
    },
  );
});

test("WSJT-X state and bridge stay focused on status plus start/stop intent", async () => {
  const calls = [];
  const invoke = async (...args) => {
    calls.push(args);
    return { phase: args[0] === "stop_active_session_wsjtx" ? "stopped" : "running" };
  };
  const request = {
    bindAddress: "127.0.0.1",
    port: 2237,
    expectedClientId: "WSJT-X",
  };

  const refreshing = beginWsjtxAction(initialState("run"));
  const ready = wsjtxActionSucceeded(refreshing, await invokeActiveSessionWsjtxStatus(invoke));
  const started = wsjtxActionSucceeded(
    beginWsjtxAction(ready, "starting"),
    await invokeStartSessionWsjtx(invoke, request),
  );
  const stopped = wsjtxActionSucceeded(
    beginWsjtxAction(started, "stopping"),
    await invokeStopSessionWsjtx(invoke),
  );
  const failed = wsjtxActionFailed(beginWsjtxAction(stopped), {
    kind: "resource",
    message: "Receiver failed.",
    detail: "wsjtx.receiver.bind_failed",
  });

  assert.deepEqual(calls, [
    ["active_session_wsjtx_status"],
    ["start_active_session_wsjtx", { request }],
    ["stop_active_session_wsjtx"],
  ]);
  assert.equal(started.wsjtx.phase, "running");
  assert.equal(stopped.wsjtx.phase, "stopped");
  assert.equal(failed.wsjtxError.kind, "resource");
});

test("automatic WSPR.live acquisition remains typed and accepts only retry intent", async () => {
  const calls = [];
  const invoke = async (...args) => {
    calls.push(args);
    return {
      status: "waiting",
      completedSlotId: "slot-1",
      notBefore: "2026-07-15T20:07:00Z",
      capturedThrough: null,
    };
  };
  const active = openSessionSucceeded(initialState(), { sessionId: "session-1" });
  const fetching = beginWsprLiveAcquisition(active);
  const outcome = await invokeAdvanceSessionWsprLive(invoke);
  const waiting = wsprLiveAcquisitionSucceeded(fetching, outcome);
  const completedSession = { sessionId: "session-1", lifecycle: "ended" };
  const completed = wsprLiveAcquisitionSucceeded(fetching, {
    status: "completed",
    session: completedSession,
    revision: 9,
    capturedThrough: "2026-07-15T20:12:00Z",
  });
  const failed = wsprLiveAcquisitionFailed(fetching, {
    kind: "resource",
    message: "WSPR.live spots could not be fetched.",
    detail: "offline",
  });
  await invokeAdvanceSessionWsprLive(invoke, true);

  assert.equal(fetching.wsprLiveAcquisitionStatus, "fetching");
  assert.equal(waiting.wsprLiveAcquisition.status, "waiting");
  assert.equal(completed.session, completedSession);
  assert.equal(failed.wsprLiveAcquisitionError.detail, "offline");
  assert.deepEqual(calls, [
    ["advance_active_session_wspr_live", { request: { retry: false } }],
    ["advance_active_session_wspr_live", { request: { retry: true } }],
  ]);
});

test("active-run public spot states keep operator copy plain and expose diagnostic detail", () => {
  const conductor = {
    phase: "between_slots",
    now: "2026-07-16T15:00:00Z",
  };
  const opaque = "b75f805a-2476-47ff-99cb-7e0a26a96ab1";
  const states = [
    { wsprLiveAcquisitionStatus: "fetching", conductor },
    { wsprLiveAcquisitionError: { message: "WSPR.live is unavailable.", detail: opaque }, conductor },
    { wsprLiveAcquisition: { status: "disabled", providerId: opaque }, conductor },
    { wsprLiveAcquisition: { status: "dormant", completedSlotId: opaque }, conductor },
    { wsprLiveAcquisition: { status: "waiting", completedSlotId: opaque, notBefore: "2026-07-16T15:05:00Z" }, conductor },
    { wsprLiveAcquisition: { status: "up_to_date", capturedThrough: "2026-07-16T15:04:00Z", requestWindow: opaque }, conductor },
    { wsprLiveAcquisition: { status: "captured", capturedThrough: "2026-07-16T15:04:00Z", observationsCreated: 3, duplicate: 1, conflict: 0, segmentId: opaque }, conductor },
    { wsprLiveAcquisition: { status: "completed", capturedThrough: "2026-07-16T15:04:00Z", providerId: opaque }, conductor },
    { wsprLiveAcquisition: { status: "failed", message: "Collection stopped.", detail: opaque }, conductor },
  ];
  const presentations = states.map(wsprLiveAcquisitionModel);
  for (const [index, presentation] of presentations.entries()) {
    if (![1, 8].includes(index)) {
      assert.doesNotMatch(JSON.stringify(presentation), new RegExp(opaque));
    }
    assert.doesNotMatch(
      `${presentation.phase} ${presentation.detail}`,
      /becomes eligible|authorize the preceding segment|overlap earlier windows/i,
    );
  }
  assert.deepEqual(presentations.map(({ phase }) => phase), [
    "Collecting best-effort public spots…",
    "Public collection needs attention",
    "Automatic collection is off",
    "Waiting for the first completed cycle",
    "Waiting briefly for public spots",
    "Best-effort public collection completed",
    "Best-effort public collection completed",
    "Best-effort public collection completed",
    "Public collection needs attention",
  ]);
  assert.deepEqual(presentations.map(({ compact }) => compact.kind), [
    "checking",
    "error",
    "offline",
    "waiting",
    "waiting",
    "success",
    "success",
    "success",
    "error",
  ]);
  assert.match(presentations[3].compact.text, /first completed cycle/);
  assert.match(presentations[4].compact.text, /Waiting for ingestion/);
  assert.match(presentations[5].compact.text, /Last configured-window check succeeded/);
  assert.match(presentations[6].compact.text, /3 new spot\(s\) retained/);
  assert.doesNotMatch(
    presentations.map(({ compact }) => compact.text).join(" "),
    /better|worse|winner|outperform/i,
  );
  const zeroSpots = wsprLiveAcquisitionModel({
    wsprLiveAcquisition: {
      status: "captured",
      capturedThrough: "2026-07-16T15:04:00Z",
      checkedAt: "2026-07-16T15:09:00Z",
      observationsCreated: 0,
    },
    conductor,
  });
  assert.match(zeroSpots.compact.text, /no new matching spots yet/);
  assert.match(presentations[5].detail, /configured request windows/);
  assert.match(presentations[5].detail, /independent completeness guarantee/);
  assert.match(presentations[8].detail, /Collection stopped/);
  assert.equal(presentations[1].diagnostic, opaque);
  assert.equal(presentations[8].diagnostic, opaque);
  assert.equal(presentations[8].retry, true);
  assert.doesNotMatch(JSON.stringify(presentations), /unknown completeness|completeness is unknown/i);

  const finalizationFailure = wsprLiveAcquisitionModel({
    wsprLiveAcquisition: {
      status: "failed",
      message: "Public spots were saved, but the session could not end automatically.",
      detail: "current lifecycle: Running",
    },
    conductor: { ...conductor, phase: "finalizing" },
  });
  assert.equal(finalizationFailure.diagnostic, "current lifecycle: Running");
  assert.equal(finalizationFailure.retry, true);
  assert.equal(finalizationFailure.endWithout, true);

  const html = readFileSync(new URL("../frontend/index.html", import.meta.url), "utf8");
  const runPanel = html.match(/data-panel="run"[\s\S]*?data-panel="transfer"/u)?.[0] ?? "";
  assert.doesNotMatch(runPanel, /data-conductor-revision|data-conductor-now|Checkpoint|Current time/);
  assert.match(runPanel, /data-conductor-lifecycle|data-conductor-antenna-in-use/);
  assert.match(runPanel, /<button type="button" data-conductor-refresh>Refresh<\/button>/);
});

test("setup bridge exposes only location, review, preferences, and reviewed creation", async () => {
  const calls = [];
  const invoke = async (...args) => {
    calls.push(args);
    return args[0] === "review_session_setup"
      ? { valid: true, reviewId: "review-1" }
      : { status: "created", session: { sessionId: "session-1" } };
  };
  const draft = { station: {}, antennas: [], schedule: {} };

  const review = await invokeReviewSessionSetup(invoke, draft);
  await invokeStationLocation(invoke);
  await invokeLoadStationPreferences(invoke);
  const created = await invokeCreateSessionFromReview(invoke, review.reviewId);

  assert.deepEqual(calls, [
    ["review_session_setup", { draft }],
    ["request_station_location"],
    ["load_station_preferences"],
    ["create_session_from_review", { reviewId: "review-1" }],
  ]);
  assert.equal(created.status, "created");
});

test("setup native errors remain typed and editable", () => {
  const failed = setupReviewFailed(beginSetupReview(initialState()), {
    kind: "resource",
    message: "The setup review is too large.",
    detail: "resource.desktop.ipc_bytes",
  });

  assert.equal(failed.setupStatus, "error");
  assert.equal(failed.setupError.kind, "resource");
  assert.equal(editSessionSetup(failed).setupStatus, "editing");
});

test("opening a session transitions through loading and ready", () => {
  const loading = beginOpenSession(initialState("transfer"));
  const session = { sessionId: "session-1", reportHtml: "<!doctype html>" };
  const ready = openSessionSucceeded(loading, session);

  assert.equal(loading.openStatus, "loading");
  assert.equal(ready.openStatus, "ready");
  assert.equal(ready.activeWorkflow, "report");
  assert.equal(ready.session, session);
  assert.equal(ready.reportPresentationId, 1);
});

test("same-ID successful opens refresh only the new report presentation", () => {
  const reportFrame = { dataset: {}, srcdoc: "" };
  const first = openSessionSucceeded(initialState("transfer"), {
    sessionId: "session-1",
    reportHtml: "<!doctype html><title>first</title>",
  });

  assert.equal(updateReportFrame(reportFrame, first), true);
  assert.equal(reportFrame.srcdoc, "<!doctype html><title>first</title>");

  const navigated = selectWorkflow(first, "transfer");
  const exporting = beginExportSession(navigated);
  const exported = exportSessionSucceeded(exporting, "session-1-copy.session.wsprabundle");
  assert.equal(updateReportFrame(reportFrame, navigated), false);
  assert.equal(updateReportFrame(reportFrame, exporting), false);
  assert.equal(updateReportFrame(reportFrame, exported), false);

  const cancelled = openSessionCancelled(beginOpenSession(first));
  const failed = openSessionFailed(beginOpenSession(first), {
    kind: "validation",
    message: "The replacement bundle did not pass validation.",
    detail: "invalid station data",
  });
  assert.equal(updateReportFrame(reportFrame, cancelled), false);
  assert.equal(updateReportFrame(reportFrame, failed), false);
  assert.equal(reportFrame.srcdoc, "<!doctype html><title>first</title>");

  const second = openSessionSucceeded(beginOpenSession(first), {
    sessionId: "session-1",
    reportHtml: "<!doctype html><title>second</title>",
  });
  assert.equal(second.session.sessionId, first.session.sessionId);
  assert.notEqual(second.reportPresentationId, first.reportPresentationId);
  assert.equal(updateReportFrame(reportFrame, second), true);
  assert.equal(reportFrame.srcdoc, "<!doctype html><title>second</title>");
});

test("revision-keyed report refresh retains coherent prior output on failure and export state", () => {
  const frame = { dataset: {}, srcdoc: "" };
  const opened = openSessionSucceeded(initialState("report"), {
    sessionId: "session-1",
    bundleName: "session.antennabundle",
  });
  const first = reportRefreshSucceeded(beginReportRefresh(opened), {
    presentationId: 11,
    revision: 4,
    lifecycle: "running",
    completeness: "full_detail",
    reportHtml: "<!doctype html><title>revision 4</title>",
  });
  assert.equal(updateReportFrame(frame, first), true);

  const exporting = beginReportExport(first);
  const cancelled = reportExportCancelled(exporting);
  const exported = reportExportSucceeded(exporting, {
    fileName: "snapshot.html",
    revision: 4,
    format: "compact_summary_html",
  });
  assert.match(exported.reportExportNotice, /compact summary/);
  assert.equal(updateReportFrame(frame, exporting), false);
  assert.equal(updateReportFrame(frame, cancelled), false);
  assert.equal(updateReportFrame(frame, exported), false);

  const failed = reportRefreshFailed(beginReportRefresh(first), {
    kind: "stale_revision",
    message: "The session kept changing.",
    detail: "retry",
  });
  assert.equal(updateReportFrame(frame, failed), false);
  assert.equal(failed.session.reportHtml, first.session.reportHtml);

  const second = reportRefreshSucceeded(beginReportRefresh(failed), {
    presentationId: 12,
    revision: 5,
    lifecycle: "ended",
    completeness: "bounded_overview",
    reportHtml: "<!doctype html><title>revision 5</title>",
  });
  assert.equal(updateReportFrame(frame, second), true);
  assert.equal(frame.srcdoc, "<!doctype html><title>revision 5</title>");
});

test("cancelling the native picker is a normal non-error transition", () => {
  const cancelled = openSessionCancelled(beginOpenSession(initialState("transfer")));

  assert.equal(cancelled.openStatus, "idle");
  assert.equal(cancelled.notice, "cancelled");
  assert.equal(cancelled.error, null);
});

test("typed native failures retain friendly and technical context", () => {
  const failed = openSessionFailed(beginOpenSession(initialState("transfer")), {
    kind: "validation",
    message: "The session bundle did not pass validation.",
    detail: "2 validation issues",
  });

  assert.equal(failed.openStatus, "error");
  assert.deepEqual(failed.error, {
    kind: "validation",
    message: "The session bundle did not pass validation.",
    detail: "2 validation issues",
  });
});

test("exporting has independent loading and success state", () => {
  const active = openSessionSucceeded(initialState("transfer"), {
    sessionId: "session-1",
  });
  const loading = beginExportSession(active);
  const exported = exportSessionSucceeded(
    loading,
    "session-1-copy.session.wsprabundle",
  );

  assert.equal(loading.exportStatus, "loading");
  assert.equal(loading.session, active.session);
  assert.equal(exported.exportStatus, "ready");
  assert.equal(
    exported.exportedBundleName,
    "session-1-copy.session.wsprabundle",
  );
  assert.equal(exported.session, active.session);
});

test("export cancellation and typed errors do not replace the active session", () => {
  const active = openSessionSucceeded(initialState("transfer"), {
    sessionId: "session-1",
  });
  const cancelled = exportSessionCancelled(beginExportSession(active));
  const failed = exportSessionFailed(beginExportSession(active), {
    kind: "destination",
    message: "A file or directory already exists at that destination.",
    detail: "/tmp/existing.session.wsprabundle",
  });

  assert.equal(cancelled.exportStatus, "idle");
  assert.equal(cancelled.exportNotice, "cancelled");
  assert.equal(cancelled.session, active.session);
  assert.equal(failed.exportStatus, "error");
  assert.equal(failed.exportError.kind, "destination");
  assert.equal(failed.session, active.session);
});

test("WSPR.live import preserves focused state and refreshes the active summary", () => {
  const active = openSessionSucceeded(initialState("transfer"), {
    sessionId: "session-1",
    lifecycle: "running",
    observationCount: 2,
    reportHtml: "old",
  });
  const loading = beginWsprLiveImport(active);
  const imported = wsprLiveImportSucceeded(loading, {
    status: "imported",
    revision: 8,
    observationsCreated: 3,
    session: {
      sessionId: "session-1",
      lifecycle: "running",
      revision: 8,
      observationCount: 5,
    },
  });
  const cancelled = wsprLiveImportCancelled(beginWsprLiveImport(active));
  const failed = wsprLiveImportFailed(beginWsprLiveImport(active), {
    kind: "validation",
    message: "Invalid response.",
    detail: "wrong projection",
  });

  assert.equal(loading.importStatus, "loading");
  assert.equal(imported.importStatus, "ready");
  assert.equal(imported.session.observationCount, 5);
  assert.equal(imported.session.reportHtml, null);
  assert.equal(imported.reportStatus, "unavailable");
  assert.equal(cancelled.importNotice, "cancelled");
  assert.equal(failed.importError.kind, "validation");
  assert.equal(failed.session, active.session);
});

test("RBN import preserves focused state and refreshes the schema-v3 summary", () => {
  const active = openSessionSucceeded(initialState("transfer"), {
    sessionId: "session-1",
    schemaVersion: 3,
    lifecycle: "ended",
    observationCount: 2,
    reportHtml: "old",
  });
  const loading = beginRbnImport(active);
  const imported = rbnImportSucceeded(loading, {
    status: "imported",
    revision: 9,
    observationsCreated: 2,
    session: {
      sessionId: "session-1",
      schemaVersion: 3,
      lifecycle: "ended",
      revision: 9,
      observationCount: 4,
    },
  });
  const cancelled = rbnImportCancelled(beginRbnImport(active));
  const failed = rbnImportFailed(beginRbnImport(active), {
    kind: "validation",
    message: "Invalid RBN archive.",
    detail: "header drift",
  });

  assert.equal(loading.importKind, "rbn");
  assert.equal(loading.importStatus, "loading");
  assert.equal(imported.importStatus, "ready");
  assert.equal(imported.session.observationCount, 4);
  assert.equal(imported.session.reportHtml, null);
  assert.equal(cancelled.importNotice, "cancelled");
  assert.equal(failed.importError.kind, "validation");
  assert.equal(failed.session, active.session);
});

test("the frontend invokes only the narrow session commands", async () => {
  const calls = [];
  const invoke = async (...args) => {
    calls.push(args);
    return args[0] === "open_session_bundle"
      ? { status: "opened", session: { sessionId: "session-1" } }
      : args[0] === "active_session_report"
        ? "<!doctype html>"
        : { status: "exported", bundleName: "session-1-copy.session.wsprabundle" };
  };

  const result = await invokeOpenSession(invoke);
  const report = await invokeActiveSessionReport(invoke);
  const exported = await invokeExportSession(invoke);
  const refreshed = await invokeRefreshActiveSessionReport(invoke);
  const reportExported = await invokeExportActiveSessionReport(invoke, "full_evidence_html");
  const imported = await invokeImportActiveSessionWsprLive(invoke);
  const rbnImported = await invokeImportActiveSessionRbn(invoke);

  assert.deepEqual(calls, [
    ["open_session_bundle"],
    ["active_session_report"],
    ["export_active_session"],
    ["refresh_active_session_report"],
    ["export_active_session_report", { format: "full_evidence_html" }],
    ["import_active_session_wspr_live"],
    ["import_active_session_rbn"],
  ]);
  assert.equal(result.status, "opened");
  assert.equal(report, "<!doctype html>");
  assert.equal(exported.status, "exported");
  assert.equal(refreshed.status, "exported");
  assert.equal(reportExported.status, "exported");
  assert.equal(imported.status, "exported");
  assert.equal(rbnImported.status, "exported");
});

test("each declared workflow can become active without mutating prior state", () => {
  let state = initialState();

  for (const workflow of WORKFLOWS) {
    const previous = state;
    state = selectWorkflow(state, workflow);

    assert.equal(state.activeWorkflow, workflow);
    assert.equal(previous.activeWorkflow, WORKFLOWS[WORKFLOWS.indexOf(workflow) - 1] ?? "setup");
  }
});

test("selecting the current workflow is an idempotent transition", () => {
  const state = initialState("report");
  assert.equal(selectWorkflow(state, "report"), state);
});

test("unknown workflow transitions are rejected", () => {
  assert.throws(
    () => selectWorkflow(initialState(), "settings"),
    /Unknown desktop workflow: settings/,
  );
});

test("hash routing falls back to setup for unsupported values", () => {
  assert.equal(workflowFromHash("#transfer"), "transfer");
  assert.equal(workflowFromHash("#settings"), "setup");
  assert.equal(workflowFromHash(""), "setup");
});

test("the view model marks exactly one workflow active", () => {
  const model = viewModel(initialState("run"));
  assert.deepEqual(
    model.filter(({ active }) => active).map(({ workflow }) => workflow),
    ["run"],
  );
});
