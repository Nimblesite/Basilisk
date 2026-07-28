// Implements [PROFILE-MEMORY-HOWTO] + [PROFILE-MEMORY-AUTOPILOT].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-HOWTO
/**
 * The editor-as-courier memory round-trip and result presentation.
 *
 * Extracted from memory-profiler.ts (which was over the 500 LOC limit) so the
 * command/lifecycle layer and the capture engine each stay small and focused.
 * Both the manual commands ([`memory-profiler.ts`](./memory-profiler.ts)) and the
 * autopilot ([`memory-autopilot.ts`](./memory-autopilot.ts)) drive memory through
 * the same primitives here, so an auto-capture is byte-for-byte the same flow as a
 * hand-clicked one — same purple track, leak decorations, dashboard, and timeline.
 *
 * Round-trip (one operation): ask the LSP for the injection script (leg 1), run it
 * in the paused debuggee via DAP `evaluate`, post the raw output to
 * `basilisk.memory.ingest` (leg 2), and present the structured result. A running
 * program is transparently paused for the script and resumed after
 * ([PROFILE-MEMORY-HOWTO]); a user's own breakpoint pause is left untouched.
 */

import { delay } from "./timeouts";
import * as vscode from "vscode";
import * as fs from "fs";
import { Logger } from "./logger";
import type { Store } from "./store";
import { acquireStoppedFrame, evaluateInDebugSession } from "./dap-evaluate";
import { withUserProgress } from "./progress-ops";
import {
  toDashboardDiff,
  toDashboardSnapshot,
  asNumber,
  type MemoryIngestResult,
} from "./memory-dashboard-mapping";
import {
  openMemoryDashboard,
  type MemoryDashboardSnapshot,
  type MemoryTimelinePoint,
} from "./memory-dashboard";
import {
  applyLeakDecorations,
  applyMemoryDecorations,
  type MemoryDiffResult,
  type MemorySnapshotResult,
} from "./memory-decorations";
// ── LSP command ids ─────────────────────────────────────────────────────────

/** The `basilisk.memory.*` LSP command names (one round-trip leg each). */
export const LSP_MEM_CMD = {
  start: "basilisk.memory.start",
  snapshot: "basilisk.memory.snapshot",
  diff: "basilisk.memory.diff",
  references: "basilisk.memory.references",
  objectsByType: "basilisk.memory.objectsByType",
  gcCollect: "basilisk.memory.gcCollect",
  ingest: "basilisk.memory.ingest",
} as const;

/** tracemalloc traceback depth injected at start. */
export const TRACEBACK_DEPTH = 25;

/** The progress title for starting memory tracking (shared by both entry points). */
export const MEM_START_TITLE = "Basilisk: Starting memory tracking";

/** Milliseconds per second (timeline x-axis is in seconds). */
const MS_PER_SECOND = 1000;

/** [PROFILE-UX-PROGRESS] Progress-notification titles, one per memory operation. */
const MEM_OP_TITLE: Readonly<Record<string, string>> = {
  [LSP_MEM_CMD.snapshot]: "Basilisk: Taking memory snapshot",
  [LSP_MEM_CMD.diff]: "Basilisk: Comparing memory snapshots",
  [LSP_MEM_CMD.gcCollect]: "Basilisk: Forcing garbage collection",
  [LSP_MEM_CMD.references]: "Basilisk: Building the reference graph",
};

/** [PROFILE-MEMORY-FINAL] How long to wait for the at-exit snapshot file after the
 *  program exits (covers the terminate-event/final-flush race), and the poll
 *  cadence while waiting. */
const FINAL_SNAPSHOT_WAIT_MS = 3000;
const FINAL_SNAPSHOT_POLL_MS = 100;

// ── In-flight guard ─────────────────────────────────────────────────────────
//
// [PROFILE-MEMORY-AUTOPILOT-PAUSE] A capture transparently pauses a running
// program, which emits its own `stopped` event; the autopilot must NOT treat that
// (or any in-progress manual op) as a fresh user pause. Every operation brackets
// itself with begin/end, so `isMemoryOperationInFlight()` is true for the whole
// pause→evaluate→resume window — the one synchronous fact both layers agree on.

let memoryOpsInFlight = 0;

/** True while any memory round-trip (manual or auto) is mid-flight. */
export function isMemoryOperationInFlight(): boolean {
  return memoryOpsInFlight > 0;
}

function beginMemoryOp(): void {
  memoryOpsInFlight += 1;
}

function endMemoryOp(): void {
  memoryOpsInFlight = Math.max(0, memoryOpsInFlight - 1);
}

// ── Presentation state ──────────────────────────────────────────────────────

/** Most recent snapshot, so a later "Compare" can show it alongside the diff. */
let lastDashboardSnapshot: MemoryDashboardSnapshot | undefined;

/**
 * Rolling timeline of every snapshot captured this session. The dashboard chart
 * comes alive across repeated (especially autopilot) captures — "watch the leak
 * grow" — instead of always reading "take multiple snapshots". Reset on stop.
 */
