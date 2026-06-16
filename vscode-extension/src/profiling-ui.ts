// Implements [PROFILE-UI-GATE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-UI-GATE
/**
 * The single switch for the profiling UI.
 *
 * Profiling is functionally complete in the LSP but its VS Code surfaces are
 * hidden from shipped users until the experience is reliable — a half-working
 * entry point is worse first-run UX than none. The whole extension still wires
 * profiling up, so the test suite keeps exercising it: the surfaces are enabled
 * exactly when VS Code runs us under test (`ExtensionMode.Test`) and hidden in
 * every shipped (`Production`) and dev-host (`Development`) session.
 *
 * One predicate drives everything: the `basilisk.profilingEnabled` context key
 * (which every profiling `when` clause in package.json keys off) and the single
 * imperative surface no `when` clause can reach (the memory status-bar item).
 * To ship profiling, return `true` here unconditionally and delete this gate.
 */

import type * as vscode from "vscode";

/** Whether the profiling UI surfaces should be shown in this session. */
export function isProfilingUiEnabled(_context: vscode.ExtensionContext): boolean {
  // Enabled unconditionally: the CPU and memory pipelines are covered by
  // real end-to-end suites on macOS and Linux (attach, live progress, heat
  // map, artifacts, courier round-trip) — the reliability bar this gate was
  // waiting for. The gate function stays so a kill-switch is one line away.
  return true;
}
