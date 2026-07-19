import assert from "node:assert/strict";
import { test } from "vitest";

import { createDesktopController } from "../frontend/controller.mjs";
import { initialState, openSessionSucceeded } from "../frontend/state.mjs";

const session = (overrides = {}) => ({
  bundleName: "test.session.antennabundle",
  lifecycle: "running",
  schemaVersion: 4,
  reportHtml: "<p>prior</p>",
  revision: 3,
  ...overrides,
});

const conductor = (overrides = {}) => ({
  actionToken: "action-1",
  revision: 3,
  lifecycle: "running",
  phase: "between_slots",
  ...overrides,
});

function harness(responses = {}, options = {}) {
  const calls = [];
  const renders = [];
  const navigations = [];
  const invoke = async (command, payload) => {
    calls.push([command, payload]);
    const response = responses[command];
    if (response instanceof Error) throw response;
    return typeof response === "function" ? response(payload, calls) : response;
  };
  const controller = createDesktopController({
    state: options.state,
    invoke,
    render: (state) => renders.push(state),
    navigate: (workflow) => navigations.push(workflow),
    ...options.effects,
  });
  return { controller, calls, renders, navigations };
}

test("copy support summary uses the redacted bounded projection and reports clipboard failure", async () => {
  const copied = [];
  const state = openSessionSucceeded(initialState("report"), session({
    operationalHistory: { supportSummary: "{\"privacy\":\"redacted\"}" },
  }));
  const success = harness({}, {
    state,
    effects: { copyText: async (value) => copied.push(value) },
  });
  await success.controller.copySupportSummary();
  assert.deepEqual(copied, ["{\"privacy\":\"redacted\"}"]);
  assert.equal(success.controller.state.supportCopyStatus, "copied");

  const failed = harness({}, {
    state,
    effects: { copyText: async () => { throw new Error("clipboard denied"); } },
  });
  await failed.controller.copySupportSummary();
  assert.equal(failed.controller.state.supportCopyStatus, "error");
  assert.match(failed.controller.state.supportCopyError.detail, /clipboard denied/);
});

test("the controller composes setup review outcomes and reviewed creation", async () => {
  const invalid = harness({
    review_session_setup: { valid: false, reviewId: "review-invalid" },
  });
  await invalid.controller.reviewSetup({ station: {} });
  assert.equal(invalid.controller.state.setupStatus, "invalid");

  const failed = harness({ review_session_setup: new Error("review failed") });
  await failed.controller.reviewSetup({ station: {} });
  assert.equal(failed.controller.state.setupStatus, "error");
  assert.match(failed.controller.state.setupError.detail, /review failed/);

  const cancelled = harness({
    review_session_setup: { valid: true, reviewId: "review-1" },
    create_session_from_review: { status: "cancelled" },
  });
  await cancelled.controller.reviewSetup({ station: {} });
  await cancelled.controller.createSession();
  assert.equal(cancelled.controller.state.setupStatus, "reviewed");
  assert.equal(cancelled.controller.state.setupNotice, "cancelled");

  const creationFailed = harness({
    review_session_setup: { valid: true, reviewId: "review-2" },
    create_session_from_review: new Error("creation failed"),
  });
  await creationFailed.controller.reviewSetup({ station: {} });
  await creationFailed.controller.createSession();
  assert.equal(creationFailed.controller.state.setupStatus, "reviewed");
  assert.match(creationFailed.controller.state.setupError.detail, /creation failed/);

  const created = harness({
    review_session_setup: { valid: true, reviewId: "review-3" },
    create_session_from_review: {
      status: "created",
      session: session(),
      managedLocation: {
        locatorId: "locator-created",
        bundleName: "test.session.antennabundle",
        origin: "managed",
        originLabel: "Saved by AntennaBench",
      },
    },
    refresh_active_session_report: {
      presentationId: 4,
      reportHtml: "<p>fresh</p>",
      revision: 4,
      lifecycle: "running",
      completeness: "full_detail",
    },
    active_session_conductor: conductor({ revision: 4 }),
    active_session_antenna_controller: { policy: "manual", attached: false, armed: false, targets: {} },
    active_session_wsjtx_status: { phase: "stopped" },
    advance_active_session_wspr_live: { status: "disabled" },
  });
  await created.controller.reviewSetup({ station: {} });
  await created.controller.createSession();
  assert.equal(created.controller.state.setupStatus, "created");
  assert.equal(created.controller.state.managedLocationNotice.locatorId, "locator-created");
  assert.deepEqual(created.navigations, ["run"]);
  assert.deepEqual(created.calls.map(([command]) => command), [
    "review_session_setup",
    "create_session_from_review",
    "refresh_active_session_report",
    "active_session_conductor",
    "active_session_antenna_controller",
    "active_session_wsjtx_status",
    "advance_active_session_wspr_live",
  ]);
});

