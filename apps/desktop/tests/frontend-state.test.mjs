import assert from "node:assert/strict";
import test from "node:test";

import {
  WORKFLOWS,
  beginExportSession,
  beginOpenSession,
  exportSessionCancelled,
  exportSessionFailed,
  exportSessionSucceeded,
  initialState,
  invokeActiveSessionReport,
  invokeExportSession,
  invokeOpenSession,
  openSessionCancelled,
  openSessionFailed,
  openSessionSucceeded,
  selectWorkflow,
  viewModel,
  workflowFromHash,
} from "../frontend/app.mjs";

test("the shell starts in session setup", () => {
  assert.deepEqual(initialState(), {
    activeWorkflow: "setup",
    openStatus: "idle",
    session: null,
    error: null,
    notice: null,
    exportStatus: "idle",
    exportError: null,
    exportNotice: null,
    exportedBundleName: null,
  });
});

test("opening a session transitions through loading and ready", () => {
  const loading = beginOpenSession(initialState("transfer"));
  const session = { sessionId: "session-1", reportHtml: "<!doctype html>" };
  const ready = openSessionSucceeded(loading, session);

  assert.equal(loading.openStatus, "loading");
  assert.equal(ready.openStatus, "ready");
  assert.equal(ready.activeWorkflow, "report");
  assert.equal(ready.session, session);
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

  assert.deepEqual(calls, [
    ["open_session_bundle"],
    ["active_session_report"],
    ["export_active_session"],
  ]);
  assert.equal(result.status, "opened");
  assert.equal(report, "<!doctype html>");
  assert.equal(exported.status, "exported");
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