let captureTimeline: MemoryTimelinePoint[] = [];

/** Whether a snapshot has been captured (so "stop" can report honestly). */
export function hasCapturedSnapshot(): boolean {
  return lastDashboardSnapshot !== undefined;
}

/** Drop per-session presentation state (called when tracking stops). */
export function resetCaptureState(): void {
  lastDashboardSnapshot = undefined;
  captureTimeline = [];
}

// ── Single operation round-trip ─────────────────────────────────────────────

/** The running LSP client handle. */
type LspClient = NonNullable<Store["client"]["value"]>;

/** Everything one staged memory round-trip needs. */
interface MemoryOperation {
  readonly store: Store;
  readonly command: string;
  readonly extraArgs: Record<string, unknown>;
  readonly report: (message: string) => void;
  /** Suppress user-facing warnings (the autopilot captures silently). */
  readonly quiet: boolean;
}

/**
 * Resolve the running client + active memory session, or null (warning unless
 * `quiet`) when the LSP is down or tracking is not active.
 */
function resolveActiveSession(op: MemoryOperation): { client: LspClient; memorySessionId: string } | null {
  const client = op.store.client.value;
  function notConnected(): null {
    if (!op.quiet) { void vscode.window.showErrorMessage("Basilisk LSP not connected"); }
    return null;
  }
  if (client === undefined) { return notConnected(); }
  if (!client.isRunning()) { return notConnected(); }
  const memorySessionId = op.store.profiler.value.memorySessionId;
  if (op.store.profiler.value.memory !== "active" || memorySessionId === undefined) {
    if (!op.quiet) { void vscode.window.showWarningMessage("Basilisk: Start memory tracking first."); }
    return null;
  }
  return { client, memorySessionId };
}

/**
 * Run one memory operation's round-trip and return the LSP's structured result.
 *
 * Returns null (with an actionable message unless `quiet`) when there is no
 * session, the debuggee cannot be paused, or evaluation fails.
 */
async function runMemoryOperation(op: MemoryOperation): Promise<MemoryIngestResult | null> {
  const active = resolveActiveSession(op);
  if (active === null) { return null; }

  beginMemoryOp();
  op.report("Pausing the program…");
  const acquired = await acquireStoppedFrame();
  if (acquired === null) {
    endMemoryOp();
    if (!op.quiet) {
      void vscode.window.showWarningMessage(
        "Basilisk: Could not pause the program for memory inspection — pause at a breakpoint and retry.",
      );
    }
    return null;
  }

  try {
    return await evaluateAndIngest(op, {
      client: active.client,
      memorySessionId: active.memorySessionId,
      frameId: acquired.frameId,
    });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.warn(`[Memory] ${op.command} round-trip failed: ${msg}`);
    if (!op.quiet) { void vscode.window.showWarningMessage(`Basilisk: ${msg}`); }
    return null;
  } finally {
    await acquired.release();
    endMemoryOp();
  }
}

/** The acquired-frame context one round-trip evaluates against. */
interface OperationContext {
  readonly client: LspClient;
  readonly memorySessionId: string;
  readonly frameId: number;
}

/** Legs 1+2: fetch the injection script, run it in the frame, post the output to ingest. */
async function evaluateAndIngest(
  op: MemoryOperation,
  ctx: OperationContext,
): Promise<MemoryIngestResult | null> {
  const phase1 = await ctx.client.sendRequest<{ script?: string } | null>("workspace/executeCommand", {
    command: op.command,
    arguments: [{ memorySessionId: ctx.memorySessionId, ...op.extraArgs }],
  });
  const script = phase1?.script;
  if (script === undefined || script === "") { return null; }

  op.report("Inspecting the debuggee…");
  const output = await evaluateInDebugSession(script, ctx.frameId);
  if (output === null) {
    if (!op.quiet) {
      void vscode.window.showWarningMessage("Basilisk: Could not run the memory script in the debuggee.");
    }
    return null;
  }

  op.report("Analyzing…");
  return ctx.client.sendRequest<MemoryIngestResult | null>("workspace/executeCommand", {
    command: LSP_MEM_CMD.ingest,
    arguments: [{ memorySessionId: ctx.memorySessionId, output }],
  });
}

/**
 * Run one memory operation under a user-facing progress notification (the manual
 * command path: snapshot / diff / gc / references). The pause → evaluate → analyze
 * round-trip takes a beat, so its stages narrate under one notification
 * ([PROFILE-UX-PROGRESS]).
 */
export async function runMemoryScript(
  store: Store,
  command: string,
  extraArgs: Record<string, unknown> = {},
): Promise<MemoryIngestResult | null> {
  return withUserProgress(
    MEM_OP_TITLE[command] ?? "Basilisk: Inspecting memory",
    async (report) => runMemoryOperation({ store, command, extraArgs, report, quiet: false }),
  );
}

