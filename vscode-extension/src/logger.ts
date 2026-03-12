/**
 * Logging abstraction for the Basilisk VS Code extension.
 *
 * Provides a backend-agnostic `Logger` interface so callers never
 * depend on VS Code APIs, the filesystem, or any concrete sink.
 * Swap the backend by calling `setLogBackend()`.
 */

// ── Public interface ─────────────────────────────────────────────────────

export enum LogLevel {
  Trace,
  Debug,
  Info,
  Warn,
  Error,
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
class NullSink implements LogSink {
  trace(): void { /* noop */ }
  debug(): void { /* noop */ }
  info(): void { /* noop */ }
  warn(): void { /* noop */ }
  error(): void { /* noop */ }
}

/** Sink that fans out to multiple backends. */
class CompositeSink implements LogSink {
  constructor(private readonly sinks: LogSink[]) {}
  trace(message: string): void { for (const s of this.sinks) s.trace(message); }
  debug(message: string): void { for (const s of this.sinks) s.debug(message); }
  info(message: string): void { for (const s of this.sinks) s.info(message); }
  warn(message: string): void { for (const s of this.sinks) s.warn(message); }
  error(message: string): void { for (const s of this.sinks) s.error(message); }
}

/** Sink that appends to a file on disk via synchronous writes. */
export class FileLogSink implements LogSink {
  private readonly fd: number;
  constructor(filePath: string) {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const fs = require("fs") as typeof import("fs");
    // Open for append+create, truncating any previous run.
    this.fd = fs.openSync(filePath, "w");
  }
  trace(message: string): void { this.write("TRACE", message); }
  debug(message: string): void { this.write("DEBUG", message); }
  info(message: string): void { this.write("INFO ", message); }
  warn(message: string): void { this.write("WARN ", message); }
  error(message: string): void { this.write("ERROR", message); }
  private write(level: string, message: string): void {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const fs = require("fs") as typeof import("fs");
    const timestamp = new Date().toISOString();
    fs.writeSync(this.fd, `${timestamp} [${level}] ${message}\n`);
  }
}

// ── Singleton logger ────────────────────────────────────────────────────

let activeSink: LogSink = new NullSink();

/** Replace the active log backend. Pass an array to fan out to multiple sinks. */
export function setLogBackend(sink: LogSink | LogSink[]): void {
  activeSink = Array.isArray(sink) ? new CompositeSink(sink) : sink;
}

/** The global logger. Always safe to call — defaults to a no-op sink. */
export const logger: LogSink = {
  trace(message: string): void { activeSink.trace(message); },
  debug(message: string): void { activeSink.debug(message); },
  info(message: string): void { activeSink.info(message); },
  warn(message: string): void { activeSink.warn(message); },
  error(message: string): void { activeSink.error(message); },
};
