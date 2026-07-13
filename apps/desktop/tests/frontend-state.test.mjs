import assert from "node:assert/strict";
import test from "node:test";

import {
  WORKFLOWS,
  beginOpenSession,
  initialState,
  invokeActiveSessionReport,
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

test("the frontend invokes only the narrow session commands", async () => {
  const calls = [];
  const invoke = async (...args) => {
    calls.push(args);
    return args[0] === "open_session_bundle"
      ? { status: "opened", session: { sessionId: "session-1" } }
      : "<!doctype html>";
  };

  const result = await invokeOpenSession(invoke);
  const report = await invokeActiveSessionReport(invoke);

  assert.deepEqual(calls, [["open_session_bundle"], ["active_session_report"]]);
  assert.equal(result.status, "opened");
  assert.equal(report, "<!doctype html>");
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