test("controller profiles can be explicitly saved and deleted during setup", async () => {
  const savedProfile = {
    profileId: "profile-1",
    revision: "revision-1",
    name: "Bench switch",
  };
  let profiles = [savedProfile];
  const run = harness({
    save_antenna_controller_profile: savedProfile,
    delete_antenna_controller_profile: undefined,
    antenna_controller_profiles: () => ({ inputStyle: "one_line", profiles }),
  });

  assert.equal(
    await run.controller.saveAntennaControllerProfile({ profileId: null, name: "Bench switch" }),
    savedProfile,
  );
  assert.deepEqual(run.controller.state.antennaControllerProfileNotice, {
    kind: "saved",
    profileId: "profile-1",
  });

  profiles = [];
  assert.equal(await run.controller.deleteAntennaControllerProfile("profile-1"), true);
  assert.deepEqual(run.controller.state.antennaControllerProfileNotice, {
    kind: "deleted",
    profileId: "",
  });
  assert.deepEqual(run.calls.map(([command]) => command), [
    "save_antenna_controller_profile",
    "antenna_controller_profiles",
    "delete_antenna_controller_profile",
    "antenna_controller_profiles",
  ]);
});

test("saved sessions load on entry, retain stale rows, and own sessionless routing", async () => {
  const catalog = {
    status: "complete",
    entries: [{ locatorId: "locator-1", bundleName: "one.session.antennabundle" }],
    diagnostics: [],
  };
  let loads = 0;
  const run = harness({
    list_managed_sessions: () => {
      loads += 1;
      if (loads === 2) throw new Error("temporary catalog failure");
      return catalog;
    },
  });

  await run.controller.selectWorkflow("saved");
  assert.equal(run.controller.state.managedCatalog, catalog);
  assert.equal(run.controller.state.catalogStatus, "ready");

  await run.controller.loadManagedSessions();
  assert.equal(run.controller.state.managedCatalog, catalog);
  assert.equal(run.controller.state.catalogStatus, "error");
  assert.match(run.controller.state.catalogError.detail, /temporary catalog failure/);

  await run.controller.routeWorkflow("report");
  assert.equal(run.controller.state.activeWorkflow, "saved");
  assert.deepEqual(run.navigations, ["saved"]);
});

test("saved-session reveal actions use opaque locators and isolate row failures", async () => {
  const state = initialState("saved");
  state.catalogStatus = "ready";
  state.managedCatalog = {
    status: "complete",
    entries: [{ locatorId: "locator-1", bundleName: "one.session.antennabundle" }],
    diagnostics: [],
  };
  const revealed = harness({
    reveal_managed_sessions_directory: undefined,
    reveal_managed_session: undefined,
  }, { state });
  await revealed.controller.revealManagedSessionsDirectory();
  await revealed.controller.revealManagedSession("locator-1");
  assert.deepEqual(revealed.calls, [
    ["reveal_managed_sessions_directory", undefined],
    ["reveal_managed_session", { locatorId: "locator-1" }],
  ]);

  const failed = harness({ open_managed_session: new Error("bundle moved") }, { state });
  await failed.controller.openManagedSession("locator-1", "report");
  assert.equal(failed.controller.state.managedCatalog, state.managedCatalog);
  assert.match(failed.controller.state.catalogRowError.error.detail, /bundle moved/);
});

