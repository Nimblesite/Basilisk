// Tests for [PROFILE-MEMORY-AUTOPILOT] + [PROFILE-MEMORY-LEAK-ACTIONS] + [PROFILE-MEMORY-REFGRAPH-PICKER].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-AUTOPILOT
//
// REAL end-to-end coverage of the memory autopilot — the whole point is that the
// TEST never calls snapshot/diff itself. A real basilisk-debug session pauses a
// real leaking Python program; the autopilot's per-pause snapshot+diff round-trip
// (and its interval timer) fire automatically, and the assertions read what the
// user sees: leak confidence escalating LOW→MEDIUM→HIGH, the purple + leak
// decorations painted, and exactly one proactive leak-action offer. No mocks.

import { delay } from "../../timeouts";
import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import {
  SESSION_WAIT_MS,
  POLL_MS,
  setBreakpoints,
  waitForPause,
  resume,
  waitForSessionEnd,
} from "./debug-e2e-helpers";
import {
  pollUntilResult,
  setupLspTestSuite,
  teardownLspTestSuite,
  closeAllEditors,
  sameFile,
} from "./test-helpers";
import { buildProfileLaunchConfig } from "../../process-launch";
import { activeMemorySession } from "../../memory-profiler";
import { recordedAutopilotCaptures, recordedLeakOffers } from "../../memory-autopilot";
import { gatherReferenceTypeCandidates } from "../../memory-ref-picker";
import { appliedMemoryDecorations, clearMemoryDecorations } from "../../memory-decorations";

/** The autopilot fixture (leaks ~1.5 MiB at the same site every loop pass). */
const FIXTURE = path.resolve(__dirname, "../../src/test/fixtures/memory_autopilot_loop.py");
/** A run-forever allocator (no breakpoints) — for interval-mode coverage. */
const BUSY_FIXTURE = path.resolve(__dirname, "../../src/test/fixtures/memory_busy.py");
/** 1-based leak/allocation site: `CACHE.append("x" * 5000)`. */
const ALLOC_LINE = 23;
/** 1-based loop breakpoint: `total = leak_round(index)`. */
const BP_LINE = 31;
/** The purple memory-allocation palette and the leak-confidence palette. */
const MEMORY_PALETTE = ["#c084fc", "#a78bfa", "#8b5cf6", "#7c3aed"];
const LEAK_PALETTE = ["#ef4444", "#f87171", "#fb923c", "#a78bfa"];
/** How long to let a (wrongly) enabled auto-capture fire before asserting none did. */
const QUIET_SETTLE_MS = 1500;
/** Max passes to drive before giving up on HIGH (seed + 3 growths needs 4). */
const MAX_PASSES = 7;

/** Profiler config keys this suite toggles; reset to default after each test. */
const CONFIG_KEYS = ["autoSnapshotOnPause", "autoSnapshot", "autoSnapshotInterval"];

async function setProfilerConfig(key: string, value: unknown): Promise<void> {
  await vscode.workspace
    .getConfiguration("basilisk.profiler")
    .update(key, value, vscode.ConfigurationTarget.Global);
}

async function resetProfilerConfig(): Promise<void> {
  for (const key of CONFIG_KEYS) {
    await setProfilerConfig(key, undefined);
  }
}

/** Open the fixture as the visible editor so the autopilot's decorations land. */
async function showFixture(file: string): Promise<void> {
  const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(file));
  await vscode.window.showTextDocument(doc, { preview: false });
}

/** Wait until the autopilot has recorded at least `count` automatic captures. */
async function waitForAutoCaptures(count: number): Promise<void> {
  await pollUntilResult({
    fn: async () => recordedAutopilotCaptures().length,
    predicate: (n) => n >= count,
    timeoutMs: SESSION_WAIT_MS,
    intervalMs: POLL_MS,
  });
}

/** Whether any recorded capture has escalated to HIGH confidence. */
function sawHighConfidence(): boolean {
  return recordedAutopilotCaptures().some((capture) => capture.maxConfidence === "HIGH");
}

/** Launch a plain (non-track-on-launch) basilisk-debug session on `file`. */
async function launchDebug(name: string, file: string): Promise<void> {
  const started = await vscode.debug.startDebugging(undefined, {
    name,
    type: "basilisk-debug",
    request: "launch",
    program: file,
    stopOnEntry: false,
    justMyCode: true,
    console: "internalConsole",
  });
  assert.ok(started, "the debug session must launch");
}

