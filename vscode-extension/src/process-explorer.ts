// Implements [PROFILE-PROCESSES-PANEL]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-PANEL
/**
 * Python Processes — TreeDataProvider for the Basilisk sidebar.
 *
 * The headline fix for #62: instead of a raw PID input box, the user picks a
 * running Python process from this panel and starts CPU/memory profiling with
 * one click. The process list is owned by the LSP (`basilisk.profiler.processes`,
 * implemented over `sysinfo`); this module is pure UI — it fetches, sorts,
 * groups, filters, and renders, and wires inline actions back to the existing
 * `basilisk.profiler.start` flow with the selected PID.
 */

import * as vscode from "vscode";
import { type Store } from "./store";
import { Logger } from "./logger";
import { registerLaunchCommands } from "./process-launch";
import { bindDebuggeeTracking, bindProcessPanelReactivity } from "./process-reactivity";
import { withViewProgress } from "./progress-ops";
import {
  GROUP_CYCLE,
  GROUP_LABEL,
  LaunchActionItem,
  MessageTreeItem,
  PROCESS_URI_SCHEME,
  ProcessGroupItem,
  ProcessTreeItem,
  SORT_COMPARATORS,
  SORT_CYCLE,
  SORT_LABEL,
  type GroupMode,
  type ProcessInfo,
  type SortMode,
  type TreeItem,
} from "./process-explorer-rows";

// The model and row presentation live in process-explorer-rows.ts (500-LOC
// split); re-exported here so consumers keep one import site for the panel.
export { type ProcessInfo } from "./process-explorer-rows";

// ── LSP response types ───────────────────────────────────────────────────

interface ProcessesResponse {
  readonly processes: readonly ProcessInfo[];
}

/** LSP command name (must match basilisk-common constants). */
const LSP_CMD = {
  processes: "basilisk.profiler.processes",
} as const;

/**
 * The process-fetch lifecycle, published as the `basilisk.processesState` context
 * key so the empty-state welcome never lies: "No Python processes running" shows
 * only after a fetch actually succeeded (`loaded`), while a still-loading or
 * errored fetch says so honestly ([PROFILE-PROCESSES-PANEL], #147).
 */
export type ProcessesState = "loading" | "loaded" | "error";

/** Context key gating the Python Processes welcome states. */
const PROCESSES_STATE_CONTEXT_KEY = "basilisk.processesState";

/** How long the "sorted/grouped by …" status hint stays visible (ms). */
const STATUS_HINT_MS = 2000;

// ── Provider ─────────────────────────────────────────────────────────────