test("saved-session deletion is row-scoped, single-submit, refreshed, and failure-safe", async () => {
  const entry = {
    locatorId: "locator-1",
    callsign: "N1RWJ",
    bundleName: "one.session.antennabundle",
  };
  const other = { locatorId: "locator-2", bundleName: "two.session.antennabundle" };
  const state = initialState("saved");
  state.catalogStatus = "ready";
  state.managedCatalog = { status: "complete", entries: [entry, other], diagnostics: [] };
  let finishDelete;
  const deleting = harness({
    delete_managed_session: () => new Promise((resolve) => { finishDelete = resolve; }),
    list_managed_sessions: { status: "complete", entries: [other], diagnostics: [] },
  }, { state });
  deleting.controller.requestManagedSessionDeletion(entry);
  const first = deleting.controller.deleteManagedSession();
  const repeated = deleting.controller.deleteManagedSession();
  assert.equal(deleting.controller.state.catalogDeleteStatus, "deleting");
  assert.deepEqual(deleting.controller.state.managedCatalog.entries, [entry, other]);
  finishDelete({ status: "trashed", bundleName: entry.bundleName });
  await Promise.all([first, repeated]);
  assert.deepEqual(deleting.calls, [
    ["delete_managed_session", { locatorId: "locator-1" }],
    ["list_managed_sessions", undefined],
  ]);
  assert.deepEqual(deleting.controller.state.managedCatalog.entries, [other]);
  assert.equal(deleting.controller.state.catalogDeleteNotice, entry.bundleName);

  const failed = harness({ delete_managed_session: new Error("Trash unavailable") }, { state });
  failed.controller.requestManagedSessionDeletion(entry);
  await failed.controller.deleteManagedSession();
  assert.equal(failed.controller.state.catalogDeleteStatus, "failed");
  assert.equal(failed.controller.state.managedCatalog, state.managedCatalog);
  assert.match(failed.controller.state.catalogDeleteError.detail, /Trash unavailable/);
});

test("open cancellation, failure, and success retain focused state and refresh reports", async () => {
  const cancelled = harness({ open_session_bundle: { status: "cancelled" } });
  await cancelled.controller.openSessionFromAnotherLocation();
  assert.equal(cancelled.controller.state.openStatus, "idle");
  assert.equal(cancelled.controller.state.notice, "cancelled");

  const failed = harness({ open_session_bundle: new Error("cannot open") });
  await failed.controller.openSessionFromAnotherLocation();
  assert.equal(failed.controller.state.openStatus, "error");
  assert.match(failed.controller.state.error.detail, /cannot open/);

  const opened = harness({
    open_session_bundle: { status: "opened", session: session({ lifecycle: "ended" }) },
    refresh_active_session_report: {
      presentationId: 5,
      reportHtml: "<p>opened</p>",
      revision: 5,
      lifecycle: "ended",
      completeness: "full_detail",
    },
  });
  await opened.controller.openSessionFromAnotherLocation();
  assert.deepEqual(opened.navigations, ["report"]);
  assert.deepEqual(opened.calls.map(([command]) => command), [
    "open_session_bundle",
    "refresh_active_session_report",
  ]);
  assert.equal(opened.controller.state.session.reportHtml, "<p>opened</p>");
});

