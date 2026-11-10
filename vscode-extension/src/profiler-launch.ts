// Implements [PROFILE-PROCESSES-LAUNCH-FILE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-LAUNCH-FILE
/**
 * Pure helpers for the "profile on launch" flow: deciding whether a freshly
 * started debug session should be auto-profiled, and waiting for the
 * debuggee's PID to land. Stateless — the session flow stays in profiler.ts.
 */

import * as vscode from "vscode";
import type { Store } from "./store";
import { POLL_INTERVAL_MS, STARTUP_TIMEOUT_MS } from "./timeouts";

/**
 * Whether a freshly started debug session should be auto-profiled: either its
 * launch configuration asked for it (`profileOnLaunch: true`, set by the
 * metric-explicit "Run & Profile CPU (Current File)" entry point — #82) or the
 * user enabled the global `basilisk.profiler.profileOnLaunch` setting.
 */
export function shouldProfileOnLaunch(
  session: Pick<vscode.DebugSession, "type" | "configuration">,
): boolean {
  if (session.type !== "basilisk-debug") { return false; }
  // A "Run & Track Memory" launch tracks memory, not CPU. Never auto-start the
  // CPU sampler on it — even when the global setting is on (read directly
  // below, so the stamp carve-out in applyDebugConfigDefaults is not enough) —
  // or the cooperative sampler and tracemalloc collide on the single entry
  // pause (dap-1).
  if (session.configuration.memoryTrackOnLaunch === true) { return false; }
  if (session.configuration.profileOnLaunch === true) { return true; }
  return vscode.workspace
    .getConfiguration("basilisk")
    .get<boolean>("profiler.profileOnLaunch", false);
}

/**
 * Resolve once the debuggee's PID for `sessionId` is known (captured from the
 * DAP `process` event into the store), or after a bounded wait. Returns whether
 * the PID became available. Used by the "Profile on Launch" auto-attach so it
 * doesn't fire before debugpy reports the process.
 */
export async function waitForDebuggeePid(store: Store, sessionId: string): Promise<boolean> {
  if (store.getDebuggeeProcessId(sessionId) !== undefined) {
    return true;
  }
  return new Promise((resolve) => {
    const interval = setInterval(() => {
      if (store.getDebuggeeProcessId(sessionId) !== undefined) {
        clearInterval(interval);
        clearTimeout(timeout);
        resolve(true);
      }
    }, POLL_INTERVAL_MS);
    // Clear BOTH timers on whichever path resolves first so neither dangles.
    const timeout = setTimeout(() => {
      clearInterval(interval);
      resolve(store.getDebuggeeProcessId(sessionId) !== undefined);
    }, STARTUP_TIMEOUT_MS);
  });
}
