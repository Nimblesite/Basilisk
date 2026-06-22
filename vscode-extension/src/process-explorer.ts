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

// ── LSP response types ───────────────────────────────────────────────────

/** One Python process; mirrors the LSP `ProcessInfo` model ([PROFILE-PROCESSES-MODEL]). */
export interface ProcessInfo {
  readonly pid: number;
  readonly ppid: number;
  readonly name: string;
  readonly interpreterPath: string | null;
  readonly script: string | null;
  readonly pythonVersion: string | null;
  readonly cpuPercent: number;
  readonly memoryBytes: number;
  readonly runtimeSecs: number;
  readonly user: string | null;
  readonly requiresElevation: boolean;
  /** `true` when the process belongs to an open workspace root — renders green. */
  readonly inWorkspace: boolean;
  /** Framework name (`uvicorn`, …) when a known launcher, else `null` — chip. */
  readonly launcher: string | null;
  /** `false` when the profiler can't attach — renders 🚫, greyed, sorted last. */
  readonly debuggable: boolean;
  /** Tooltip reason shown when `debuggable` is `false`. */
  readonly undebuggableReason: string | null;
}

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

// ── Sort / group modes ───────────────────────────────────────────────────

type SortMode = "cpu" | "memory" | "pid" | "name" | "runtime" | "version";

const SORT_CYCLE: readonly SortMode[] = ["cpu", "memory", "pid", "name", "runtime", "version"];

const SORT_LABEL: Readonly<Record<SortMode, string>> = {
  cpu: "CPU %",
  memory: "Memory",
  pid: "PID",
  name: "Name",
  runtime: "Runtime",
  version: "Python version",
};

/** Per-mode row comparators; `sortProcesses` wraps these to sink non-debuggable rows. */
const SORT_COMPARATORS: Readonly<Record<SortMode, (a: ProcessInfo, b: ProcessInfo) => number>> = {
  cpu: (a, b) => b.cpuPercent - a.cpuPercent,
  memory: (a, b) => b.memoryBytes - a.memoryBytes,
  pid: (a, b) => a.pid - b.pid,
  name: (a, b) => a.name.localeCompare(b.name),
  runtime: (a, b) => b.runtimeSecs - a.runtimeSecs,
  version: (a, b) => (a.pythonVersion ?? "").localeCompare(b.pythonVersion ?? ""),
};

type GroupMode = "none" | "version" | "interpreter" | "user" | "parent";

const GROUP_CYCLE: readonly GroupMode[] = ["none", "version", "interpreter", "user", "parent"];

const GROUP_LABEL: Readonly<Record<GroupMode, string>> = {
  none: "None",
  version: "Python version",
  interpreter: "Interpreter",
  user: "User",
  parent: "Parent process",
};

// ── Formatting helpers ───────────────────────────────────────────────────

const BYTES_PER_UNIT = 1024;
const MEMORY_UNITS: readonly string[] = ["B", "KB", "MB", "GB", "TB"];
const SECONDS_PER_MINUTE = 60;
const SECONDS_PER_HOUR = 3600;

/** Human-readable memory size, e.g. `88 MB`. */
function formatBytes(bytes: number): string {
  if (bytes <= 0) { return "0 B"; }
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(BYTES_PER_UNIT)),
    MEMORY_UNITS.length - 1,
  );
  const value = bytes / BYTES_PER_UNIT ** exponent;
  const rounded = exponent === 0 ? value : Math.round(value);
  return `${rounded} ${MEMORY_UNITS[exponent]}`;
}