test("managed opening obeys explicit report and work intents from the fresh summary", async () => {
  for (const lifecycle of ["ready", "running", "interrupted", "ended", "abandoned", null]) {
    const reportOnly = harness({
      open_managed_session: { status: "opened", session: session({ lifecycle }) },
      refresh_active_session_report: {
        presentationId: 4,
        reportHtml: "<p>report only</p>",
        revision: lifecycle === null ? null : 3,
        lifecycle,
        completeness: "full_detail",
      },
    });
    await reportOnly.controller.openManagedSession(`locator-${lifecycle}`, "report");
    assert.deepEqual(reportOnly.navigations, ["report"]);
    assert.deepEqual(reportOnly.calls, [
      ["open_managed_session", { locatorId: `locator-${lifecycle}` }],
      ["refresh_active_session_report", undefined],
    ]);
    assert.equal(reportOnly.controller.state.conductor, null);
  }

  for (const lifecycle of ["ready", "interrupted"]) {
    const work = harness({
      open_managed_session: { status: "opened", session: session({ lifecycle }) },
      refresh_active_session_report: {
        presentationId: 4,
        reportHtml: `<p>${lifecycle}</p>`,
        revision: 3,
        lifecycle,
        completeness: "full_detail",
      },
      active_session_conductor: conductor({ lifecycle }),
      active_session_antenna_controller: {
        policy: "manual",
        attached: false,
        armed: false,
        targets: {},
      },
      active_session_wsjtx_status: { phase: "stopped" },
    });
    await work.controller.openManagedSession(`locator-${lifecycle}`, "work");
    assert.deepEqual(work.navigations, ["run"]);
    assert.deepEqual(work.calls.map(([command]) => command), [
      "open_managed_session",
      "refresh_active_session_report",
      "active_session_conductor",
      "active_session_antenna_controller",
      "active_session_wsjtx_status",
    ]);
    assert.equal(work.calls.some(([command]) => command === "mutate_active_session_conductor"), false);
  }

  let reportCount = 0;
  const recoveredWork = harness({
    open_managed_session: {
      status: "opened",
      session: session({ lifecycle: "running", revision: 3 }),
    },
    refresh_active_session_report: () => {
      reportCount += 1;
      return {
        presentationId: 4 + reportCount,
        reportHtml: `<p>revision ${reportCount === 1 ? 3 : 4}</p>`,
        revision: reportCount === 1 ? 3 : 4,
        lifecycle: reportCount === 1 ? "running" : "interrupted",
        completeness: "full_detail",
      };
    },
    active_session_conductor: conductor({ lifecycle: "interrupted", revision: 4 }),
    active_session_antenna_controller: {
      policy: "manual",
      attached: false,
      armed: false,
      targets: {},
    },
    active_session_wsjtx_status: { phase: "stopped" },
  });
  await recoveredWork.controller.openManagedSession("locator-work", "work");
  assert.deepEqual(recoveredWork.navigations, ["run"]);
  assert.deepEqual(recoveredWork.calls.map(([command]) => command), [
    "open_managed_session",
    "refresh_active_session_report",
    "active_session_conductor",
    "active_session_antenna_controller",
    "active_session_wsjtx_status",
    "refresh_active_session_report",
  ]);
  assert.equal(recoveredWork.controller.state.session.lifecycle, "interrupted");
  assert.equal(recoveredWork.controller.state.session.revision, 4);
  assert.match(recoveredWork.controller.state.session.reportHtml, /revision 4/);
});

test("terminal work requests redirect to report without loading run services", async () => {
  const run = harness({
    open_managed_session: {
      status: "opened",
      session: session({ lifecycle: "ended", revision: 8 }),
    },
    refresh_active_session_report: {
      presentationId: 8,
      reportHtml: "<p>ended</p>",
      revision: 8,
      lifecycle: "ended",
      completeness: "full_detail",
    },
  });

  await run.controller.openManagedSession("locator-ended", "work");

  assert.deepEqual(run.navigations, ["report"]);
  assert.equal(run.controller.state.notice, "work_redirected");
  assert.deepEqual(run.calls.map(([command]) => command), [
    "open_managed_session",
    "refresh_active_session_report",
  ]);
});

test("the external picker defaults fresh resumable sessions to work", async () => {
  const run = harness({
    open_session_bundle: { status: "opened", session: session({ lifecycle: "ready" }) },
    refresh_active_session_report: {
      presentationId: 3,
      reportHtml: "<p>ready</p>",
      revision: 3,
      lifecycle: "ready",
      completeness: "full_detail",
    },
    active_session_conductor: conductor({ lifecycle: "ready" }),
    active_session_antenna_controller: {
      policy: "manual",
      attached: false,
      armed: false,
      targets: {},
    },
    active_session_wsjtx_status: { phase: "stopped" },
  });

  await run.controller.openSessionFromAnotherLocation();

  assert.deepEqual(run.navigations, ["run"]);
  assert.equal(run.controller.state.openIntent, "work");
  assert.deepEqual(run.calls.map(([command]) => command), [
    "open_session_bundle",
    "refresh_active_session_report",
    "active_session_conductor",
    "active_session_antenna_controller",
    "active_session_wsjtx_status",
  ]);
});

