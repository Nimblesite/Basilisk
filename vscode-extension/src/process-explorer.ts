// Implements [PROFILE-PROCESSES-PANEL]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-PANEL
/**
 * Python Processes — TreeDataProvider for the Basilisk sidebar.
 *
 * The headline fix for #62: instead of a raw PID input box, the user picks a
 * running Python process from this panel and starts CPU/memory profiling with
 * one click. The process list is owned by the LSP (`basilisk.profiler.processes`,
 * implemented over `sysinfo`) and fetched into the store's `processes` Signal by
 * the store-side poll (process-poll.ts). This provider is a pure projection of
 * that centralised state (#148): it holds no data or timer of its own — it
 * sorts, groups, filters, and renders whatever the store says, re-rendering on
 * each `processesRevision` bump exactly like the Modules panel
 * ([EXTACT-REACTIVE-STATE]).
 */

import * as vscode from "vscode";
import { type Store } from "./store";
import { registerLaunchCommands } from "./process-launch";
import { bindProcessesContextKey, bindProcessPolling, fetchProcessesIntoStore } from "./process-poll";
import { bindDebuggeeTracking, bindProcessPanelReactivity } from "./process-reactivity";
import { subscribeRevision } from "./reactive-refresh";
import { withViewProgress } from "./progress-ops";
import type { ProcessPanelState } from "./processes-state";
import {
  GROUP_LABEL,
  LaunchActionItem,
  MessageTreeItem,
  PROCESS_URI_SCHEME,
  ProcessGroupItem,
  ProcessTreeItem,
  SORT_COMPARATORS,
  SORT_LABEL,
  type GroupMode,
  type ProcessInfo,
  type TreeItem,
} from "./process-explorer-rows";

// The model and row presentation live in process-explorer-rows.ts (500-LOC
// split), the reactive state in processes-state.ts (#148); re-exported here so
// consumers keep one import site for the panel.
export { type ProcessInfo } from "./process-explorer-rows";
export { type ProcessesFetchState as ProcessesState } from "./processes-state";

/** How long the "sorted/grouped by …" status hint stays visible (ms). */
const STATUS_HINT_MS = 2000;

// ── Provider ─────────────────────────────────────────────────────────────

export class PythonProcessesProvider implements vscode.TreeDataProvider<TreeItem>, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<TreeItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  public readonly disposables: vscode.Disposable[] = [];

  constructor(private readonly store: Store) {}

  /** Repaint from the store's current state (no fetch — pure projection, #148). */
  public refresh(): void {
    this.emitter.fire(undefined);
  }

  /**
   * The current process-fetch state — the e2e seam for [PROFILE-PROCESSES-PANEL]
   * empty-state honesty (#147), mirrored to the `basilisk.processesState` key.
   */
  public get processesState(): ProcessPanelState["fetch"] {
    return this.store.processes.value.fetch;
  }

  /**
   * Fetch fresh process data into the store — awaitable, so the manual Refresh
   * command can run it under the view's progress bar ([PROFILE-UX-PROGRESS]).
   * The resulting revision bump repaints every subscribed view; the silent
   * store-side poll needs no progress bar (a flash every tick would be noise).
   */
  public async refreshNow(): Promise<void> {
    await fetchProcessesIntoStore(this.store);
  }

  public cycleSortMode(): void {
    const mode = this.store.cycleProcessSort();
    void vscode.window.setStatusBarMessage(`Python Processes sorted by ${SORT_LABEL[mode]}`, STATUS_HINT_MS);
  }

  public cycleGroupMode(): void {
    const mode = this.store.cycleProcessGroup();
    void vscode.window.setStatusBarMessage(`Python Processes grouped by ${GROUP_LABEL[mode]}`, STATUS_HINT_MS);
  }

  public setFilter(text: string): void {
    this.store.setProcessFilter(text);
  }

  public dispose(): void {
    for (const d of this.disposables) { d.dispose(); }
    this.emitter.dispose();
  }

  public getTreeItem(element: TreeItem): vscode.TreeItem {
    return element;
  }

  public getChildren(element?: TreeItem): TreeItem[] {
    const state = this.store.processes.value;
    if (element instanceof ProcessGroupItem) {
      // Re-sort so non-debuggable rows sink to the bottom *within the group* too,
      // not just in the flat list ([PROFILE-PROCESSES-DISPLAY]).
      return sortProcesses([...element.members], state).map((proc) => this.processRow(proc));
    }
    if (
      element instanceof ProcessTreeItem ||
      element instanceof MessageTreeItem ||
      element instanceof LaunchActionItem
    ) {
      return [];
    }

    // With no processes at all, return [] so the empty/loading/error
    // `viewsWelcome` (which carries the big launch buttons) renders honestly.
    if (state.list.length === 0) {
      return [];
    }

    // Processes exist → the welcome can't show, so pin the current-file launches
    // to the top as rows ([PROFILE-PROCESSES-LAUNCH-FILE]).
    const actions = this.launchActionRows();
    const visible = sortProcesses(applyFilter(state), state);
    if (visible.length === 0) {
      // The user's search filter hid every running process — keep the launches
      // and an honest placeholder rather than an empty list (procexp-2).
      return [...actions, new MessageTreeItem(filteredEmptyLabel(state))];
    }
    const rows = state.groupMode === "none"
      ? visible.map((proc) => this.processRow(proc))
      : buildGroups(visible, state.groupMode);
    return [...actions, ...rows];
  }

  /**
   * Render one process row. The "this row is being profiled" marker derives
   * straight from the store's profiler signal — never a provider field (#148) —
   * and the active-debuggee marker from the centralised panel state.
   */
  private processRow(proc: ProcessInfo): ProcessTreeItem {
    const session = this.store.profiler.value;
    const profilingPid = session.cpu === "active" ? session.cpuPid : undefined;
    return new ProcessTreeItem(proc, profilingPid, this.store.processes.value.activeDebuggeePid);
  }

  /**
   * The pinned current-file launch rows, gated per-activity: the CPU launch is
   * hidden while CPU profiling is busy, the memory launch while memory tracking
   * is busy — so a second same-metric run is never offered, yet either can start
   * while the other runs ([PROFILE-PROCESSES-REACTIVE]).
   */
  private launchActionRows(): LaunchActionItem[] {
    const rows: LaunchActionItem[] = [];
    if (!this.store.cpuBusy.value) {
      rows.push(new LaunchActionItem("Run & Profile CPU (Current File)", "basilisk.profileCurrentFileCpu", "flame"));
    }
    if (!this.store.memoryBusy.value) {
      rows.push(new LaunchActionItem("Run & Track Memory (Current File)", "basilisk.trackMemoryCurrentFile", "database"));
    }
    return rows;
  }

}

