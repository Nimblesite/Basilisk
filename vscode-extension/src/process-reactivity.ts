// Implements [PROFILE-PROCESSES-REACTIVE]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-REACTIVE
/**
 * Makes the Python Processes panel react to the store's profiling state.
 *
 * One `effect` over `store.profiler` drives three things whenever a session
 * starts, streams samples, or stops:
 *   1. the view's native chrome — a live message ("🔥 Profiling PID 1234 ·
 *      12.3K samples (4s)") and a badge dot;
 *   2. the gating context keys (`basilisk.profilerBusy` / `.profiling` /
 *      `.memoryTracking` / `.profilerStarting`) that hide the "Run & Profile"
 *      launches and reveal the Stop affordances in package.json;
 *   3. the provider's "which row is being profiled" marker, so that row swaps
 *      its inline Profile button for a Stop button.
 *
 * The message/badge update on every change (cheap), but the context keys and
 * the tree rebuild only fire when the *gating* state changes — a per-second
 * sample-count tick must not rebuild the whole tree.
 */

import * as vscode from "vscode";
import { effect } from "@preact/signals-core";
import type { Store } from "./store";
import type { ProfilerSession } from "./profiler-state";
import type { PythonProcessesProvider } from "./process-explorer";
import { formatProfileDuration, formatSampleCount } from "./profiler-format";

/** The live tree view, exposed read-only for the e2e seam below. */
let boundView: vscode.TreeView<unknown> | undefined;

/**
 * Bind the panel to the store's profiling state. Returns a disposable that
 * stops the effect and drops the seam reference.
 */
export function bindProcessPanelReactivity(
  store: Store,
  treeView: vscode.TreeView<unknown>,
  provider: PythonProcessesProvider,
): vscode.Disposable {
  boundView = treeView;
  let lastGate = "";
  const disposeEffect = effect(() => {
    const session = store.profiler.value;
    // Cheap, every change: the live readout above the tree.
    treeView.message = panelMessage(session);
    treeView.badge = panelBadge(session);

    // Expensive (context keys + tree rebuild): only on gating transitions.
    const gate = `${session.cpu}|${session.cpuPid ?? ""}|${session.memory}`;
    if (gate !== lastGate) {
      lastGate = gate;
      applyContextKeys(session);
      provider.setActiveProfilingPid(session.cpu === "active" ? session.cpuPid : undefined);
      provider.refresh();
    }
  });
  return {
    dispose() {
      disposeEffect();
      boundView = undefined;
    },
  };
}

/** Push the gating context keys every profiling `when` clause keys off. */
function applyContextKeys(session: ProfilerSession): void {
  const busy = session.cpu !== "idle" || session.memory !== "idle";
  void vscode.commands.executeCommand("setContext", "basilisk.profilerBusy", busy);
  void vscode.commands.executeCommand("setContext", "basilisk.profiling", session.cpu === "active");
  void vscode.commands.executeCommand("setContext", "basilisk.memoryTracking", session.memory === "active");
  void vscode.commands.executeCommand(
    "setContext",
    "basilisk.profilerStarting",
    session.cpu === "starting" || session.memory === "starting",
  );
}

/**
 * The live message shown above the tree, or undefined when idle. Pure so it can
 * be asserted directly; the live view value is read via [`pythonProcessesViewState`].
 */
export function panelMessage(session: ProfilerSession): string | undefined {
  if (session.cpu === "starting") { return "⏳ Starting CPU profiler…"; }
  if (session.cpu === "active") {
    const head = `🔥 Profiling PID ${session.cpuPid ?? "?"}`;
    if (session.sampleCount > 0) {
      const live = `${formatSampleCount(session.sampleCount)} samples (${formatProfileDuration(session.durationSecs)})`;
      const top = session.topFunction !== undefined && session.topFunction !== ""
        ? ` · ${session.topFunction}`
        : "";
      return `${head} · ${live}${top}`;
    }
    return `${head}…`;
  }
  if (session.memory === "starting") { return "⏳ Starting memory tracking…"; }
  if (session.memory === "active") { return "🗄️ Tracking memory allocations…"; }
  return undefined;
}

/** A badge dot while busy (its tooltip mirrors the message), else none. */
export function panelBadge(session: ProfilerSession): vscode.ViewBadge | undefined {
  const message = panelMessage(session);
  if (message === undefined) { return undefined; }
  return { value: 1, tooltip: message };
}

/**
 * The panel's live chrome, or `undefined`s when idle. E2e seam for
 * [PROFILE-PROCESSES-REACTIVE]: TreeView message/badge are not otherwise
 * readable once the view is owned by VS Code.
 */
export function pythonProcessesViewState(): { message: string | undefined; badge: number | undefined } {
  return { message: boundView?.message, badge: boundView?.badge?.value };
}