test("the external picker defaults terminal and legacy sessions to report", async () => {
  for (const lifecycle of ["ended", "abandoned", null]) {
    const run = harness({
      open_session_bundle: { status: "opened", session: session({ lifecycle }) },
      refresh_active_session_report: {
        presentationId: 3,
        reportHtml: "<p>historical</p>",
        revision: lifecycle === null ? null : 3,
        lifecycle,
        completeness: "full_detail",
      },
    });
    await run.controller.openSessionFromAnotherLocation();
    assert.deepEqual(run.navigations, ["report"]);
    assert.equal(run.controller.state.openIntent, "report");
    assert.deepEqual(run.calls.map(([command]) => command), [
      "open_session_bundle",
      "refresh_active_session_report",
    ]);
  }
});

test("one opening state machine serializes managed and picker requests", async () => {
  let resolveManaged;
  const run = harness({
    open_managed_session: () => new Promise((resolve) => { resolveManaged = resolve; }),
    refresh_active_session_report: {
      presentationId: 1,
      reportHtml: "<p>ended</p>",
      revision: 3,
      lifecycle: "ended",
      completeness: "full_detail",
    },
  });

  const first = run.controller.openManagedSession("locator-1", "report");
  const second = run.controller.openSessionFromAnotherLocation();
  assert.deepEqual(run.calls, [["open_managed_session", { locatorId: "locator-1" }]]);
  resolveManaged({ status: "opened", session: session({ lifecycle: "ended" }) });
  await Promise.all([first, second]);
  assert.equal(run.calls.filter(([command]) => command === "open_session_bundle").length, 0);
  await assert.rejects(
    () => run.controller.openManagedSession("locator-1", "edit"),
    /Unknown session opening intent/,
  );
});

test("open, report, and conductor foreground operations do not overlap", async () => {
  const prior = openSessionSucceeded(initialState("report"), session({ lifecycle: "ended" }));
  let resolveReport;
  const reportFirst = harness({
    refresh_active_session_report: () => new Promise((resolve) => { resolveReport = resolve; }),
  }, { state: prior });
  const refresh = reportFirst.controller.refreshReport();
  await reportFirst.controller.openSessionFromAnotherLocation();
  assert.deepEqual(reportFirst.calls.map(([command]) => command), [
    "refresh_active_session_report",
  ]);
  resolveReport({
    presentationId: 5,
    reportHtml: "<p>fresh</p>",
    revision: 3,
    lifecycle: "ended",
    completeness: "full_detail",
  });
  await refresh;

  let resolveOpen;
  const openFirst = harness({
    open_managed_session: () => new Promise((resolve) => { resolveOpen = resolve; }),
  }, { state: prior });
  const opening = openFirst.controller.openManagedSession("locator-1", "report");
  await Promise.all([
    openFirst.controller.refreshReport(),
    openFirst.controller.refreshConductor(),
  ]);
  assert.deepEqual(openFirst.calls.map(([command]) => command), ["open_managed_session"]);
  resolveOpen({ status: "cancelled" });
  await opening;
});

test("cancelled and failed replacement opens preserve the prior presentation", async () => {
  const priorSession = session({ lifecycle: "ended", reportHtml: "<p>prior</p>" });
  const prior = openSessionSucceeded(initialState("report"), priorSession);
  prior.activeWorkflow = "transfer";
  const cancelled = harness({ open_session_bundle: { status: "cancelled" } }, { state: prior });
  await cancelled.controller.openSessionFromAnotherLocation();
  assert.equal(cancelled.controller.state.session, priorSession);
  assert.equal(cancelled.controller.state.activeWorkflow, "transfer");
  assert.equal(cancelled.controller.state.reportPresentationId, prior.reportPresentationId);
  assert.deepEqual(cancelled.calls.map(([command]) => command), ["open_session_bundle"]);

  const failed = harness({ open_managed_session: new Error("changed") }, { state: prior });
  await failed.controller.openManagedSession("locator-stale", "report");
  assert.equal(failed.controller.state.session, priorSession);
  assert.equal(failed.controller.state.activeWorkflow, "transfer");
  assert.equal(failed.controller.state.reportPresentationId, prior.reportPresentationId);
  assert.deepEqual(failed.calls.map(([command]) => command), ["open_managed_session"]);
});

