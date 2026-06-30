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
import { suppressBreakpointsForProfiling, type DapMessage } from "../../dap-proxy";

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
