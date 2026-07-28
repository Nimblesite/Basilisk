// Tests for [PROFILE-LAUNCH-NOSTOP]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-LAUNCH-NOSTOP
//
// A "Run & Profile CPU (Current File)" launch sets `profileOnLaunch` and must
// run to completion — it must NOT present as an interactive debug session that
// halts at the user's breakpoints / exception stops (#145). The DAP proxy
// neutralises breakpoints for profiling launches while leaving normal debug
// sessions (and `stopOnEntry`, which is a launch arg, not a breakpoint)
// untouched. `suppressBreakpointsForProfiling` is the transformation the proxy
// applies to every client→debugpy request before forwarding.

import * as assert from "assert";
import { parseDapMessage, suppressBreakpointsForProfiling, type DapMessage } from "../../dap-proxy";
import { isRecord } from "../../unknown-shape";

suite("Run & Profile launches run to completion, not as a debug session (#145)", () => {
  test("a profiling launch arms no user breakpoints", () => {
    const setBreakpoints: DapMessage = {
      type: "request",
      command: "setBreakpoints",
      seq: 7,
      arguments: { source: { path: "/work/app.py" }, breakpoints: [{ line: 10 }, { line: 20 }] },
    };
    const forwarded = suppressBreakpointsForProfiling(setBreakpoints, true);
    assert.deepStrictEqual(
      forwarded.arguments?.breakpoints,
      [],
      "a profiling run must strip user breakpoints so debugpy never halts the run (#145)",
    );
    // The source is preserved so debugpy clears the right file's breakpoints.
    assert.deepStrictEqual(forwarded.arguments?.source, { path: "/work/app.py" });
  });

  test("a profiling launch arms no function breakpoints either", () => {
    const setFunctionBreakpoints: DapMessage = {
      type: "request",
      command: "setFunctionBreakpoints",
      seq: 11,
      arguments: { breakpoints: [{ name: "hot_function" }] },
    };
    const forwarded = suppressBreakpointsForProfiling(setFunctionBreakpoints, true);
    assert.deepStrictEqual(
      forwarded.arguments?.breakpoints,
      [],
      "function breakpoints must be stripped too, or a profiling run still halts (#145)",
    );
  });

  test("a profiling launch disables exception stops", () => {
    const setExceptionBreakpoints: DapMessage = {
      type: "request",
      command: "setExceptionBreakpoints",
      seq: 8,
      arguments: { filters: ["raised", "uncaught"], filterOptions: [{ filterId: "raised" }] },
    };
    const forwarded = suppressBreakpointsForProfiling(setExceptionBreakpoints, true);
    assert.deepStrictEqual(
      forwarded.arguments?.filters,
      [],
      "a profiling run must not stop on raised/uncaught exceptions (#145)",
    );
    assert.deepStrictEqual(
      forwarded.arguments?.filterOptions,
      [],
      "filterOptions must be cleared too, or exception stops sneak back in",
    );
  });

  test("a normal debug launch keeps the user's breakpoints intact", () => {
    const setBreakpoints: DapMessage = {
      type: "request",
      command: "setBreakpoints",
      seq: 7,
      arguments: { source: { path: "/work/app.py" }, breakpoints: [{ line: 10 }] },
    };
    const forwarded = suppressBreakpointsForProfiling(setBreakpoints, false);
    assert.deepStrictEqual(
      forwarded.arguments?.breakpoints,
      [{ line: 10 }],
      "ordinary debugging must keep user breakpoints — only profiling launches strip them",
    );
  });

  test("a profiling launch leaves non-breakpoint requests (e.g. continue) untouched", () => {
    const cont: DapMessage = { type: "request", command: "continue", seq: 9, arguments: { threadId: 1 } };
    const forwarded = suppressBreakpointsForProfiling(cont, true);
    assert.strictEqual(
      forwarded,
      cont,
      "non-breakpoint requests must pass through unchanged (incl. the resume after stopOnEntry)",
    );
  });
});

// Tests for [VSIX-DEBUGGING]. See docs/specs/VSIX-SPEC.md#VSIX-DEBUGGING
//
// The proxy sits between the editor and debugpy and re-serialises every frame
// it forwards (`sendToClient`/`sendToDebugpy` both `JSON.stringify(msg)`).
// Whatever the decoder drops therefore never reaches the other end. The
// protocol has fields beyond the handful the proxy itself switches on — the
// standard `message` on a failed response, plus adapter-specific extensions —
// and dropping those silently degrades the debug session.
suite("The DAP proxy forwards frames without dropping fields", () => {
  /** What the proxy would put back on the wire for a decoded frame. */
  function reserialize(message: DapMessage | undefined): Record<string, unknown> {
    assert.notStrictEqual(message, undefined, "the frame must decode");
    const wire: unknown = JSON.parse(JSON.stringify(message));
    assert.ok(isRecord(wire), "a re-serialised DAP frame is a JSON object");
    return wire;
  }

  test("a failed response keeps the adapter's error text", () => {
    const wire = JSON.stringify({
      type: "response",
      request_seq: 4,
      success: false,
      command: "evaluate",
      message: "Unable to evaluate expression: name 'x' is not defined",
    });
    assert.strictEqual(
      reserialize(parseDapMessage(wire)).message,
      "Unable to evaluate expression: name 'x' is not defined",
      "`message` is the DAP field that carries an error to the user — dropping it blanks the failure",
    );
  });

  test("adapter-specific fields survive the round trip", () => {
    const wire = JSON.stringify({
      type: "event",
      event: "debugpySockets",
      seq: 12,
      body: { sockets: [] },
      pydevdAuthToken: "opaque-token",
    });
    assert.strictEqual(
      reserialize(parseDapMessage(wire)).pydevdAuthToken,
      "opaque-token",
      "the proxy is a relay: fields it does not understand must still reach the other side",
    );
  });

  test("the fields the proxy switches on are still decoded", () => {
    const parsed = parseDapMessage(
      JSON.stringify({ type: "request", command: "next", seq: 3, arguments: { threadId: 1 } }),
    );
    assert.strictEqual(parsed?.type, "request");
    assert.strictEqual(parsed?.command, "next");
    assert.strictEqual(parsed?.seq, 3);
    assert.strictEqual(parsed?.arguments?.threadId, 1);
  });

  test("bytes that are not a DAP frame are rejected", () => {
    assert.strictEqual(parseDapMessage("{not json"), undefined, "unparseable bytes are dropped");
    assert.strictEqual(
      parseDapMessage(JSON.stringify({ seq: 1 })),
      undefined,
      "a frame with no `type` matches no branch of the proxy, so it is not a DAP frame",
    );
  });
});