test("conductor mutations serialize follow-up adapter, acquisition, and report refreshes", async () => {
  const state = openSessionSucceeded(initialState("run"), session());
  state.activeWorkflow = "run";
  state.conductorStatus = "ready";
  state.conductor = conductor();
  const run = harness({
    mutate_active_session_conductor: conductor({ revision: 4 }),
    active_session_wsjtx_status: { phase: "running" },
    advance_active_session_wspr_live: { status: "up_to_date", capturedThrough: "2026-07-16T23:00:00Z" },
    refresh_active_session_report: {
      presentationId: 4,
      reportHtml: "<p>revision 4</p>",
      revision: 4,
      lifecycle: "running",
      completeness: "full_detail",
    },
  }, { state });

  await run.controller.submitConductorAction({ kind: "add_note", slotId: null, note: "test" });
  assert.deepEqual(run.calls.map(([command]) => command), [
    "mutate_active_session_conductor",
    "active_session_wsjtx_status",
    "advance_active_session_wspr_live",
    "refresh_active_session_report",
  ]);
  assert.deepEqual(run.calls[0][1].request, {
    actionToken: "action-1",
    expectedRevision: 3,
    action: { kind: "add_note", slotId: null, note: "test" },
  });

  let resolveMutation;
  const pending = harness({
    mutate_active_session_conductor: () => new Promise((resolve) => { resolveMutation = resolve; }),
  }, { state });
  const first = pending.controller.submitConductorAction({ kind: "start", note: null });
  const second = pending.controller.submitConductorAction({ kind: "start", note: null });
  assert.equal(pending.calls.length, 1);
  resolveMutation(conductor());
  await Promise.all([first, second]);

  const mutationFailed = harness({
    mutate_active_session_conductor: new Error("stale action"),
  }, { state });
  await mutationFailed.controller.submitConductorAction({ kind: "start", note: null });
  assert.equal(mutationFailed.controller.state.conductorStatus, "error");
  assert.match(mutationFailed.controller.state.conductorError.detail, /stale action/);
  assert.equal(mutationFailed.calls.length, 1);

  const loadFailed = harness({ active_session_conductor: new Error("cannot recover") }, { state });
  await loadFailed.controller.refreshConductor();
  assert.equal(loadFailed.controller.state.conductorStatus, "error");
  assert.match(loadFailed.controller.state.conductorError.detail, /cannot recover/);
});

test("WSPR.live, WSJT-X, and report failures preserve coherent state", async () => {
  const state = openSessionSucceeded(initialState("run"), session());
  state.activeWorkflow = "run";
  state.conductorStatus = "ready";
  state.conductor = conductor({ phase: "finalizing" });
  const run = harness({
    advance_active_session_wspr_live: new Error("mirror unavailable"),
    start_active_session_wsjtx: { phase: "running" },
    stop_active_session_wsjtx: { phase: "stopped" },
    refresh_active_session_report: new Error("render failed"),
    export_active_session_report: new Error("export failed"),
  }, { state });

  await run.controller.advanceWsprLive(true);
  assert.equal(run.calls[0][1].request.retry, true);
  assert.equal(run.controller.state.wsprLiveAcquisitionStatus, "error");
  await run.controller.startWsjtx({ bindAddress: "127.0.0.1", port: 2237, expectedClientId: "WSJT-X" });
  await run.controller.stopWsjtx();
  assert.equal(run.controller.state.wsjtx.phase, "stopped");
  await run.controller.refreshReport();
  assert.equal(run.controller.state.session.reportHtml, "<p>prior</p>");
  assert.equal(run.controller.state.reportStatus, "ready");
  await run.controller.exportReport("full_evidence_html", "omitted_at_export");
  assert.equal(run.controller.state.reportExportStatus, "error");
  assert.deepEqual(
    run.calls.find(([command]) => command === "export_active_session_report")[1],
    {
      format: "full_evidence_html",
      controllerEvidence: "omitted_at_export",
      operationalHistory: "omitted",
    },
  );

  const completed = harness({
    advance_active_session_wspr_live: { status: "completed", session: session({ lifecycle: "ended" }) },
    active_session_conductor: conductor({ lifecycle: "ended", phase: "complete" }),
    active_session_antenna_controller: { policy: "manual", attached: false, armed: false, targets: {} },
    active_session_wsjtx_status: { phase: "stopped" },
    refresh_active_session_report: {
      presentationId: 6,
      reportHtml: "<p>complete</p>",
      revision: 6,
      lifecycle: "ended",
      completeness: "full_detail",
    },
  }, { state });
  await completed.controller.advanceWsprLive(true);
  assert.deepEqual(completed.calls.map(([command]) => command), [
    "advance_active_session_wspr_live",
    "active_session_conductor",
    "active_session_antenna_controller",
    "active_session_wsjtx_status",
    "refresh_active_session_report",
  ]);
  assert.equal(completed.controller.state.session.reportHtml, "<p>complete</p>");
});