/** Human-readable elapsed runtime, e.g. `3m`, `2h 14m`. */
function formatRuntime(seconds: number): string {
  if (seconds < SECONDS_PER_MINUTE) { return `${seconds}s`; }
  if (seconds < SECONDS_PER_HOUR) { return `${Math.floor(seconds / SECONDS_PER_MINUTE)}m`; }
  const hours = Math.floor(seconds / SECONDS_PER_HOUR);
  const minutes = Math.floor((seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  return `${hours}h ${minutes}m`;
}

/** Final path component, used to render a script's basename. */
function basename(path: string): string {
  const lastSep = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return lastSep >= 0 ? path.slice(lastSep + 1) : path;
}

// ── Tree items ───────────────────────────────────────────────────────────

type TreeItem = LaunchActionItem | ProcessGroupItem | ProcessTreeItem | MessageTreeItem;

/**
 * A non-process placeholder row. Returned (instead of an empty list) when the
 * user's search filter empties a NON-empty process list, so the empty-tree
 * viewsWelcome never claims "No Python processes running" while processes are in
 * fact running, just filtered (procexp-2).
 */
class MessageTreeItem extends vscode.TreeItem {
  constructor(label: string) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.contextValue = "processesMessage";
    this.iconPath = new vscode.ThemeIcon("filter");
  }
}

/**
 * A persistent "Run & …(Current File)" launch row pinned to the top of the tree.
 * VS Code only renders the big `viewsWelcome` buttons when the tree is EMPTY, so
 * once any process appears those buttons vanish — these rows keep the current-file
 * launches reachable alongside a populated list ([PROFILE-PROCESSES-LAUNCH-FILE]).
 * Gated per-activity (hidden while the matching metric is busy) like the title-bar
 * buttons, so a second same-metric run is never offered ([PROFILE-PROCESSES-REACTIVE]).
 */
class LaunchActionItem extends vscode.TreeItem {
  constructor(label: string, command: string, icon: string) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.id = `launchAction:${command}`;
    this.contextValue = "launchAction";
    this.iconPath = new vscode.ThemeIcon(icon, new vscode.ThemeColor("textLink.foreground"));
    this.command = { command, title: label };
  }
}

/** A collapsible group header (when grouping is active). */
class ProcessGroupItem extends vscode.TreeItem {
  constructor(
    public readonly label: string,
    public readonly members: readonly ProcessInfo[],
  ) {
    super(label, vscode.TreeItemCollapsibleState.Expanded);
    // Stable identity so VS Code preserves expansion state across refreshes.
    this.id = `processGroup:${label}`;
    this.description = `${members.length}`;
    this.contextValue = "processGroup";
    this.iconPath = new vscode.ThemeIcon("folder");
  }
}

/** 🚫 marker prefixed to the label of a process the profiler can't attach to. */
const BLOCKED_MARK = "🚫 ";

/**
 * A single process row. Carries the visual cues for [PROFILE-PROCESSES-DISPLAY]:
 * a 🚫-prefixed, greyed, sunk row when not `debuggable`; a green row when
 * `inWorkspace`; a launcher chip; and the flame for the actively-profiled row.
 */
class ProcessTreeItem extends vscode.TreeItem {
  constructor(
    public readonly process: ProcessInfo,
    activeProfilingPid?: number,
    activeDebuggeePid?: number,
  ) {
    const scriptName = process.script !== null ? basename(process.script) : undefined;
    const base = scriptName !== undefined ? `${process.name} — ${scriptName}` : process.name;
    super(
      process.debuggable ? base : `${BLOCKED_MARK}${base}`,
      vscode.TreeItemCollapsibleState.None,
    );

    // Stable identity across the panel's 2s auto-refresh: without it VS Code
    // can fail to map an inline-button click back to a (recreated) element and
    // invokes the command with no argument (#79).
    this.id = `pythonProcess:${process.pid}`;

    // The row currently being CPU-profiled gets a distinct look + contextValue
    // so package.json swaps its inline Profile button for a Stop button
    // ([PROFILE-PROCESSES-REACTIVE]).
    const profilingThis = activeProfilingPid !== undefined && activeProfilingPid === process.pid;
    // Only the active Basilisk debuggee can be memory-tracked (tracemalloc rides
    // the DAP courier — [PROFILE-MEMORY-HOWTO]); its row gets a contextValue that
    // reveals the inline Track Memory action, hidden everywhere else so the
    // action is never offered where it would just refuse.
    const memoryTrackable = activeDebuggeePid !== undefined && activeDebuggeePid === process.pid;
    this.description = rowDescription(process, profilingThis);
    this.tooltip = rowTooltip(process, profilingThis, memoryTrackable);
    this.iconPath = processIcon(process, profilingThis);
    this.contextValue = rowContextValue(process, profilingThis, memoryTrackable);
    // The FileDecorationProvider keys off this synthetic URI to colour the whole
    // label green (workspace) or grey (non-debuggable) — [PROFILE-PROCESSES-DISPLAY].
    this.resourceUri = processResourceUri(process);

    if (process.script !== null) {
      this.command = {
        command: "vscode.open",
        title: "Reveal Script",
        arguments: [vscode.Uri.file(process.script)],
      };
    }
  }
}

/** Scheme of the synthetic per-row URI the decoration provider colours. */
const PROCESS_URI_SCHEME = "basilisk-process";