// ── Test bodies (top-level so the suite arrow stays small) ───────────────────

async function moneyFlowAutoEscalatesToHigh(): Promise<void> {
  // The whole pitch: ONE breakpoint in the leaking loop, launch "Run & Track
  // Memory", and just press Continue. The autopilot snapshots+diffs each pause —
  // the test never invokes snapshot/diff.
  await setProfilerConfig("autoSnapshotOnPause", true);
  await showFixture(FIXTURE);
  clearMemoryDecorations();
  setBreakpoints(FIXTURE, [BP_LINE]);

  const started = await vscode.debug.startDebugging(undefined, buildProfileLaunchConfig("memory", FIXTURE));
  assert.ok(started, "the Run & Track Memory launch must start");

  // Tracking auto-starts at the entry pause, the program runs to the first loop
  // breakpoint, and the autopilot takes its first automatic capture there.
  await pollUntilResult({
    fn: async () => activeMemorySession(),
    predicate: (sessionId) => sessionId !== undefined,
    timeoutMs: SESSION_WAIT_MS,
    intervalMs: POLL_MS,
  });
  await waitForPause();
  await waitForAutoCaptures(1);

  // Press Continue and let the autopilot capture each pass until a site escalates
  // to HIGH (seed diff + 3 consecutive growth diffs). The TEST only resumes.
  for (let capture = 2; capture <= MAX_PASSES && !sawHighConfidence(); capture += 1) {
    await resume();
    await waitForAutoCaptures(capture);
  }

  assertEscalatedToHigh();
  assertAutoPaintedDecorations();
  assertSingleLeakOffer();

  await vscode.debug.stopDebugging();
  await waitForSessionEnd();
}

/** The automation climbed the confidence ladder by itself, attributing the leak line. */
function assertEscalatedToHigh(): void {
  assert.ok(
    sawHighConfidence(),
    `the autopilot must escalate the leak to HIGH on Continue alone, got: ${
      recordedAutopilotCaptures().map((c) => c.maxConfidence).join(" → ")}`,
  );
  const high = recordedAutopilotCaptures().find((capture) => capture.maxConfidence === "HIGH");
  assert.ok(
    high?.leakLines.includes(ALLOC_LINE),
    `the HIGH capture must attribute the real leak line ${ALLOC_LINE}, got: ${JSON.stringify(high)}`,
  );
}

/** The purple track AND the HIGH leak badge are painted on the fixture — automatically. */
function assertAutoPaintedDecorations(): void {
  const applied = appliedMemoryDecorations().filter(sameFile(FIXTURE));
  assert.ok(
    applied.some((entry) => entry.line === ALLOC_LINE && MEMORY_PALETTE.includes(entry.color)),
    `the leak line must wear an auto-painted purple track, got: ${JSON.stringify(applied)}`,
  );
  assert.ok(
    applied.some(
      (entry) =>
        entry.line === ALLOC_LINE &&
        LEAK_PALETTE.includes(entry.color) &&
        entry.contentText.includes("HIGH"),
    ),
    `the leak line must wear an auto-painted HIGH leak badge, got: ${JSON.stringify(applied)}`,
  );
}

/** Exactly one proactive leak action is offered ([PROFILE-MEMORY-LEAK-ACTIONS]). */
function assertSingleLeakOffer(): void {
  const offers = recordedLeakOffers();
  assert.strictEqual(offers.length, 1, `exactly one leak action must be offered, got: ${JSON.stringify(offers)}`);
  assert.strictEqual(offers[0]?.line, ALLOC_LINE, "the offer must point at the leak line");
  assert.strictEqual(offers[0]?.confidence, "HIGH", "the offer must carry the HIGH confidence that triggered it");
}

async function offSwitchSuppressesAutoCapture(): Promise<void> {
  await setProfilerConfig("autoSnapshotOnPause", false);
  await showFixture(FIXTURE);
  setBreakpoints(FIXTURE, [BP_LINE]);
  await launchDebug("Autopilot off", FIXTURE);

  // Start tracking by hand at the first pause (resets the autopilot ledger).
  await waitForPause();
  await vscode.commands.executeCommand("basilisk.memoryStart");
  assert.ok(activeMemorySession() !== undefined, "tracking must start");

  // Continue to the next pass and give any (erroneous) auto-capture time to fire.
  await resume();
  await waitForPause();
  await delay(QUIET_SETTLE_MS);

  assert.strictEqual(
    recordedAutopilotCaptures().length,
    0,
    `no auto-capture must happen when autoSnapshotOnPause is off, got: ${JSON.stringify(recordedAutopilotCaptures())}`,
  );

  await vscode.debug.stopDebugging();
  await waitForSessionEnd();
}