// ── Pure projection helpers (#148) ─────────────────────────────────────────

/**
 * Sort by the active mode, but always sink non-`debuggable` rows to the bottom
 * so the processes the user can act on stay on top ([PROFILE-PROCESSES-DISPLAY]).
 */
function sortProcesses(processes: ProcessInfo[], state: ProcessPanelState): ProcessInfo[] {
  const byMode = SORT_COMPARATORS[state.sortMode];
  return processes.sort((a, b) => {
    if (a.debuggable !== b.debuggable) { return a.debuggable ? -1 : 1; }
    return byMode(a, b);
  });
}

/** Why the filtered view is empty though processes are running (procexp-2). */
function filteredEmptyLabel(state: ProcessPanelState): string {
  return `No process matches "${state.filterText}" (${state.list.length} running)`;
}

/**
 * Apply only the user's explicit search filter. Enumeration is zero-filter
 * ([PROFILE-PROCESSES-SCOPE]); the panel never auto-hides a process.
 */
function applyFilter(state: ProcessPanelState): ProcessInfo[] {
  if (state.filterText === "") { return [...state.list]; }
  return state.list.filter((proc) => {
    const haystack = `${proc.name} ${proc.script ?? ""} ${proc.pid}`.toLowerCase();
    return haystack.includes(state.filterText);
  });
}

function buildGroups(processes: readonly ProcessInfo[], groupMode: GroupMode): ProcessGroupItem[] {
  const groups = new Map<string, ProcessInfo[]>();
  for (const proc of processes) {
    const key = groupKey(proc, groupMode);
    const bucket = groups.get(key);
    if (bucket === undefined) {
      groups.set(key, [proc]);
    } else {
      bucket.push(proc);
    }
  }
  return [...groups.entries()]
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([key, members]) => new ProcessGroupItem(key, members));
}

function groupKey(proc: ProcessInfo, groupMode: GroupMode): string {
  switch (groupMode) {
    case "version": return proc.pythonVersion ?? "Unknown version";
    case "interpreter": return proc.interpreterPath ?? proc.name;
    case "user": return proc.user ?? "Unknown user";
    case "parent": return `Parent ${proc.ppid}`;
    case "none": return "";
  }
}

// ── Row decorations (green / grey) ─────────────────────────────────────────

/**
 * Colours whole process-row labels green (workspace) or grey (non-debuggable),
 * keyed on the synthetic `basilisk-process:` URI each row carries — the only way
 * to tint a tree item's full label. Implements [PROFILE-PROCESSES-DISPLAY].
 *
 * Greying wins over green: a process you cannot debug is never shown as an
 * actionable workspace row.
 */