/** A synthetic URI encoding the row's workspace + debuggability for decoration. */
function processResourceUri(process: ProcessInfo): vscode.Uri {
  const ws = process.inWorkspace ? "1" : "0";
  const dbg = process.debuggable ? "1" : "0";
  return vscode.Uri.from({ scheme: PROCESS_URI_SCHEME, path: `/${process.pid}`, query: `ws=${ws}&dbg=${dbg}` });
}

/** The launcher chip (`[uvicorn] `) for a row's description, or empty. */
function launcherChip(process: ProcessInfo): string {
  return process.launcher !== null ? `[${process.launcher}] ` : "";
}

/** The description line: launcher chip, then the key live metrics. */
function rowDescription(process: ProcessInfo, profilingThis: boolean): string {
  const version = process.pythonVersion ?? "—";
  const profilingSuffix = profilingThis ? " · profiling" : "";
  return `${launcherChip(process)}PID ${process.pid} · ${version} · ${process.cpuPercent.toFixed(1)}% · ${formatBytes(process.memoryBytes)}${profilingSuffix}`;
}

/**
 * The row's contextValue, which selects its package.json affordances:
 * `pythonProcessProfiling` (Stop), `pythonProcessDebuggee` (Track Memory enabled),
 * `pythonProcessElevated` (lock), or plain `pythonProcess`.
 */
function rowContextValue(process: ProcessInfo, profilingThis: boolean, memoryTrackable: boolean): string {
  if (profilingThis) { return "pythonProcessProfiling"; }
  if (memoryTrackable) { return "pythonProcessDebuggee"; }
  return process.requiresElevation ? "pythonProcessElevated" : "pythonProcess";
}

/** Multi-line hover tooltip surfacing every resolved detail for a process row. */
function rowTooltip(process: ProcessInfo, profilingThis: boolean, memoryTrackable: boolean): string {
  return [
    `${process.name} (PID ${process.pid})`,
    process.interpreterPath !== null ? `Interpreter: ${process.interpreterPath}` : "",
    process.script !== null ? `Script: ${process.script}` : "",
    `Python: ${process.pythonVersion ?? "unknown"}`,
    `CPU: ${process.cpuPercent.toFixed(1)}%  ·  Memory: ${formatBytes(process.memoryBytes)}`,
    `Runtime: ${formatRuntime(process.runtimeSecs)}`,
    process.user !== null ? `User: ${process.user}` : "",
    process.launcher !== null ? `Launcher: ${process.launcher}` : "",
    process.inWorkspace ? "📁 Workspace process" : "",
    profilingThis ? "🔥 Basilisk is profiling this process — click Stop to finish" : "",
    !process.debuggable && process.undebuggableReason !== null
      ? `🚫 Can't profile — ${process.undebuggableReason}`
      : "",
    // An external process is profilable only with elevation.
    process.debuggable && process.requiresElevation
      ? "🔒 Profiling this process will prompt for elevation"
      : "",
    // Memory tracking (tracemalloc via the DAP courier — [PROFILE-MEMORY-HOWTO])
    // only works on a process Basilisk launched, so it is offered only on the
    // active debuggee; elsewhere point at the current-file launch.
    process.debuggable && !memoryTrackable
      ? "🧠 Memory tracking needs the process under Basilisk — use “Run & Track Memory (Current File)”"
      : "",
  ].filter(Boolean).join("\n");
}

/**
 * Leading icon for a process row: the flame while profiling, `circle-slash` when
 * not debuggable, a `lock` when it needs elevation, `rocket` for launchers, else
 * a running-VM glyph. In-workspace debuggable rows are tinted green to match the
 * label decoration.
 */
function processIcon(process: ProcessInfo, profilingThis: boolean): vscode.ThemeIcon {
  if (profilingThis) {
    return new vscode.ThemeIcon("flame", new vscode.ThemeColor("statusBarItem.warningBackground"));
  }
  if (!process.debuggable) {
    return new vscode.ThemeIcon("circle-slash", new vscode.ThemeColor("disabledForeground"));
  }
  if (process.requiresElevation) {
    return new vscode.ThemeIcon("lock", new vscode.ThemeColor("list.warningForeground"));
  }
  const glyph = process.launcher !== null ? "rocket" : "vm-running";
  return process.inWorkspace
    ? new vscode.ThemeIcon(glyph, new vscode.ThemeColor("charts.green"))
    : new vscode.ThemeIcon(glyph);
}

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