test("background refresh does not duplicate an in-flight final WSPR.live acquisition", async () => {
  const state = openSessionSucceeded(initialState("run"), session());
  state.activeWorkflow = "run";
  state.conductorStatus = "ready";
  state.conductor = conductor({ phase: "finalizing" });
  let resolveAcquisition;
  const run = harness({
    advance_active_session_wspr_live: () => new Promise((resolve) => {
      resolveAcquisition = resolve;
    }),
    active_session_conductor: conductor({ phase: "finalizing" }),
    active_session_antenna_controller: { policy: "manual", attached: false, armed: false, targets: {} },
    active_session_wsjtx_status: { phase: "running" },
    refresh_active_session_report: {
      presentationId: 7,
      reportHtml: "<p>ended</p>",
      revision: 7,
      lifecycle: "ended",
      completeness: "full_detail",
    },
  }, { state });

  const acquisition = run.controller.advanceWsprLive();
  await new Promise((resolve) => setImmediate(resolve));
  await run.controller.refreshConductor();
  assert.equal(
    run.calls.filter(([command]) => command === "advance_active_session_wspr_live").length,
    1,
  );

  resolveAcquisition({
    status: "completed",
    session: session({ lifecycle: "ended", revision: 7 }),
    revision: 7,
    capturedThrough: "2026-07-16T23:00:00Z",
  });
  await acquisition;
  assert.equal(
    run.calls.filter(([command]) => command === "advance_active_session_wspr_live").length,
    1,
  );
  assert.equal(run.controller.state.session.reportHtml, "<p>ended</p>");
});

test("report background checks are silent, change-aware, and bounded by lifecycle", async () => {
  const endedState = openSessionSucceeded(initialState("report"), session({
    lifecycle: "ended",
    presentationId: 7,
    revision: 7,
  }));
  const ended = harness({
    refresh_active_session_report: {
      presentationId: 7,
      reportHtml: "<p>prior</p>",
      revision: 7,
      lifecycle: "ended",
      completeness: "full_detail",
    },
  }, { state: endedState });

  ended.controller.periodicRefresh();
  ended.controller.periodicRefresh();
  ended.controller.periodicRefresh();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(ended.calls.length, 0, "terminal reports are not polled by the timer");
  assert.equal(ended.renders.length, 0);

  ended.controller.refreshOnReturn();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(ended.calls.length, 1, "focus performs one external-change check");
  assert.equal(ended.renders.length, 0, "an unchanged presentation has no visible state transition");
  assert.equal(ended.controller.state, endedState);

  const runningState = openSessionSucceeded(initialState("report"), session({
    lifecycle: "running",
    presentationId: 10,
    revision: 10,
  }));
  let presentationId = 10;
  const running = harness({
    refresh_active_session_report: () => ({
      presentationId,
      reportHtml: `<p>revision ${presentationId}</p>`,
      revision: presentationId,
      lifecycle: "running",
      completeness: "full_detail",
    }),
  }, { state: runningState });

  running.controller.periodicRefresh();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(running.calls.length, 1);
  assert.equal(running.renders.length, 0);
  assert.equal(running.controller.state, runningState);

  presentationId = 11;
  running.controller.periodicRefresh();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(running.controller.state.reportPresentationId, 11);
  assert.equal(running.controller.state.session.reportHtml, "<p>revision 11</p>");
  assert.equal(running.renders.length, 1, "a new coherent presentation is committed once");

  running.controller.periodicRefresh();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(running.renders.length, 1, "subsequent no-op checks stay silent");

  const failed = harness({
    refresh_active_session_report: new Error("externally changed bundle is invalid"),
  }, { state: runningState });
  await failed.controller.refreshReport(true);
  assert.equal(failed.controller.state.session.reportHtml, "<p>prior</p>");
  assert.equal(failed.controller.state.reportStatus, "ready");
  assert.match(failed.controller.state.reportError.detail, /externally changed bundle is invalid/);
  assert.equal(failed.renders.length, 1, "background failures remain visible and typed");
});

