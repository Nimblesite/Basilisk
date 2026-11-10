// Implements [VSIX-OUTPUT-CHANNELS]. See docs/specs/VSIX-SPEC.md#VSIX-OUTPUT-CHANNELS
/**
 * The "Basilisk LSP Trace" output channel handed to vscode-languageclient as
 * `traceOutputChannel` — the user-facing observability surface for LSP
 * request/response traffic (GitHub #201).
 *
 * vscode-languageclient 10 only enables tracing while the trace channel's own
 * `logLevel` is `Trace` (`refreshTrace` in its client.js); the documented
 * `basilisk.trace.server` setting is consulted strictly after that gate. A
 * real `LogOutputChannel` defaults to `Info` — leaving the setting a no-op
 * and the channel permanently blank. This adapter makes the setting the real
 * switch: its `logLevel` derives from `basilisk.trace.server` (`Trace` for
 * "messages"/"verbose", `Info` for "off") and it fires `onDidChangeLogLevel`
 * on setting changes, which the client observes to re-evaluate trace mode.
 *
 * VS Code offers no API to read an output channel back, so writes are also
 * recorded for the e2e seam (`lspTraceLines`), mirroring the
 * `memoryStatusText()` seam pattern in memory-status.ts.
 */

import * as vscode from "vscode";

/** Lines written to the visible trace channel, in write order. */
const writtenLines: string[] = [];

/** E2E seam: every line written to the "Basilisk LSP Trace" channel. */
export function lspTraceLines(): readonly string[] {
  return writtenLines;
}

/** The channel's effective log level: `Trace` iff `basilisk.trace.server` is on. */
function configuredLevel(): vscode.LogLevel {
  const setting =
    vscode.workspace.getConfiguration("basilisk").get<string>("trace.server") ?? "off";
  return setting === "off" ? vscode.LogLevel.Info : vscode.LogLevel.Trace;
}

/** Severity labels rendered into trace lines, keyed by log level. */
const LEVEL_LABELS: Partial<Record<vscode.LogLevel, string>> = {
  [vscode.LogLevel.Trace]: "trace",
  [vscode.LogLevel.Debug]: "debug",
  [vscode.LogLevel.Info]: "info",
  [vscode.LogLevel.Warning]: "warning",
  [vscode.LogLevel.Error]: "error",
};

/** Emit one line at `level` iff the channel's current level admits it. */
function writeAt(
  channel: vscode.OutputChannel,
  level: vscode.LogLevel,
  message: string
): void {
  if (level < configuredLevel()) {
    return;
  }
  const line = `${new Date().toISOString()} [${LEVEL_LABELS[level] ?? "info"}] ${message}`;
  writtenLines.push(line);
  channel.appendLine(line);
}

/** The level-tagged log methods of the `LogOutputChannel` contract. */
type LeveledMethods = Pick<
  vscode.LogOutputChannel,
  "trace" | "debug" | "info" | "warn" | "error"
>;

/** Build the level-tagged log methods writing through `writeAt`. */
function leveledMethods(channel: vscode.OutputChannel): LeveledMethods {
  return {
    trace: (message: string): void => writeAt(channel, vscode.LogLevel.Trace, message),
    debug: (message: string): void => writeAt(channel, vscode.LogLevel.Debug, message),
    info: (message: string): void => writeAt(channel, vscode.LogLevel.Info, message),
    warn: (message: string): void => writeAt(channel, vscode.LogLevel.Warning, message),
    error: (error: string | Error): void =>
      writeAt(channel, vscode.LogLevel.Error, typeof error === "string" ? error : error.message),
  };
}

/** Fire the log-level event when a `basilisk.trace.server` change flips it. */
function watchTraceSetting(
  emitter: vscode.EventEmitter<vscode.LogLevel>
): vscode.Disposable {
  let lastLevel = configuredLevel();
  return vscode.workspace.onDidChangeConfiguration((event) => {
    if (!event.affectsConfiguration("basilisk.trace.server")) {
      return;
    }
    const level = configuredLevel();
    if (level !== lastLevel) {
      lastLevel = level;
      emitter.fire(level);
    }
  });
}

/**
 * Create the "Basilisk LSP Trace" channel for the LanguageClient.
 *
 * Satisfies the `LogOutputChannel` shape vscode-languageclient 10 requires,
 * but the sink is a plain output channel so admitted lines always render —
 * a real `LogOutputChannel` would re-filter them by its own UI-set level.
 */
export function createLspTraceChannel(): vscode.LogOutputChannel {
  const channel = vscode.window.createOutputChannel("Basilisk LSP Trace");
  const levelEmitter = new vscode.EventEmitter<vscode.LogLevel>();
  const settingWatcher = watchTraceSetting(levelEmitter);
  return {
    name: channel.name,
    get logLevel(): vscode.LogLevel {
      return configuredLevel();
    },
    onDidChangeLogLevel: levelEmitter.event,
    ...leveledMethods(channel),
    append: (value: string): void => {
      writtenLines.push(value);
      channel.append(value);
    },
    appendLine: (value: string): void => {
      writtenLines.push(value);
      channel.appendLine(value);
    },
    replace: (value: string): void => {
      writtenLines.length = 0;
      writtenLines.push(value);
      channel.replace(value);
    },
    clear: (): void => {
      writtenLines.length = 0;
      channel.clear();
    },
    show: (
      columnOrPreserveFocus?: vscode.ViewColumn | boolean,
      preserveFocus?: boolean
    ): void => {
      const focus =
        typeof columnOrPreserveFocus === "boolean" ? columnOrPreserveFocus : preserveFocus;
      channel.show(focus);
    },
    hide: (): void => {
      channel.hide();
    },
    dispose: (): void => {
      settingWatcher.dispose();
      levelEmitter.dispose();
      channel.dispose();
    },
  };
}
