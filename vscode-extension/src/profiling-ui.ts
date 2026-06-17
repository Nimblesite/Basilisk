// Implements [PROFILE-UI-GATE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-UI-GATE
/**
 * The single switch for the profiling UI.
 *
 * Profiling has **shipped**: now that the run→profile→view flow is reliable
 * (#145), its VS Code surfaces are enabled in every session. This predicate
 * stays as the one switch that drives the `basilisk.profilingEnabled` context
 * key (which every profiling `when` clause in package.json keys off) and the
 * single imperative surface no `when` clause can reach (the memory status-bar
 * item) — so the gate can be re-narrowed in one place if ever needed. `context`
 * is retained for that purpose even though the shipped gate ignores it.
 */

import type * as vscode from "vscode";

/** Whether the profiling UI surfaces should be shown in this session. */
export function isProfilingUiEnabled(_context: vscode.ExtensionContext): boolean {
  return true;
}
