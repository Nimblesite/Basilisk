// Tests for [PROFILE-MEMORY-COURIER] stop-state bookkeeping. See
// docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-COURIER and dap-output.ts.
//
// The DAP spec makes the `continued` event OPTIONAL after a resume request:
// "a debug adapter is not expected to send this event in response to a
// request that implies that execution continues, e.g. launch or continue" —
// the client must treat a successful `continue`/step RESPONSE as "running".
// The tracker used to clear its stopped bookkeeping only on the `continued`
// event, so in the window between the continue response and that (late,
// optional) event, `currentStoppedFrameId` saw a stale "stopped" thread,
// asked debugpy for its stack, got a sampled non-evaluable frame, and the
// memory-snapshot courier evaluated against a bogus frame and failed — the
// large-heap CI failure. These tests drive the production tracker through
// its public message hooks, exactly as VS Code delivers DAP traffic.

import * as assert from "assert";
import type * as vscode from "vscode";
import { BasiliskDebugAdapterTrackerFactory } from "../../debug-adapter";
import { clearDebugOutput, stoppedThreadIds } from "../../dap-output";

/** Build a tracker for a throwaway session id via the production factory. */
function trackerFor(sessionId: string): {
  tracker: vscode.DebugAdapterTracker;
  stopped: () => readonly number[];
} {
  // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- generic DebugSession double; the tracker reads only id and name
  const session = { id: sessionId, name: "stop-state test" } as vscode.DebugSession;
  const created = new BasiliskDebugAdapterTrackerFactory().createDebugAdapterTracker(session);
  // ProviderResult is `T | undefined | null | Thenable<T>`; narrow it by
  // checking rather than asserting, so a factory that starts returning a
  // promise fails here instead of silently handing the tests a thenable.
  assert.ok(created !== undefined && created !== null, "the factory must create a tracker");
  assert.ok(!("then" in created), "the factory must create the tracker synchronously");
  return { tracker: created, stopped: () => stoppedThreadIds(sessionId) };
}

function stoppedEvent(threadId: number): unknown {
  return {
    type: "event",
    event: "stopped",
    body: { reason: "breakpoint", threadId, allThreadsStopped: true },
  };
}

suite("DAP stop-state — a successful resume response means running", () => {
  test("a successful continue RESPONSE clears the stopped bookkeeping without any continued event", () => {
    const id = "stop-state-continue-response";
    const { tracker, stopped } = trackerFor(id);
    try {
      tracker.onDidSendMessage?.(stoppedEvent(1));
      assert.ok(stopped().length > 0, "the stopped event must be recorded first");

      tracker.onWillReceiveMessage?.({
        type: "request",
        command: "continue",
        seq: 7,
        arguments: { threadId: 1 },
      });
      tracker.onDidSendMessage?.({
        type: "response",
        command: "continue",
        request_seq: 7,
        success: true,
        body: {},
      });

      assert.deepStrictEqual(
        stopped(),
        [],
        "after a successful continue response the thread must read as RUNNING — " +
          "the continued event is optional per the DAP spec and can land late"
      );
    } finally {
      clearDebugOutput(id);
    }
  });

  test("a FAILED continue response leaves the stopped bookkeeping intact", () => {
    const id = "stop-state-continue-failed";
    const { tracker, stopped } = trackerFor(id);
    try {
      tracker.onDidSendMessage?.(stoppedEvent(2));
      tracker.onWillReceiveMessage?.({
        type: "request",
        command: "continue",
        seq: 3,
        arguments: { threadId: 2 },
      });
      tracker.onDidSendMessage?.({
        type: "response",
        command: "continue",
        request_seq: 3,
        success: false,
        message: "cannot continue",
      });

      assert.ok(
        stopped().length > 0,
        "a rejected continue did not resume anything — the pause must survive"
      );
    } finally {
      clearDebugOutput(id);
    }
  });

  test("a successful step (next) response also clears the stepped thread", () => {
    const id = "stop-state-step-response";
    const { tracker, stopped } = trackerFor(id);
    try {
      tracker.onDidSendMessage?.(stoppedEvent(5));
      tracker.onWillReceiveMessage?.({
        type: "request",
        command: "next",
        seq: 11,
        arguments: { threadId: 5 },
      });
      tracker.onDidSendMessage?.({
        type: "response",
        command: "next",
        request_seq: 11,
        success: true,
      });

      assert.deepStrictEqual(
        stopped(),
        [],
        "a stepping thread is running until its own stopped event lands"
      );
    } finally {
      clearDebugOutput(id);
    }
  });

  test("an unrelated successful response (stackTrace) never clears the pause", () => {
    const id = "stop-state-unrelated-response";
    const { tracker, stopped } = trackerFor(id);
    try {
      tracker.onDidSendMessage?.(stoppedEvent(9));
      tracker.onWillReceiveMessage?.({
        type: "request",
        command: "stackTrace",
        seq: 21,
        arguments: { threadId: 9 },
      });
      tracker.onDidSendMessage?.({
        type: "response",
        command: "stackTrace",
        request_seq: 21,
        success: true,
        body: { stackFrames: [] },
      });

      assert.ok(stopped().length > 0, "probing a stopped thread must not mark it running");
    } finally {
      clearDebugOutput(id);
    }
  });
});
