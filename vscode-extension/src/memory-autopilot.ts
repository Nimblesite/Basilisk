// Implements [PROFILE-MEMORY-AUTOPILOT] + [PROFILE-MEMORY-LEAK-ACTIONS].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-AUTOPILOT
/**
 * The memory autopilot: captures snapshots automatically while tracking is
 * active, so the interactive leak hunt is "set a breakpoint and press Continue"
 * instead of a per-pause click treadmill.
 *
 * Two triggers, one capture core ([`captureSnapshotAndDiff`](./memory-capture.ts)):
 *   - every debugger pause   — [PROFILE-MEMORY-AUTOPILOT-PAUSE] (on by default)
 *   - a fixed interval       — [PROFILE-MEMORY-AUTOPILOT-INTERVAL] (opt-in)
 *
 * It also surfaces one proactive leak action the first time a site escalates to
 * High/Definite ([PROFILE-MEMORY-LEAK-ACTIONS]).
 *
 * The interval timer's lifecycle follows the store's tracking signal — no timer
 * outlives its session. Guards (a synchronous re-entrancy flag plus the shared
 * `isMemoryOperationInFlight` flag) ensure a capture's own transparent pause, an
 * in-progress manual op, or two near-simultaneous `stopped` events never trigger
 * a duplicate capture.
 */

import * as vscode from "vscode";
import * as path from "path";
import { effect } from "@preact/signals-core";
import { Logger } from "./logger";
import type { Store } from "./store";
import type { ProfilerActivity } from "./profiler-state";
import { withUserProgress } from "./progress-ops";
import { captureSnapshotAndDiff, isMemoryOperationInFlight } from "./memory-capture";
import {
  confidenceRank,
  type LeakConfidence,
  type MemoryDiffResult,
  type SuspectedLeak,
} from "./memory-decorations";

/** Progress title for an automatic capture (visible feedback each pass). */
const AUTO_CAPTURE_TITLE = "Basilisk: Auto-capturing memory";

/** Confidence at/above which the proactive leak action is offered. */
const ACTION_THRESHOLD: LeakConfidence = "HIGH";

/** Minimum interval (seconds) so a misconfigured tiny value can't busy-loop. */
const MIN_INTERVAL_SECS = 1;
const DEFAULT_INTERVAL_SECS = 30;
/** Milliseconds per second (interval setting is in seconds). */
const MS_PER_SECOND = 1000;

// ── E2e ledgers ─────────────────────────────────────────────────────────────
// Same observability pattern as recordedOperations()/appliedMemoryDecorations():
// the autopilot fires real captures, and these record what it did so tests can
// assert the automation without driving snapshot/diff themselves.

/** One recorded automatic capture. */
export interface AutopilotCapture {
  /** What triggered it. */
  readonly trigger: "pause" | "interval";
  /** Suspected-leak count from the diff. */
  readonly suspectedLeakCount: number;
  /** Highest leak confidence in the diff, or "none". */
  readonly maxConfidence: LeakConfidence | "none";
  /** 1-based line numbers flagged as suspected leaks. */
  readonly leakLines: readonly number[];
}

/** One recorded proactive leak-action offer. */
export interface LeakActionOffer {
  readonly file: string;
  readonly line: number;
  readonly confidence: LeakConfidence;
}

let autopilotCaptures: AutopilotCapture[] = [];
let leakActionOffers: LeakActionOffer[] = [];

/** The automatic captures performed this session (e2e seam). */
export function recordedAutopilotCaptures(): readonly AutopilotCapture[] {
  return autopilotCaptures;
}

/** The proactive leak-action offers made this session (e2e seam). */
export function recordedLeakOffers(): readonly LeakActionOffer[] {
  return leakActionOffers;
}

// ── State ─────────────────────────────────────────────────────────────────