export class PythonProcessesProvider implements vscode.TreeDataProvider<TreeItem>, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<TreeItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  public readonly disposables: vscode.Disposable[] = [];
  private processes: readonly ProcessInfo[] = [];
  private fetched = false;
  /** Fetch lifecycle, mirrored to the `basilisk.processesState` context key (#147). */
  private fetchState: ProcessesState = "loading";
  private sortMode: SortMode = "cpu";
  private groupMode: GroupMode = "none";
  private filterText = "";
  /** PID currently being CPU-profiled, so its row renders a Stop affordance. */
  private activeProfilingPid: number | undefined;
  /** PID of the active Basilisk debuggee, the only row that can be memory-tracked. */
  private activeDebuggeePid: number | undefined;

  constructor(private readonly store: Store) {}

  public refresh(): void {
    this.fetched = false;
    this.emitter.fire(undefined);
  }

  /**
   * The current process-fetch state — the e2e seam for [PROFILE-PROCESSES-PANEL]
   * empty-state honesty (#147), mirrored to the `basilisk.processesState` key.
   */
  public get processesState(): ProcessesState {
    return this.fetchState;
  }

  /** Record the fetch state and mirror it to the context key gating the welcome. */
  private setProcessesState(state: ProcessesState): void {
    this.fetchState = state;
    void vscode.commands.executeCommand("setContext", PROCESSES_STATE_CONTEXT_KEY, state);
  }

  /**
   * Mark which PID is being CPU-profiled ([PROFILE-PROCESSES-REACTIVE]). The
   * reactive wiring calls this then `refresh()`, so the next render distinguishes
   * the active row; pass `undefined` to clear.
   */
  public setActiveProfilingPid(pid: number | undefined): void {
    this.activeProfilingPid = pid;
  }

  /**
   * Mark which PID is the active Basilisk debuggee — the only row whose inline
   * Track Memory action is shown, since tracemalloc can only target a process
   * Basilisk launched ([PROFILE-MEMORY-HOWTO]). Pass `undefined` to clear.
   */
  public setActiveDebuggeePid(pid: number | undefined): void {
    this.activeDebuggeePid = pid;
  }

  /**
   * Fetch fresh process data, then repaint — awaitable, so the manual Refresh
   * command can run it under the view's progress bar ([PROFILE-UX-PROGRESS]).
   * The silent timer-driven [`refresh`] stays untouched: a progress bar
   * flashing every poll tick would be noise, not feedback.
   */
  public async refreshNow(): Promise<void> {
    await this.fetchProcesses();
    this.emitter.fire(undefined);
  }

  public cycleSortMode(): void {
    const idx = SORT_CYCLE.indexOf(this.sortMode);
    this.sortMode = SORT_CYCLE[(idx + 1) % SORT_CYCLE.length];
    void vscode.window.setStatusBarMessage(`Python Processes sorted by ${SORT_LABEL[this.sortMode]}`, STATUS_HINT_MS);
    this.emitter.fire(undefined);
  }

  public cycleGroupMode(): void {
    const idx = GROUP_CYCLE.indexOf(this.groupMode);
    this.groupMode = GROUP_CYCLE[(idx + 1) % GROUP_CYCLE.length];
    void vscode.window.setStatusBarMessage(`Python Processes grouped by ${GROUP_LABEL[this.groupMode]}`, STATUS_HINT_MS);
    this.emitter.fire(undefined);
  }

  public setFilter(text: string): void {
    this.filterText = text.trim().toLowerCase();
    this.emitter.fire(undefined);
  }

  public dispose(): void {
    for (const d of this.disposables) { d.dispose(); }
    this.emitter.dispose();
  }

  public getTreeItem(element: TreeItem): vscode.TreeItem {
    return element;
  }

  public async getChildren(element?: TreeItem): Promise<TreeItem[]> {
    if (element instanceof ProcessGroupItem) {
      // Re-sort so non-debuggable rows sink to the bottom *within the group* too,
      // not just in the flat list ([PROFILE-PROCESSES-DISPLAY]).
      return this.sortProcesses([...element.members]).map(
        (proc) => new ProcessTreeItem(proc, this.activeProfilingPid, this.activeDebuggeePid),
      );
    }
    if (
      element instanceof ProcessTreeItem ||
      element instanceof MessageTreeItem ||
      element instanceof LaunchActionItem
    ) {
      return [];
    }

    if (!this.fetched) {
      await this.fetchProcesses();
    }

    // With no processes at all, return [] so the empty/loading/error
    // `viewsWelcome` (which carries the big launch buttons) renders honestly.
    if (this.processes.length === 0) {
      return [];
    }

    // Processes exist → the welcome can't show, so pin the current-file launches
    // to the top as rows ([PROFILE-PROCESSES-LAUNCH-FILE]).
    const actions = this.launchActionRows();
    const visible = this.sortProcesses(this.applyFilter(this.processes));
    if (visible.length === 0) {
      // The user's search filter hid every running process — keep the launches
      // and an honest placeholder rather than an empty list (procexp-2).
      return [...actions, new MessageTreeItem(this.filteredEmptyLabel())];
    }
    const rows = this.groupMode === "none"
      ? visible.map((proc) => new ProcessTreeItem(proc, this.activeProfilingPid, this.activeDebuggeePid))
      : this.buildGroups(visible);
    return [...actions, ...rows];
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

  /** Why the filtered view is empty though processes are running (procexp-2). */
  private filteredEmptyLabel(): string {
    return `No process matches "${this.filterText}" (${this.processes.length} running)`;
  }

  /**
   * Apply only the user's explicit search filter. Enumeration is zero-filter
   * ([PROFILE-PROCESSES-SCOPE]); the panel never auto-hides a process.
   */
  private applyFilter(processes: readonly ProcessInfo[]): ProcessInfo[] {
    if (this.filterText === "") { return [...processes]; }
    return processes.filter((proc) => {
      const haystack = `${proc.name} ${proc.script ?? ""} ${proc.pid}`.toLowerCase();
      return haystack.includes(this.filterText);
    });
  }

  /**
   * Sort by the active mode, but always sink non-`debuggable` rows to the bottom
   * so the processes the user can act on stay on top ([PROFILE-PROCESSES-DISPLAY]).
   */
  private sortProcesses(processes: ProcessInfo[]): ProcessInfo[] {
    const byMode = SORT_COMPARATORS[this.sortMode];
    return processes.sort((a, b) => {
      if (a.debuggable !== b.debuggable) { return a.debuggable ? -1 : 1; }
      return byMode(a, b);
    });
  }

  private buildGroups(processes: readonly ProcessInfo[]): ProcessGroupItem[] {
    const groups = new Map<string, ProcessInfo[]>();
    for (const proc of processes) {
      const key = this.groupKey(proc);
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

  private groupKey(proc: ProcessInfo): string {
    switch (this.groupMode) {
      case "version": return proc.pythonVersion ?? "Unknown version";
      case "interpreter": return proc.interpreterPath ?? proc.name;
      case "user": return proc.user ?? "Unknown user";
      case "parent": return `Parent ${proc.ppid}`;
      case "none": return "";
    }
  }

  private async fetchProcesses(): Promise<void> {
    const client = this.store.client.value;
    if (!client?.isRunning()) {
      // Can't fetch yet — stay honestly "loading"; the serverState welcome shows
      // the connecting/stopped copy. Never assert "no processes" here (#147).
      this.processes = [];
      this.fetched = true;
      this.setProcessesState("loading");
      return;
    }
    try {
      const result = await client.sendRequest<ProcessesResponse>(
        "workspace/executeCommand",
        { command: LSP_CMD.processes, arguments: [{}] },
      );
      this.processes = result?.processes ?? [];
      this.fetched = true;
      // Only now is an empty list a genuine "no processes" rather than a lie (#147).
      this.setProcessesState("loaded");
    } catch (err: unknown) {
      Logger.error(`Python Processes fetch failed: ${err instanceof Error ? err.message : String(err)}`);
      this.processes = [];
      this.fetched = true;
      this.setProcessesState("error");
    }
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

/** Default poll interval (ms) when the setting is absent. */
const DEFAULT_REFRESH_MS = 2000;

/**
 * Register the Python Processes panel. Returns command disposables for
 * `singletonDisposables`; the tree view and provider go to subscriptions.
 */
export function registerPythonProcesses(
  context: vscode.ExtensionContext,
  store: Store,
): { provider: PythonProcessesProvider; disposables: vscode.Disposable[] } {
  const provider = new PythonProcessesProvider(store);

  // Seed the welcome gate honestly: until the first fetch resolves the panel is
  // "loading", never "no processes" ([PROFILE-PROCESSES-PANEL], #147).
  void vscode.commands.executeCommand("setContext", PROCESSES_STATE_CONTEXT_KEY, "loading");

  const treeView = vscode.window.createTreeView("basilisk.pythonProcesses", {
    treeDataProvider: provider,
  });
  context.subscriptions.push(treeView, provider);

  wireVisibilityRefresh(treeView, provider);
  // React to the store's profiling state: live chrome, button-gating context
  // keys, and the active-row marker ([PROFILE-PROCESSES-REACTIVE]).
  provider.disposables.push(bindProcessPanelReactivity(store, treeView, provider));
  // Reveal the inline Track Memory action only on the active debuggee row
  // ([PROFILE-PROCESSES-PANEL]) — memory tracking can't target external processes.
  provider.disposables.push(bindDebuggeeTracking(store, provider));
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

/**
 * Auto-refresh the panel on a timer only while it is visible, so polling the
 * LSP stops when the user navigates away.
 */
function wireVisibilityRefresh(
  treeView: vscode.TreeView<TreeItem>,
  provider: PythonProcessesProvider,
): void {
  let timer: ReturnType<typeof setInterval> | undefined;

  function start(): void {
    if (timer !== undefined) { return; }
    const intervalMs = vscode.workspace
      .getConfiguration("basilisk")
      .get<number>("profiler.processRefreshMs", DEFAULT_REFRESH_MS);
    timer = setInterval(() => { provider.refresh(); }, intervalMs);
  }
  function stop(): void {
    if (timer !== undefined) { clearInterval(timer); timer = undefined; }
  }

  provider.disposables.push(
    treeView.onDidChangeVisibility((e) => { if (e.visible) { start(); } else { stop(); } }),
    { dispose: stop },
  );
  if (treeView.visible) { start(); }
}
