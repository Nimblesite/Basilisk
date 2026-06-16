// Implements [PROFILE-UX-PROGRESS] + [PROFILE-NOTIFICATIONS-PROGRESS].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-UX-PROGRESS
/**
 * Profiler status-bar lifecycle: hidden → starting spinner → live flame
 * counter. A CPU-profiling start is never silent — between the click and the
 * first sample batch the item shows `$(loading~spin)`, then the LSP's progress
 * notifications drive the live counter.
 *
 * The item renders purely from the store's reactive `profiler` signal
 * ([PROFILE-PROCESSES-REACTIVE]): one `effect` repaints it whenever the session
 * state changes, so there is no imperative "set status" call to forget. Owned
 * here so profiler.ts stays focused on session flow.
 */

import * as vscode from "vscode";
import { effect } from "@preact/signals-core";
import type { Store } from "./store";
import type { ProfilerActivity, ProfilerSession } from "./profiler-state";
import { formatProfileDuration, formatSampleCount } from "./profiler-format";

/** Status bar priority — slightly lower than the main Basilisk item. */
const PROFILER_STATUS_BAR_PRIORITY = 99;

let statusBarItem: vscode.StatusBarItem | undefined;
/** Last-rendered CPU activity, so the e2e seam knows when the item is hidden. */
let renderedActivity: ProfilerActivity = "idle";

/**
 * Create the profiler status-bar item (click = stop) and bind it to the store's
 * `profiler` signal. Returns a disposable that tears down both the effect and
 * the item.
 */
export function bindProfilerStatusBar(store: Store): vscode.Disposable {
  const item = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    PROFILER_STATUS_BAR_PRIORITY,
  );
  item.command = "basilisk.profileStop";
  statusBarItem = item;
  renderStatusBar(item, store.profiler.value);
  const disposeEffect = effect(() => { renderStatusBar(item, store.profiler.value); });
  return {
    dispose() {
      disposeEffect();
      item.dispose();
      statusBarItem = undefined;
      renderedActivity = "idle";
    },
  };
}

/** Repaint the item to match the current CPU session state. */
function renderStatusBar(item: vscode.StatusBarItem, session: ProfilerSession): void {
  renderedActivity = session.cpu;

  if (session.cpu === "idle") {
    item.hide();
    return;
  }

  if (session.cpu === "starting") {
    // [PROFILE-UX-PROGRESS] Click → spinner, never click → silence.
    item.text = "$(loading~spin) Profiler starting…";
    item.tooltip = "Basilisk is setting up CPU profiling for the program";
    item.backgroundColor = undefined;
    item.show();
    return;
  }

  // Active: the live flame counter, warning-tinted so it reads as "running".
  item.backgroundColor = new vscode.ThemeColor("statusBarItem.warningBackground");
  if (session.sampleCount > 0) {
    const samples = formatSampleCount(session.sampleCount);
    const duration = formatProfileDuration(session.durationSecs);
    item.text = `$(flame) ${samples} samples (${duration})`;
    item.tooltip =
      `PID ${session.cpuPid ?? "?"} — ${samples} samples, ${duration}\n` +
      `Top: ${session.topFunction ?? "—"}\nClick to stop`;
  } else {
    item.text = "$(flame) Profiling...";
    item.tooltip = `Profiling PID ${session.cpuPid ?? "?"} — click to stop`;
  }
  item.show();
}

/**
 * The profiler status-bar text, or undefined when idle/hidden. E2e seam for
 * [PROFILE-NOTIFICATIONS-PROGRESS]/[PROFILE-UX-PROGRESS]: the lifecycle
 * (starting spinner → live samples) lands here and StatusBarItem state is not
 * readable via the public API.
 */
export function profilerStatusText(): string | undefined {
  if (renderedActivity === "idle") { return undefined; }
  return statusBarItem?.text;
}