async function intervalModeCapturesRunningProgram(): Promise<void> {
  // Wire the (previously dead) interval settings: snapshot every second, no
  // pause-based capture, on a program that never stops on its own.
  await setProfilerConfig("autoSnapshotOnPause", false);
  await setProfilerConfig("autoSnapshot", true);
  await setProfilerConfig("autoSnapshotInterval", 1);
  vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  await showFixture(BUSY_FIXTURE);
  await launchDebug("Autopilot interval", BUSY_FIXTURE);
  await pollUntilResult({
    fn: async () => vscode.debug.activeDebugSession,
    predicate: (session) => session !== undefined,
    timeoutMs: SESSION_WAIT_MS,
    intervalMs: POLL_MS,
  });

  // Start tracking on the running program (auto-pause → inject → resume); the
  // interval timer then captures on its own.
  await vscode.commands.executeCommand("basilisk.memoryStart");
  assert.ok(activeMemorySession() !== undefined, "tracking must start on the run-forever program");

  await waitForAutoCaptures(2);
  const captures = recordedAutopilotCaptures();
  assert.ok(
    captures.every((capture) => capture.trigger === "interval"),
    `every capture must be interval-triggered, got: ${captures.map((c) => c.trigger).join(", ")}`,
  );

  await vscode.debug.stopDebugging();
  await waitForSessionEnd();
}

async function pickerIsPopulatedFromRealSymbols(): Promise<void> {
  // The picker offers the user's OWN classes (via the real documentSymbol
  // provider) plus container builtins — never a blank "type a name" box.
  await showFixture(FIXTURE);
  const uri = vscode.Uri.file(FIXTURE);

  const candidates = await pollUntilResult({
    fn: async () => gatherReferenceTypeCandidates(uri),
    predicate: (types) => types.includes("Widget"),
    timeoutMs: SESSION_WAIT_MS,
    intervalMs: POLL_MS,
  });

  assert.ok(
    candidates.includes("Widget"),
    `the picker must offer the file's own class from real symbols, got: ${candidates.join(", ")}`,
  );
  for (const builtin of ["dict", "list", "set", "tuple"]) {
    assert.ok(candidates.includes(builtin), `the picker must offer the container builtin ${builtin}`);
  }
}

suite("Memory autopilot — real end-to-end", () => {
  let tmpDir = "";

  suiteSetup(async function () {
    this.timeout(60_000);
    const result = await setupLspTestSuite("basilisk-mem-autopilot-");
    tmpDir = result.tmpDir;
    assert.ok(fs.existsSync(FIXTURE), `autopilot fixture must exist: ${FIXTURE}`);
  });

  suiteTeardown(async function () {
    this.timeout(30_000);
    vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
    clearMemoryDecorations();
    await resetProfilerConfig();
    await closeAllEditors();
    teardownLspTestSuite(tmpDir);
  });

  teardown(async function () {
    this.timeout(30_000);
    if (vscode.debug.activeDebugSession !== undefined) {
      await vscode.debug.stopDebugging();
      await waitForSessionEnd();
    }
    await vscode.commands.executeCommand("basilisk.memoryStop");
    await resetProfilerConfig();
    vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  });

  test("the money flow: Run & Track Memory + a loop breakpoint auto-escalates a leak to HIGH on Continue alone", async function () {
    this.timeout(150_000);
    await moneyFlowAutoEscalatesToHigh();
  });

  test("the off switch: with autoSnapshotOnPause disabled, a pause is NOT auto-captured", async function () {
    this.timeout(90_000);
    await offSwitchSuppressesAutoCapture();
  });

  test("interval mode: a running program with no breakpoint is auto-captured on a timer", async function () {
    this.timeout(90_000);
    await intervalModeCapturesRunningProgram();
  });

  test("reference-graph picker is populated from the file's real document symbols (no free-text)", async function () {
    this.timeout(60_000);
    await pickerIsPopulatedFromRealSymbols();
  });
});
