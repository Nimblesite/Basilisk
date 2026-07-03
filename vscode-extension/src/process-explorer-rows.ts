// Implements [PROFILE-PROCESSES-DISPLAY]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-DISPLAY
/**
 * Python Processes panel — row presentation layer.
 *
 * Everything about how a process renders as a tree row lives here: the process
 * model, sort/group mode tables, formatting helpers, the tree-item classes, and
 * the row cue builders (description, tooltip, icon, contextValue, decoration
 * URI). The provider (fetching, sorting, grouping, registration) lives in
 * `process-explorer.ts`; split to keep both under the 500-LOC file limit.
 */

import * as vscode from "vscode";

// ── Process model ─────────────────────────────────────────────────────────

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

// ── Sort / group modes ───────────────────────────────────────────────────

export type SortMode = "cpu" | "memory" | "pid" | "name" | "runtime" | "version";

export const SORT_CYCLE: readonly SortMode[] = ["cpu", "memory", "pid", "name", "runtime", "version"];

export const SORT_LABEL: Readonly<Record<SortMode, string>> = {
  cpu: "CPU %",
  memory: "Memory",
  pid: "PID",
  name: "Name",
  runtime: "Runtime",
  version: "Python version",
};

/** Per-mode row comparators; `sortProcesses` wraps these to sink non-debuggable rows. */
export const SORT_COMPARATORS: Readonly<Record<SortMode, (a: ProcessInfo, b: ProcessInfo) => number>> = {
  cpu: (a, b) => b.cpuPercent - a.cpuPercent,
  memory: (a, b) => b.memoryBytes - a.memoryBytes,
  pid: (a, b) => a.pid - b.pid,
  name: (a, b) => a.name.localeCompare(b.name),
  runtime: (a, b) => b.runtimeSecs - a.runtimeSecs,
  version: (a, b) => (a.pythonVersion ?? "").localeCompare(b.pythonVersion ?? ""),
};

export type GroupMode = "none" | "version" | "interpreter" | "user" | "parent";

export const GROUP_CYCLE: readonly GroupMode[] = ["none", "version", "interpreter", "user", "parent"];

export const GROUP_LABEL: Readonly<Record<GroupMode, string>> = {
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

export type TreeItem = LaunchActionItem | ProcessGroupItem | ProcessTreeItem | MessageTreeItem;

/**
 * A non-process placeholder row. Returned (instead of an empty list) when the
 * user's search filter empties a NON-empty process list, so the empty-tree
 * viewsWelcome never claims "No Python processes running" while processes are in
 * fact running, just filtered (procexp-2).
 */
export class MessageTreeItem extends vscode.TreeItem {
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
export class LaunchActionItem extends vscode.TreeItem {
  constructor(label: string, command: string, icon: string) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.id = `launchAction:${command}`;
    this.contextValue = "launchAction";
    this.iconPath = new vscode.ThemeIcon(icon, new vscode.ThemeColor("textLink.foreground"));
    this.command = { command, title: label };
  }
}

/** A collapsible group header (when grouping is active). */
export class ProcessGroupItem extends vscode.TreeItem {
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
export class ProcessTreeItem extends vscode.TreeItem {
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
export const PROCESS_URI_SCHEME = "basilisk-process";

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