// ── Presentation ────────────────────────────────────────────────────────────

/**
 * Land a snapshot result: paint the purple allocation track, append a timeline
 * point, and retain it for a later "Compare". With `openResultsView` (the
 * manual "Take Memory Snapshot" affordance), open the Basilisk memory dashboard
 * — the raw V8 `.heapprofile` stays one click away on its own button
 * ([PROFILE-NATIVE]). Without it (the autopilot's quiet per-pass capture) only
 * the decorations + timeline update — the diff step surfaces the dashboard.
 */
export function presentSnapshot(
  result: MemoryIngestResult,
  options: { openResultsView: boolean },
): void {
  applyMemoryDecorations(result as unknown as MemorySnapshotResult);
  const dashboard = toDashboardSnapshot(result);
  recordTimelinePoint(dashboard);
  dashboard.timeline = [...captureTimeline];
  lastDashboardSnapshot = dashboard;
  Logger.info(`Memory snapshot: ${dashboard.currentMemory} bytes current`);
  if (options.openResultsView) {
    openMemoryDashboard(dashboard);
  }
}

/**
 * Land a diff result: paint the leak decorations (confidence-coloured) and refresh
 * the dashboard with the leak analysis alongside the last snapshot. Returns the
 * typed diff so callers (the autopilot) can read suspected-leak confidence.
 */
export function presentDiff(result: MemoryIngestResult): MemoryDiffResult {
  const diff = result as unknown as MemoryDiffResult;
  applyLeakDecorations(diff);
  if (lastDashboardSnapshot !== undefined) {
    openMemoryDashboard(lastDashboardSnapshot, toDashboardDiff(result));
  }
  const leaks = Array.isArray(diff.suspectedLeaks) ? diff.suspectedLeaks : [];
  Logger.info(`Memory diff: ${leaks.length} suspected leak(s)`);
  return diff;
}

/** The result of one combined autopilot capture. */
export interface CaptureResult {
  readonly snapshot: MemoryIngestResult | null;
  readonly diff: MemoryDiffResult | null;
}

/**
 * The autopilot's combined capture: a snapshot then a diff, presented quietly
 * (decorations + dashboard + timeline, no new `.heapprofile` tab on each pass).
 * Brackets the whole pair as one in-flight op so the transparent pauses it may
 * perform never trigger a second auto-capture ([PROFILE-MEMORY-AUTOPILOT-PAUSE]).
 */
export async function captureSnapshotAndDiff(
  store: Store,
  report: (message: string) => void,
): Promise<CaptureResult> {
  beginMemoryOp();
  try {
    const snapshotResult = await runMemoryOperation({
      store, command: LSP_MEM_CMD.snapshot, extraArgs: {}, report, quiet: true,
    });
    if (snapshotResult?.kind === "snapshot") {
      presentSnapshot(snapshotResult, { openResultsView: false });
    }
    const diffResult = await runMemoryOperation({
      store, command: LSP_MEM_CMD.diff, extraArgs: {}, report, quiet: true,
    });
    const diff = diffResult?.kind === "diff" ? presentDiff(diffResult) : null;
    return { snapshot: snapshotResult, diff };
  } finally {
    endMemoryOp();
  }
}

/** Append a timeline point for a snapshot (seconds-epoch x-axis the chart reads). */
function recordTimelinePoint(snapshot: MemoryDashboardSnapshot): void {
  captureTimeline.push({
    timestamp: Date.now() / MS_PER_SECOND,
    currentMemory: snapshot.currentMemory,
    peakMemory: snapshot.peakMemory,
    gcObjects: snapshot.gcObjects,
  });
}

// ── At-exit final snapshot ──────────────────────────────────────────────────

/**
 * Read the debuggee's at-exit snapshot payload, deleting the file once it is read
 * intact ([PROFILE-MEMORY-FINAL]). The debuggee writes atomically (sibling temp +
 * `os.replace`), so a readable marker-bearing file is whole. A short poll covers
 * the terminate-event/flush race. The file is unlinked ONLY once a complete
 * (marker-bearing) payload is read — a missing or marker-less read is never
 * destructive. Returns null when no usable payload arrived by the deadline.
 */
export async function readFinalSnapshot(path: string): Promise<string | null> {
  const deadline = Date.now() + FINAL_SNAPSHOT_WAIT_MS;
  for (;;) {
    const contents = await fs.promises.readFile(path, "utf8").catch(() => null);
    if (contents?.includes("__BASILISK_MEM__") === true) {
      await fs.promises.unlink(path).catch(() => undefined);
      return contents;
    }
    if (Date.now() >= deadline) { return null; }
    await delay(FINAL_SNAPSHOT_POLL_MS);
  }
}

/** Read a `currentMemory` figure off an ingest result (for logging/summaries). */
export function snapshotCurrentMemory(result: MemoryIngestResult): number {
  return asNumber(result.currentMemory);
}
