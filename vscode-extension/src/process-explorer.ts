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

// ── LSP response types ───────────────────────────────────────────────────

/** One attachable Python process; mirrors the LSP `ProcessInfo` model. */
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
  readonly kind: "interpreter" | "launcher";
}

interface ProcessesResponse {
  readonly processes: readonly ProcessInfo[];
}

/** LSP command name (must match basilisk-common constants). */
const LSP_CMD = {
  processes: "basilisk.profiler.processes",
} as const;

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

type TreeItem = ProcessGroupItem | ProcessTreeItem;

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

/** A single process row with a one-click profiling affordance. */
class ProcessTreeItem extends vscode.TreeItem {
  constructor(public readonly process: ProcessInfo) {
    const scriptName = process.script !== null ? basename(process.script) : undefined;
    super(
      scriptName !== undefined ? `${process.name} — ${scriptName}` : process.name,
      vscode.TreeItemCollapsibleState.None,
    );

    // Stable identity across the panel's 2s auto-refresh: without it VS Code
    // can fail to map an inline-button click back to a (recreated) element and
    // invokes the command with no argument (#79).
    this.id = `pythonProcess:${process.pid}`;

    const version = process.pythonVersion ?? "—";
    this.description =
      `PID ${process.pid} · ${version} · ${process.cpuPercent.toFixed(1)}% · ${formatBytes(process.memoryBytes)}`;

    this.tooltip = [
      `${process.name} (PID ${process.pid})`,
      process.interpreterPath !== null ? `Interpreter: ${process.interpreterPath}` : "",
      process.script !== null ? `Script: ${process.script}` : "",
      `Python: ${process.pythonVersion ?? "unknown"}`,
      `CPU: ${process.cpuPercent.toFixed(1)}%  ·  Memory: ${formatBytes(process.memoryBytes)}`,
      `Runtime: ${formatRuntime(process.runtimeSecs)}`,
      process.user !== null ? `User: ${process.user}` : "",
      process.requiresElevation ? "⚠ Profiling this process will require elevation" : "",
    ].filter(Boolean).join("\n");

    this.iconPath = processIcon(process);
    // `pythonProcessElevated` lets package.json offer a distinct affordance set.
    this.contextValue = process.requiresElevation ? "pythonProcessElevated" : "pythonProcess";

    if (process.script !== null) {
      this.command = {
        command: "vscode.open",
        title: "Reveal Script",
        arguments: [vscode.Uri.file(process.script)],
      };
    }
  }
}

/** Icon for a process row: a lock badge when elevation is required. */
function processIcon(process: ProcessInfo): vscode.ThemeIcon {
  if (process.requiresElevation) {
    return new vscode.ThemeIcon("lock", new vscode.ThemeColor("list.warningForeground"));
  }
  return process.kind === "launcher"
    ? new vscode.ThemeIcon("rocket")
    : new vscode.ThemeIcon("vm-running");
}

// ── Provider ─────────────────────────────────────────────────────────────

export class PythonProcessesProvider implements vscode.TreeDataProvider<TreeItem>, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<TreeItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  public readonly disposables: vscode.Disposable[] = [];
  private processes: readonly ProcessInfo[] = [];
  private fetched = false;
  private sortMode: SortMode = "cpu";
  private groupMode: GroupMode = "none";
  private filterText = "";

  constructor(private readonly store: Store) {}

  public refresh(): void {
    this.fetched = false;
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
      return element.members.map((proc) => new ProcessTreeItem(proc));
    }
    if (element instanceof ProcessTreeItem) {
      return [];
    }

    if (!this.fetched) {
      await this.fetchProcesses();
    }

    const visible = this.sortProcesses(this.applyFilter(this.processes));
    if (this.groupMode === "none") {
      return visible.map((proc) => new ProcessTreeItem(proc));
    }
    return this.buildGroups(visible);
  }

  /** Apply the search filter and the "hide launchers" setting. */
  private applyFilter(processes: readonly ProcessInfo[]): ProcessInfo[] {
    const showLaunchers = vscode.workspace
      .getConfiguration("basilisk")
      .get<boolean>("profiler.showLaunchers", true);
    return processes.filter((proc) => {
      if (!showLaunchers && proc.kind === "launcher") { return false; }
      if (this.filterText === "") { return true; }
      const haystack = `${proc.name} ${proc.script ?? ""} ${proc.pid}`.toLowerCase();
      return haystack.includes(this.filterText);
    });
  }

  private sortProcesses(processes: ProcessInfo[]): ProcessInfo[] {
    switch (this.sortMode) {
      case "cpu": return processes.sort((a, b) => b.cpuPercent - a.cpuPercent);
      case "memory": return processes.sort((a, b) => b.memoryBytes - a.memoryBytes);
      case "pid": return processes.sort((a, b) => a.pid - b.pid);
      case "name": return processes.sort((a, b) => a.name.localeCompare(b.name));
      case "runtime": return processes.sort((a, b) => b.runtimeSecs - a.runtimeSecs);
      case "version": return processes.sort((a, b) => (a.pythonVersion ?? "").localeCompare(b.pythonVersion ?? ""));
    }
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
      this.processes = [];
      this.fetched = true;
      return;
    }
    try {
      const result = await client.sendRequest<ProcessesResponse>(
        "workspace/executeCommand",
        { command: LSP_CMD.processes, arguments: [{}] },
      );
      this.processes = result?.processes ?? [];
      this.fetched = true;
    } catch (err: unknown) {
      Logger.error(`Python Processes fetch failed: ${err instanceof Error ? err.message : String(err)}`);
      this.processes = [];
      this.fetched = true;
    }
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

  const treeView = vscode.window.createTreeView("basilisk.pythonProcesses", {
    treeDataProvider: provider,
  });
  context.subscriptions.push(treeView, provider);

  wireVisibilityRefresh(treeView, provider);

  const disposables = [
    ...registerLaunchCommands(store, treeView),
    vscode.commands.registerCommand("basilisk.refreshProcesses", () => { provider.refresh(); }),
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