test("manual report refresh retains visible progress and queues behind a silent check", async () => {
  const state = openSessionSucceeded(initialState("report"), session({
    lifecycle: "running",
    presentationId: 3,
    revision: 3,
  }));
  const resolvers = [];
  const run = harness({
    refresh_active_session_report: () => new Promise((resolve) => resolvers.push(resolve)),
  }, { state });

  const silent = run.controller.refreshReport(true);
  const manual = run.controller.refreshReport();
  assert.equal(run.controller.state.reportStatus, "ready");
  assert.equal(run.renders.length, 0);
  resolvers.shift()({
    presentationId: 3,
    reportHtml: "<p>prior</p>",
    revision: 3,
    lifecycle: "running",
    completeness: "full_detail",
  });
  await silent;
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(run.controller.state.reportStatus, "refreshing");
  assert.equal(run.renders.length, 1);
  resolvers.shift()({
    presentationId: 3,
    reportHtml: "<p>prior</p>",
    revision: 3,
    lifecycle: "running",
    completeness: "full_detail",
  });
  await manual;
  assert.equal(run.controller.state.reportStatus, "ready");
  assert.equal(run.renders.length, 2);
});

test("focus, visibility, periodic, countdown, and disposal use injected lifecycle ports", async () => {
  const intervals = [];
  const cleared = [];
  const listeners = {};
  const countdowns = [];
  let disposalCount = 0;
  let now = 2000;
  let visible = true;
  const state = openSessionSucceeded(initialState("run"), session());
  state.activeWorkflow = "run";
  state.conductorStatus = "ready";
  state.conductor = conductor();
  const run = harness({
    active_session_conductor: conductor(),
    active_session_wsjtx_status: { phase: "running" },
    advance_active_session_wspr_live: { status: "disabled" },
  }, {
    state,
    effects: {
      setInterval(callback, milliseconds) {
        const timer = { callback, milliseconds };
        intervals.push(timer);
        return timer;
      },
      clearInterval: (timer) => cleared.push(timer),
      onFocus(callback) { listeners.focus = callback; return () => { listeners.focus = null; }; },
      onVisibilityChange(callback) { listeners.visibility = callback; return () => { listeners.visibility = null; }; },
      onHashChange(callback) { listeners.hash = callback; return () => { listeners.hash = null; }; },
      isVisible: () => visible,
      monotonicNow: () => now,
      getCountdownAnchor: () => ({ key: "cycle-1", seconds: 1, sampledAtMilliseconds: 1000 }),
      renderCountdown: (seconds) => countdowns.push(seconds),
      onDispose: () => { disposalCount += 1; },
    },
  });

  run.controller.start();
  run.controller.start();
  assert.deepEqual(intervals.map(({ milliseconds }) => milliseconds), [5000, 1000]);
  run.controller.tickCountdown();
  run.controller.tickCountdown();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(countdowns, [0]);
  assert.equal(run.calls.filter(([command]) => command === "active_session_conductor").length, 1);

  visible = false;
  listeners.visibility();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(run.calls.filter(([command]) => command === "active_session_conductor").length, 1);
  visible = true;
  listeners.focus();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(run.calls.filter(([command]) => command === "active_session_conductor").length, 2);

  now = 2500;
  intervals[0].callback();
  await new Promise((resolve) => setImmediate(resolve));
  assert.ok(run.calls.some(([command]) => command === "active_session_conductor"));
  run.controller.dispose();
  run.controller.dispose();
  assert.equal(cleared.length, 2);
  assert.equal(listeners.focus, null);
  assert.equal(listeners.visibility, null);
  assert.equal(listeners.hash, null);
  assert.equal(disposalCount, 1);
});