let boundStore: Store | undefined;
/** Synchronous re-entrancy guard — set before the first `await` of a capture. */
let autopilotBusy = false;
/** Interval timer handle while interval mode is armed. */
let intervalTimer: ReturnType<typeof setInterval> | undefined;
/** The memory session we have already offered a leak action for (offer once). */
let leakOfferedForSession: string | undefined;
/** Previous tracking state, to detect active⇄idle transitions in the effect. */
let prevMemoryState: ProfilerActivity = "idle";

// ── Registration ────────────────────────────────────────────────────────────

/**
 * Wire the autopilot to the store. The returned disposable tears down the
 * tracking-signal effect and any live interval timer.
 */
export function registerMemoryAutopilot(store: Store): vscode.Disposable[] {
  boundStore = store;
  prevMemoryState = store.profiler.value.memory;
  const disposeEffect = effect(() => { autopilotLifecycle(store); });
  return [{ dispose: () => { disposeEffect(); disposeMemoryAutopilot(); } }];
}

/** Clear all autopilot state (deactivation / test teardown). */
export function disposeMemoryAutopilot(): void {
  stopInterval();
  boundStore = undefined;
  autopilotBusy = false;
  leakOfferedForSession = undefined;
  prevMemoryState = "idle";
  autopilotCaptures = [];
  leakActionOffers = [];
}

// ── Pause trigger ────────────────────────────────────────────────────────────

/**
 * Called by the DAP tracker on every `stopped` event ([PROFILE-MEMORY-AUTOPILOT-PAUSE]).
 * Captures automatically when tracking is active for *this* session, the pause
 * is a genuine user pause (no memory op already in flight), and pause-capture is
 * enabled. Fire-and-forget — never blocks the DAP tracker.
 */
export function notifyDebuggeePause(sessionId: string): void {
  const store = boundStore;
  if (store === undefined) { return; }
  if (autopilotBusy || isMemoryOperationInFlight()) { return; }
  if (store.profiler.value.memory !== "active") { return; }
  if (store.profiler.value.memoryDebugSessionId !== sessionId) { return; }
  if (!isPauseCaptureEnabled()) { return; }
  // Set the synchronous guard NOW so a second `stopped` event in the same tick
  // (per-thread + allThreadsStopped) cannot start a duplicate capture.
  autopilotBusy = true;
  Logger.info(`[Memory] autopilot: capturing on pause (session ${sessionId})`);
  void runAutoCapture(store, "pause");
}

// ── Interval trigger ─────────────────────────────────────────────────────────

/** Start/stop the interval timer as tracking turns on and off. */
function autopilotLifecycle(store: Store): void {
  const state = store.profiler.value.memory;
  if (state === "active" && prevMemoryState !== "active") {
    // A fresh tracking session: reset per-session state, arm the interval timer.
    resetSessionState();
    armIntervalIfEnabled(store);
  } else if (state !== "active" && prevMemoryState === "active") {
    stopInterval();
  }
  prevMemoryState = state;
}

/** Arm the interval timer if interval mode is enabled in settings. */
function armIntervalIfEnabled(store: Store): void {
  const config = vscode.workspace.getConfiguration("basilisk.profiler");
  if (!config.get<boolean>("autoSnapshot", false)) { return; }
  const seconds = Math.max(MIN_INTERVAL_SECS, config.get<number>("autoSnapshotInterval", DEFAULT_INTERVAL_SECS));
  stopInterval();
  intervalTimer = setInterval(() => { void onIntervalTick(store); }, seconds * MS_PER_SECOND);
  Logger.info(`[Memory] autopilot interval armed: every ${seconds}s`);
}

/** Stop the interval timer if running. */
function stopInterval(): void {
  if (intervalTimer !== undefined) {
    clearInterval(intervalTimer);
    intervalTimer = undefined;
  }
}

/** One interval tick: capture if idle and still tracking. */
async function onIntervalTick(store: Store): Promise<void> {
  if (autopilotBusy || isMemoryOperationInFlight()) { return; }
  if (store.profiler.value.memory !== "active") { return; }
  autopilotBusy = true;
  Logger.info("[Memory] autopilot: capturing on interval");
  await runAutoCapture(store, "interval");
}

// ── Capture ──────────────────────────────────────────────────────────────────

