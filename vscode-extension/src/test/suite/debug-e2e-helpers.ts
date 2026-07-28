// Implements [PROFILE-MEMORY-HOWTO] + [PROFILE-MEMORY-INGEST].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-HOWTO
//
// Shared debug-driving + editor-as-courier helpers for the memory e2e suites.
// The LSP holds no DAP connection, so every memory command is a round-trip: the
// LSP hands back a Python injection script, the editor runs it in the paused
// debuggee via DAP `evaluate`, and posts the raw output to
// `basilisk.memory.ingest`. These helpers drive a real `basilisk-debug` session
// and run that round-trip — no mocks. Centralised here so the snapshot/diff
// suite and the introspection (reference-graph / gc-collect) suite share one
// implementation instead of duplicating it.

import * as assert from "assert";
import * as vscode from "vscode";
import { currentStoppedFrameId, evaluateInDebugSession } from "../../dap-evaluate";
import { numberField, recordArrayField } from "../../unknown-shape";
import { pollUntilResult } from "./test-helpers";

/** Budget for a debug session to start / stop / pause. */
export const SESSION_WAIT_MS = 20_000;
/** Poll cadence for debug-state changes. */
export const POLL_MS = 100;

/** Replace all breakpoints with source breakpoints at the given 1-based lines. */
export function setBreakpoints(filePath: string, lines: number[]): void {
  vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  vscode.debug.addBreakpoints(
    lines.map(
      (line) =>
        new vscode.SourceBreakpoint(
          new vscode.Location(vscode.Uri.file(filePath), new vscode.Position(line - 1, 0)),
        ),
    ),
  );
}

/** Wait until the active debuggee is paused, returning the stopped frame id. */
export async function waitForPause(): Promise<number> {
  const frameId = await pollUntilResult({
    fn: async () => currentStoppedFrameId(),
    predicate: (frame) => frame !== null,
    timeoutMs: SESSION_WAIT_MS,
    intervalMs: POLL_MS,
  });
  assert.ok(frameId !== null, "debuggee must reach a paused state");
  return frameId;
}

/** Resume the debuggee (first stopped thread). */
export async function resume(): Promise<void> {
  const session = vscode.debug.activeDebugSession;
  assert.ok(session, "an active debug session is required to resume");
  const threads: unknown = await session.customRequest("threads");
  const threadId = numberField(recordArrayField(threads, "threads")[0], "id");
  assert.ok(threadId !== undefined, "the debuggee must report a thread");
  await session.customRequest("continue", { threadId });
}

/** Wait for the active debug session to terminate. */
export async function waitForSessionEnd(): Promise<void> {
  await pollUntilResult({
    fn: async () => vscode.debug.activeDebugSession,
    predicate: (session) => session === undefined,
    timeoutMs: SESSION_WAIT_MS,
    intervalMs: POLL_MS,
  });
}

/** One marker-tagged ingest result. */
export interface IngestResult {
  kind: string;
  [field: string]: unknown;
}

/**
 * Run one memory command's full courier round-trip against the paused debuggee:
 * fetch its injection script (leg 1), evaluate it in `frameId`, and post the
 * real output back through `basilisk.memory.ingest`.
 *
 * `ingestSessionId` routes the ingest; omit it for `basilisk.memory.start`,
 * whose session is minted in its own leg-1 response.
 */
export async function memoryCourier<T extends IngestResult>(opts: {
  command: string;
  leg1Args: Record<string, unknown>;
  frameId: number;
  ingestSessionId?: string;
}): Promise<T> {
  const leg1 = await vscode.commands.executeCommand<
    { memorySessionId?: string; script?: string } | null
  >(opts.command, opts.leg1Args);
  const script = leg1?.script;
  assert.ok(script !== undefined && script !== "", `${opts.command} must return an injection script`);

  const output = await evaluateInDebugSession(script, opts.frameId);
  assert.ok(output !== null, `${opts.command} script must evaluate in the paused debuggee`);

  const ingested = await vscode.commands.executeCommand<T | null>("basilisk.memory.ingest", {
    memorySessionId: opts.ingestSessionId ?? leg1?.memorySessionId,
    output,
  });
  assert.ok(ingested !== null, "ingest must return a kind-tagged result");
  return ingested;
}

/**
 * Convenience wrapper for the snapshot/diff/start commands, which take either
 * `{ tracebackDepth }` (start, minting a session) or `{ memorySessionId }`.
 */
export async function memoryRoundTrip<T extends IngestResult>(
  command: string,
  memorySessionId: string | undefined,
  frameId: number,
): Promise<T> {
  return memoryCourier<T>({
    command,
    leg1Args: memorySessionId === undefined ? { tracebackDepth: 25 } : { memorySessionId },
    frameId,
    ingestSessionId: memorySessionId,
  });
}
