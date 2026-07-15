import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  WORKFLOWS,
  beginConductorLoad,
  beginConductorMutation,
  beginExportSession,
  beginOpenSession,
  beginReportExport,
  beginReportRefresh,
  beginSetupCreation,
  beginSetupReview,
  beginWsjtxAction,
  beginWsprLiveAcquisition,
  beginWsprLiveImport,
  conductorLoadSucceeded,
  conductorMutationFailed,
  editSessionSetup,
  exportSessionCancelled,
  exportSessionFailed,
  exportSessionSucceeded,
  initialState,
  invokeActiveSessionReport,
  invokeActiveSessionConductor,
  invokeCreateSessionFromReview,
  invokeExportSession,
  invokeExportActiveSessionReport,
  invokeImportActiveSessionWsprLive,
  invokeOpenSession,
  invokeRefreshActiveSessionReport,
  invokeReviewSessionSetup,
  invokeMutateSessionConductor,
  invokeActiveSessionWsjtxStatus,
  invokeStartSessionWsjtx,
  invokeStopSessionWsjtx,
  invokeAdvanceSessionWsprLive,
  openSessionCancelled,
  openSessionFailed,
  openSessionSucceeded,
  reportExportCancelled,
  reportExportSucceeded,
  reportRefreshFailed,
  reportRefreshSucceeded,
  readSetupDraft,
  selectWorkflow,
  setupCreationCancelled,
  setupCreationFailed,
  setupCreationSucceeded,
  setupReviewFailed,
  setupReviewSucceeded,
  updateReportFrame,
  viewModel,
  workflowFromHash,
  wsjtxActionFailed,
  wsjtxActionSucceeded,
  wsprLiveAcquisitionFailed,
  wsprLiveAcquisitionSucceeded,
  wsprLiveImportCancelled,
  wsprLiveImportFailed,
  wsprLiveImportSucceeded,
} from "../frontend/app.mjs";

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

  const values = new Map([
    ["callsign", "N1RWJ"],
    ["grid", "FN42"],
    ["powerWatts", "5"],
    ["operatorNotes", ""],
    ["mode", "whole_station_ab"],
    ["goal", "general_coverage"],
    ["startsAt", ""],
    ["band", "20m"],
    ["durationSeconds", "120"],
    ["guardSeconds", "10"],
    ["rounds", "2"],
  ]);
  const publicSpots = { checked: true };
  const form = {
    querySelector(selector) {
      if (selector.includes("wsprLiveAcquisitionEnabled")) return publicSpots;
      const field = selector.match(/data-setup-field="([^"]+)"/)[1];
      return { value: values.get(field) };
    },
    querySelectorAll() { return []; },
  };

  assert.equal(readSetupDraft(form).wsprLiveAcquisitionEnabled, true);
  publicSpots.checked = false;
  assert.equal(readSetupDraft(form).wsprLiveAcquisitionEnabled, false);
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
  const mutating = beginConductorMutation(ready);
  const failed = conductorMutationFailed(mutating, {
    kind: "stale_revision",
    message: "The session changed.",
    detail: "expected 4, actual 5",
  });

  assert.equal(loading.conductorStatus, "loading");
  assert.equal(ready.conductorStatus, "ready");
  assert.equal(ready.conductor, conductor);
  assert.equal(mutating.conductorStatus, "mutating");
  assert.equal(failed.conductorStatus, "error");
  assert.equal(failed.conductor, conductor);
  assert.equal(failed.conductorError.kind, "stale_revision");
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

test("setup bridge exposes only review and reviewed-creation commands", async () => {
  const calls = [];
  const invoke = async (...args) => {
    calls.push(args);
    return args[0] === "review_session_setup"
      ? { valid: true, reviewId: "review-1" }
      : { status: "created", session: { sessionId: "session-1" } };
  };
  const draft = { station: {}, antennas: [], schedule: {} };

  const review = await invokeReviewSessionSetup(invoke, draft);
  const created = await invokeCreateSessionFromReview(invoke, review.reviewId);

  assert.deepEqual(calls, [
    ["review_session_setup", { draft }],
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
  });
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
  const reportExported = await invokeExportActiveSessionReport(invoke);
  const imported = await invokeImportActiveSessionWsprLive(invoke);

  assert.deepEqual(calls, [
    ["open_session_bundle"],
    ["active_session_report"],
    ["export_active_session"],
    ["refresh_active_session_report"],
    ["export_active_session_report"],
    ["import_active_session_wspr_live", { request: { authorityConfirmed: true } }],
  ]);
  assert.equal(result.status, "opened");
  assert.equal(report, "<!doctype html>");
  assert.equal(exported.status, "exported");
  assert.equal(refreshed.status, "exported");
  assert.equal(reportExported.status, "exported");
  assert.equal(imported.status, "exported");
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
