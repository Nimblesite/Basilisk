// Implements [VSIX-OUTPUT-CHANNELS]. See docs/specs/VSIX-SPEC.md#VSIX-OUTPUT-CHANNELS
/**
 * Logging abstraction for the Basilisk VS Code extension.
 *
 * Provides a backend-agnostic `Logger` interface so callers never
 * depend on VS Code APIs, the filesystem, or any concrete sink.
 * The active sink is stored in the centralized Store — no global
 * mutable state lives in this module.
 */

import type FsModule from "fs";

// ── Public interface ─────────────────────────────────────────────────────

export enum LogLevel {
  Trace = 0,
  Debug = 1,
  Info = 2,
  Warn = 3,
  Error = 4,
}

/** Backend-agnostic logging interface. */
export interface LogSink {
  trace(message: string): void;
  debug(message: string): void;
  info(message: string): void;
  warn(message: string): void;
  error(message: string): void;
}

// ── Built-in sinks ──────────────────────────────────────────────────────

/** Sink that silently discards all messages. */
export const nullSink: LogSink = {
  trace(): void { /* noop */ },
  debug(): void { /* noop */ },
  info(): void { /* noop */ },
  warn(): void { /* noop */ },
  error(): void { /* noop */ },
};

/** Sink that fans out to multiple backends. */
export class CompositeSink implements LogSink {
  constructor(private readonly sinks: LogSink[]) {}
  public trace(message: string): void { for (const s of this.sinks) {s.trace(message);} }
  public debug(message: string): void { for (const s of this.sinks) {s.debug(message);} }
  public info(message: string): void { for (const s of this.sinks) {s.info(message);} }
  public warn(message: string): void { for (const s of this.sinks) {s.warn(message);} }
  public error(message: string): void { for (const s of this.sinks) {s.error(message);} }
}

/** Sink that appends to a file on disk via synchronous writes. */
export class FileLogSink implements LogSink {
  private readonly fd: number;
  constructor(filePath: string) {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const fsModule = require("fs") as typeof FsModule;
    // Open for append+create, truncating any previous run.
    this.fd = fsModule.openSync(filePath, "w");
  }
  public trace(message: string): void { this.write("TRACE", message); }
  public debug(message: string): void { this.write("DEBUG", message); }
  public info(message: string): void { this.write("INFO ", message); }
  public warn(message: string): void { this.write("WARN ", message); }
  public error(message: string): void { this.write("ERROR", message); }
  private write(level: string, message: string): void {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const fsModule = require("fs") as typeof FsModule;
    const timestamp = new Date().toISOString();
    fsModule.writeSync(this.fd, `${timestamp} [${level}] ${message}\n`);
  }
}

// ── Logger with injectable sink ─────────────────────────────────────────

/** Sink accessor — set once at startup by extension.ts via bindLogger(). */
// eslint-disable-next-line func-style
let sinkAccessor: () => LogSink = () => nullSink;

/**
 * Bind the Logger to a sink accessor function. Called once during
 * activate() so the Logger reads from the store without importing it
 * (avoiding circular deps).
 */
export function bindLogger(accessor: () => LogSink): void {
  sinkAccessor = accessor;
}

/** The global logger. Always safe to call — defaults to a no-op sink. */
export const Logger = {
  trace(message: string): void { sinkAccessor().trace(message); },
  debug(message: string): void { sinkAccessor().debug(message); },
  info(message: string): void { sinkAccessor().info(message); },
  warn(message: string): void { sinkAccessor().warn(message); },
  error(message: string): void { sinkAccessor().error(message); },
} as const;
