// Implements [PROFILE-UX-PROGRESS] + [PROFILE-NOTIFICATIONS-PROGRESS].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-UX-PROGRESS
/**
 * Profiler status-bar lifecycle: hidden → starting spinner → live flame
 * counter. A CPU-profiling start is never silent — between the click and the
 * first sample batch the item shows `$(loading~spin)`, then the LSP's
 * progress notifications drive the live counter. Owned here so profiler.ts
 * stays focused on session flow.
 */

import * as vscode from "vscode";

/** Status bar priority — slightly lower than the main Basilisk item. */
const PROFILER_STATUS_BAR_PRIORITY = 99;
/** Seconds per minute — threshold for switching from "Ns" to "N.Mm" display. */
const SECONDS_PER_MINUTE = 60;
/** Threshold for switching from raw count to "N.NK" display. */
const KILO_THRESHOLD = 1000;

/** The status-bar lifecycle states. */
export type ProfilerStatusState = "idle" | "starting" | "profiling";

let statusBarItem: vscode.StatusBarItem | undefined;
let statusState: ProfilerStatusState = "idle";
let displayedPid: number | undefined;

/** Create the profiler status-bar item (click = stop). Returns its disposable. */
export function createProfilerStatusBar(): vscode.Disposable {
  statusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    PROFILER_STATUS_BAR_PRIORITY,
  );
  statusBarItem.command = "basilisk.profileStop";
  setProfilerStatus("idle");
  return statusBarItem;
}

/** Drive the lifecycle. `pid` is shown in tooltips once known. */
export function setProfilerStatus(state: ProfilerStatusState, pid?: number): void {
  statusState = state;
  displayedPid = pid;
  if (statusBarItem === undefined) { return; }

  if (state === "idle") {
    statusBarItem.hide();
    return;
  }

  if (state === "starting") {
    // [PROFILE-UX-PROGRESS] Click → spinner, never click → silence.
    statusBarItem.text = "$(loading~spin) Profiler starting…";
    statusBarItem.tooltip = "Basilisk is setting up CPU profiling for the program";
    statusBarItem.backgroundColor = undefined;
    statusBarItem.show();
    return;
  }

  statusBarItem.text = "$(flame) Profiling...";
  statusBarItem.tooltip = `Profiling PID ${pid ?? "?"} — click to stop`;
  statusBarItem.backgroundColor = new vscode.ThemeColor("statusBarItem.warningBackground");
  statusBarItem.show();
}

/** Live sample-count update ([PROFILE-NOTIFICATIONS-PROGRESS]). */
export function updateProfilerProgress(
  sampleCount: number,
  duration: number,
  topFunction: string,
): void {
  if (statusBarItem === undefined) { return; }
  const durationStr = duration < SECONDS_PER_MINUTE
    ? `${duration.toFixed(0)}s`
    : `${(duration / SECONDS_PER_MINUTE).toFixed(1)}m`;
  const samplesStr = sampleCount >= KILO_THRESHOLD
    ? `${(sampleCount / KILO_THRESHOLD).toFixed(1)}K`
    : String(sampleCount);
  statusBarItem.text = `$(flame) ${samplesStr} samples (${durationStr})`;
  statusBarItem.tooltip =
    `PID ${displayedPid ?? "?"} — ${samplesStr} samples, ${durationStr}\n` +
    `Top: ${topFunction}\nClick to stop`;
}

/**
 * The profiler status-bar text, or undefined when idle/hidden. E2e seam for
 * [PROFILE-NOTIFICATIONS-PROGRESS]/[PROFILE-UX-PROGRESS]: the lifecycle
 * (starting spinner → live samples) lands here and StatusBarItem state is
 * not readable via the public API.
 */
export function profilerStatusText(): string | undefined {
  if (statusState === "idle") { return undefined; }
  return statusBarItem?.text;
}