export class ProcessDecorationProvider implements vscode.FileDecorationProvider, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<undefined>();
  public readonly onDidChangeFileDecorations = this.emitter.event;
  private readonly subscription: vscode.Disposable;
  private readonly scheme = PROCESS_URI_SCHEME;

  constructor(provider: PythonProcessesProvider) {
    // Rows are recreated on every refresh with state baked into their URI query;
    // re-query decorations whenever the tree repaints so colours never go stale.
    this.subscription = provider.onDidChangeTreeData(() => this.emitter.fire(undefined));
  }

  public provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    if (uri.scheme !== this.scheme) { return undefined; }
    const params = new URLSearchParams(uri.query);
    if (params.get("dbg") === "0") {
      return { color: new vscode.ThemeColor("disabledForeground"), tooltip: "Can't profile" };
    }
    if (params.get("ws") === "1") {
      return { color: new vscode.ThemeColor("charts.green"), tooltip: "Workspace process" };
    }
    return undefined;
  }

  public dispose(): void {
    this.subscription.dispose();
    this.emitter.dispose();
  }
}

// ── Registration ─────────────────────────────────────────────────────────

/**
 * Register the Python Processes panel. Returns command disposables for
 * `singletonDisposables`; the tree view and provider go to subscriptions.
 */
export function registerPythonProcesses(
  context: vscode.ExtensionContext,
  store: Store,
): { provider: PythonProcessesProvider; disposables: vscode.Disposable[] } {
  const provider = new PythonProcessesProvider(store);

  const treeView = vscode.window.createTreeView("basilisk.pythonProcesses", {
    treeDataProvider: provider,
  });
  context.subscriptions.push(treeView, provider);

  // The panel is a pure projection of centralised state (#148): the store-side
  // poll feeds `store.processes` while the view is visible, the revision
  // subscription repaints on every store change, and the context-key mirror
  // keeps the welcome's loading/error/empty copy honest (#147).
  provider.disposables.push(bindProcessPolling(store, treeView), bindProcessesContextKey(store));
  subscribeRevision(store.processesRevision, provider);
  // React to the store's profiling state: live chrome, button-gating context
  // keys, and the active-row repaint ([PROFILE-PROCESSES-REACTIVE]).
  provider.disposables.push(bindProcessPanelReactivity(store, treeView, provider));
  // Reveal the inline Track Memory action only on the active debuggee row
  // ([PROFILE-PROCESSES-PANEL]) — memory tracking can't target external processes.
  provider.disposables.push(bindDebuggeeTracking(store));
  // Colour workspace rows green and non-debuggable rows grey ([PROFILE-PROCESSES-DISPLAY]).
  const decorations = new ProcessDecorationProvider(provider);
  provider.disposables.push(decorations, vscode.window.registerFileDecorationProvider(decorations));

  const disposables = [
    ...registerLaunchCommands(store, treeView),
    // Manual refresh runs under the view's progress bar so the click visibly
    // does something; returns the promise so callers/tests can await it.
    vscode.commands.registerCommand("basilisk.refreshProcesses", async () =>
      withViewProgress("basilisk.pythonProcesses", "Refresh Python processes", async () =>
        provider.refreshNow(),
      ),
    ),
    vscode.commands.registerCommand("basilisk.sortProcesses", () => { provider.cycleSortMode(); }),
    vscode.commands.registerCommand("basilisk.groupProcesses", () => { provider.cycleGroupMode(); }),
    vscode.commands.registerCommand("basilisk.filterProcesses", async () => {
      const input = await vscode.window.showInputBox({
        prompt: "Filter Python processes by name, script, or PID",
        placeHolder: "e.g. uvicorn",
      });
      provider.setFilter(input ?? "");
    }),
    vscode.commands.registerCommand("basilisk.copyProcessPid", (item?: ProcessTreeItem) => {
      if (item !== undefined) {
        void vscode.env.clipboard.writeText(String(item.process.pid));
        vscode.window.showInformationMessage(`Copied PID ${item.process.pid}`);
      }
    }),
    vscode.commands.registerCommand("basilisk.revealProcessScript", (item?: ProcessTreeItem) => {
      const scriptPath = item?.process.script;
      if (typeof scriptPath === "string") {
        void vscode.window.showTextDocument(vscode.Uri.file(scriptPath));
      } else {
        vscode.window.showInformationMessage("Basilisk: This process has no resolvable script file.");
      }
    }),
  ];

  return { provider, disposables };
}