/**
 * Run one automatic capture under a progress notification, record it, and offer
 * a leak action if a site just escalated. Assumes `autopilotBusy` is already set
 * by the caller (synchronously, to close the two-events race); always clears it.
 */
async function runAutoCapture(store: Store, trigger: "pause" | "interval"): Promise<void> {
  try {
    const result = await withUserProgress(AUTO_CAPTURE_TITLE, async (report) =>
      captureSnapshotAndDiff(store, report),
    );
    recordCapture(trigger, result.diff);
    maybeOfferLeakActions(store, result.diff);
  } catch (err: unknown) {
    Logger.warn(`[Memory] autopilot capture failed: ${err instanceof Error ? err.message : String(err)}`);
  } finally {
    autopilotBusy = false;
  }
}

/** Record a capture's diff into the ledger (the e2e seam). */
function recordCapture(trigger: "pause" | "interval", diff: MemoryDiffResult | null): void {
  const leaks = diff?.suspectedLeaks ?? [];
  const worst = worstLeak(leaks);
  autopilotCaptures.push({
    trigger,
    suspectedLeakCount: leaks.length,
    maxConfidence: worst?.confidence ?? "none",
    leakLines: leaks.map((leak) => leak.line),
  });
  Logger.info(
    `[Memory] autopilot capture (${trigger}): ${leaks.length} suspected leak(s), ` +
    `max confidence ${worst?.confidence ?? "none"}`,
  );
}

/** The highest-confidence leak in a list, or undefined when there are none. */
function worstLeak(leaks: readonly SuspectedLeak[]): SuspectedLeak | undefined {
  return leaks.reduce<SuspectedLeak | undefined>((worst, leak) => {
    if (worst === undefined || confidenceRank(leak.confidence) > confidenceRank(worst.confidence)) {
      return leak;
    }
    return worst;
  }, undefined);
}

/**
 * Offer the proactive leak action the first time a site reaches the threshold
 * confidence this session ([PROFILE-MEMORY-LEAK-ACTIONS]) — at most once, so the
 * Continue loop is never spammed.
 */
function maybeOfferLeakActions(store: Store, diff: MemoryDiffResult | null): void {
  const worst = worstLeak(diff?.suspectedLeaks ?? []);
  if (worst === undefined || confidenceRank(worst.confidence) < confidenceRank(ACTION_THRESHOLD)) {
    return;
  }
  const sessionId = store.profiler.value.memorySessionId;
  if (sessionId === undefined || leakOfferedForSession === sessionId) { return; }
  leakOfferedForSession = sessionId;
  leakActionOffers.push({ file: worst.file, line: worst.line, confidence: worst.confidence });
  Logger.warn(`[Memory] autopilot: suspected leak ${path.basename(worst.file)}:${worst.line} (${worst.confidence})`);
  void offerLeakActions(worst);
}

/** Show the one-click leak-action notification (Reference Graph / Force GC). */
async function offerLeakActions(leak: SuspectedLeak): Promise<void> {
  const showGraph = "Show Reference Graph";
  const forceGc = "Force Garbage Collection";
  const choice = await vscode.window.showWarningMessage(
    `Basilisk: Suspected memory leak at ${path.basename(leak.file)}:${leak.line} (${leak.confidence}). ${leak.reason}`,
    showGraph,
    forceGc,
  );
  if (choice === showGraph) {
    await vscode.commands.executeCommand("basilisk.memoryReferences");
  } else if (choice === forceGc) {
    await vscode.commands.executeCommand("basilisk.memoryGcCollect");
  }
}

/** Reset per-session state at the start of a new tracking session. */
function resetSessionState(): void {
  autopilotCaptures = [];
  leakActionOffers = [];
  leakOfferedForSession = undefined;
}

/** Whether per-pause auto-capture is enabled (default true). */
function isPauseCaptureEnabled(): boolean {
  return vscode.workspace
    .getConfiguration("basilisk.profiler")
    .get<boolean>("autoSnapshotOnPause", true);
}
