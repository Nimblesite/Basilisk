// Tests for [PROFILE-MEMORY-HOWTO] + [PROFILE-MEMORY-INGEST] + [PROFILE-PROCESSES-LAUNCH-FILE].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-HOWTO
//
// REAL memory-profiling end-to-end: a real debugpy session pauses a real
// Python program, the editor-as-courier round-trip injects tracemalloc and
// posts the output back via `basilisk.memory.ingest`, and the assertions
// cover what the user actually sees — snapshot allocations attributed to the
// real allocation line, leak suspicion on growth, the purple heat-map track
// (via the applied-decoration ledger), the `.heapprofile` for VS Code's
// built-in viewer, and the "Run & Track Memory (Current File)" auto-start.

import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { currentStoppedFrameId, evaluateInDebugSession } from "../../dap-evaluate";
import { activeMemorySession } from "../../memory-profiler";
import { recordedOperations } from "../../progress-ops";
import { buildProfileLaunchConfig } from "../../process-launch";
import {
  applyLeakDecorations,
  applyMemoryDecorations,
  appliedMemoryDecorations,
  clearMemoryDecorations,
  type MemoryDiffResult,
  type MemorySnapshotResult,
} from "../../memory-decorations";
import {
  openPythonFile,
  pollUntilResult,
  setupLspTestSuite,
  teardownLspTestSuite,
  closeAllEditors,
} from "./test-helpers";

/** The memory fixture, opened from the real fixtures directory. */
const FIXTURE = path.resolve(__dirname, "../../src/test/fixtures/memory_growth.py");
/** 1-based allocation site: `CACHE.append("x" * 5000)`. */
const ALLOC_LINE = 9;
/** Breakpoints between allocation chunks in main(). */
const BP_AFTER_CHUNK1 = 15;
const BP_AFTER_CHUNK2 = 16;
const BP_AFTER_CHUNK3 = 17;
/** The memory decoration palette (purple track + leak colors). */
const MEMORY_PALETTE = ["#c084fc", "#a78bfa", "#8b5cf6", "#7c3aed"];
const LEAK_PALETTE = ["#ef4444", "#f87171", "#fb923c", "#a78bfa"];

/** Budget for a debug session to start / stop / pause. */
const SESSION_WAIT_MS = 20_000;
/** Poll cadence for debug-state changes. */
const POLL_MS = 100;

// ── Slim debug helpers (local to this suite) ─────────────────────────────

function setBreakpoints(filePath: string, lines: number[]): void {
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
async function waitForPause(): Promise<number> {
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
async function resume(): Promise<void> {
  const session = vscode.debug.activeDebugSession;
  assert.ok(session, "an active debug session is required to resume");
  const threads = (await session.customRequest("threads")) as { threads?: { id: number }[] };
  const threadId = threads.threads?.[0]?.id;
  assert.ok(threadId !== undefined, "the debuggee must report a thread");
  await session.customRequest("continue", { threadId });
}

/** Wait for the active debug session to terminate. */
async function waitForSessionEnd(): Promise<void> {
  await pollUntilResult({
    fn: async () => vscode.debug.activeDebugSession,
    predicate: (session) => session === undefined,
    timeoutMs: SESSION_WAIT_MS,
    intervalMs: POLL_MS,
  });
}

// ── Courier round-trip (leg 1 → evaluate → leg 2) ────────────────────────

/** One marker-tagged ingest result. */
interface IngestResult {
  kind: string;
  [field: string]: unknown;
}

/** Run one memory command's full courier round-trip against the paused debuggee. */
async function memoryRoundTrip<T extends IngestResult>(
  command: string,
  memorySessionId: string | undefined,
  frameId: number,
): Promise<T> {
  const leg1 = await vscode.commands.executeCommand<
    { memorySessionId?: string; script?: string } | null
  >(command, {
    ...(memorySessionId === undefined ? { tracebackDepth: 25 } : { memorySessionId }),
  });
  const script = leg1?.script;
  assert.ok(script !== undefined && script !== "", `${command} must return an injection script`);

  const output = await evaluateInDebugSession(script, frameId);
  assert.ok(output !== null, `${command} script must evaluate in the paused debuggee`);

  const ingested = await vscode.commands.executeCommand<T | null>("basilisk.memory.ingest", {
    memorySessionId: memorySessionId ?? leg1?.memorySessionId,
    output,
  });
  assert.ok(ingested !== null, "ingest must return a kind-tagged result");
  return ingested;
}

/** Assert the snapshot's user-facing surface: heapprofile artifact + purple track. */
function assertSnapshotSurface(snapshot: MemorySnapshotResult & IngestResult): void {
  assert.ok(snapshot.currentMemory > 0, "tracemalloc must report live memory");
  assert.ok(
    snapshot.topAllocations.some((a) => a.file.endsWith("memory_growth.py") && a.line === ALLOC_LINE),
    `the real allocation site (line ${ALLOC_LINE}) must be attributed, got: ${ 
      snapshot.topAllocations.map((a) => `${path.basename(a.file)}:${a.line}`).join(", ")}`,
  );

  // Native .heapprofile artifact for the built-in viewer ([PROFILE-NATIVE]).
  const heapProfilePath = snapshot.heapProfilePath;
  assert.ok(typeof heapProfilePath === "string" && heapProfilePath !== "", "heapProfilePath must be returned");
  assert.ok(fs.existsSync(heapProfilePath), ".heapprofile must be written to disk");
  const heapprofile = JSON.parse(fs.readFileSync(heapProfilePath, "utf8")) as {
    head?: { children?: unknown[]; selfSize?: number };
  };
  assert.ok(heapprofile.head !== undefined, ".heapprofile must have a head tree");

  // The purple memory track is really painted ([PROFILE-VIS-HEATMAP]).
  applyMemoryDecorations(snapshot);
  const memApplied = appliedMemoryDecorations().filter((entry) => entry.file === FIXTURE);
  assert.ok(memApplied.length > 0, "snapshot allocations must paint the open fixture");
  assert.ok(
    memApplied.some((entry) => entry.line === ALLOC_LINE && MEMORY_PALETTE.includes(entry.color)),
    `the allocation line must wear a purple-palette decoration, got: ${JSON.stringify(memApplied)}`,
  );
  assert.ok(
    memApplied.some((entry) => /(B|KB|MB|GB) allocated/.test(entry.contentText)),
    "memory decorations must show allocation sizes",
  );
}

/**
 * Launch the run-forever fixture (no breakpoints) and probe
 * `currentStoppedFrameId` `probes` times while it runs: every probe must
 * decline, with the session still alive to prove the timing.
 */
async function probeRunningDebuggeeForFrames(probes: number): Promise<void> {
  vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  const runningFixture = path.resolve(__dirname, "../../src/test/fixtures/busy_wait.py");
  const started = await vscode.debug.startDebugging(undefined, {
    name: "Memory E2E running probe",
    type: "basilisk-debug",
    request: "launch",
    program: runningFixture,
    stopOnEntry: false,
    justMyCode: true,
    console: "internalConsole",
  });
  assert.ok(started, "the debug session must launch");
  await pollUntilResult({
    fn: async () => vscode.debug.activeDebugSession,
    predicate: (session) => session !== undefined,
    timeoutMs: SESSION_WAIT_MS,
    intervalMs: POLL_MS,
  });

  for (let probe = 0; probe < probes; probe += 1) {
    const frameId = await currentStoppedFrameId();
    assert.ok(
      vscode.debug.activeDebugSession !== undefined,
      "the program must still be running while probing",
    );
    assert.strictEqual(
      frameId,
      null,
      "a running debuggee must never yield a frame id (debugpy samples stackTrace for running threads)",
    );
    await new Promise<void>((resolve) => setTimeout(resolve, 200));
  }

  await vscode.debug.stopDebugging();
  await waitForSessionEnd();
}

/**
 * Launch the run-forever allocator WITHOUT breakpoints, start tracking and
 * snapshot via the real command handlers while it runs, and assert the
 * auto-pause/auto-resume surface: a session is minted, the live allocation
 * line is attributed, and the program is still running afterwards.
 */
async function trackAndSnapshotRunningProgram(): Promise<void> {
  vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
  const busyFixture = path.resolve(__dirname, "../../src/test/fixtures/memory_busy.py");
  const busyAllocLine = 12; // CACHE.append("x" * 5000)
  const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(busyFixture));
  await vscode.window.showTextDocument(doc, { preview: false });

  const started = await vscode.debug.startDebugging(undefined, {
    name: "Memory E2E auto-pause",
    type: "basilisk-debug",
    request: "launch",
    program: busyFixture,
    stopOnEntry: false,
    justMyCode: true,
    console: "internalConsole",
  });
  assert.ok(started, "the debug session must launch");
  await pollUntilResult({
    fn: async () => vscode.debug.activeDebugSession,
    predicate: (session) => session !== undefined,
    timeoutMs: SESSION_WAIT_MS,
    intervalMs: POLL_MS,
  });

  // Start tracking while RUNNING — must auto-pause, inject, auto-resume.
  await vscode.commands.executeCommand("basilisk.memoryStart");
  assert.ok(
    activeMemorySession() !== undefined,
    "Start Memory Tracking on a running program must mint a session (auto-pause)",
  );

  // Let the program allocate under tracemalloc, then snapshot while RUNNING.
  await new Promise<void>((resolve) => setTimeout(resolve, 1000));
  clearMemoryDecorations();
  const opsBefore = recordedOperations().length;
  await vscode.commands.executeCommand("basilisk.memorySnapshot");
  const applied = appliedMemoryDecorations().filter((entry) => entry.file === busyFixture);
  assert.ok(
    applied.some((entry) => entry.line === busyAllocLine),
    `the snapshot must attribute the live allocation line ${busyAllocLine}, got: ${JSON.stringify(applied)}`,
  );

  // [PROFILE-UX-PROGRESS] The snapshot must run under a progress notification
  // that narrates its stages and closes on completion — never a silent wait.
  const snapshotOps = recordedOperations().slice(opsBefore);
  const beginIdx = snapshotOps.indexOf("begin:Basilisk: Taking memory snapshot");
  const endIdx = snapshotOps.indexOf("end:Basilisk: Taking memory snapshot");
  assert.ok(beginIdx !== -1, `snapshot must show progress, ops: ${snapshotOps.join(" | ")}`);
  assert.ok(endIdx > beginIdx, "the progress notification must close when the snapshot completes");
  assert.ok(
    snapshotOps.includes("step:Basilisk: Taking memory snapshot:Pausing the program…"),
    `stage messages must narrate the auto-pause, ops: ${snapshotOps.join(" | ")}`,
  );

  // The program must have been resumed — still alive after the snapshot.
  assert.ok(
    vscode.debug.activeDebugSession !== undefined,
    "the program must keep running after an auto-paused snapshot",
  );
  await vscode.debug.stopDebugging();
  await waitForSessionEnd();
}

/** Assert the diff's user-facing surface: leak suspicion + leak decorations. */
function assertLeakSurface(diff: MemoryDiffResult & IngestResult): void {
  assert.ok(diff.totalGrowth > 0, "allocating a chunk between pauses must register growth");
  assert.ok(
    diff.suspectedLeaks.some((leak) => leak.file.endsWith("memory_growth.py") && leak.line === ALLOC_LINE),
    `growth at the allocation site must be a suspected leak, got: ${JSON.stringify(diff.suspectedLeaks)}`,
  );

  applyLeakDecorations(diff);
  const leakApplied = appliedMemoryDecorations().filter(
    (entry) => entry.file === FIXTURE && LEAK_PALETTE.includes(entry.color) && entry.contentText.includes("leak"),
  );
  assert.ok(leakApplied.length > 0, "suspected leaks must paint leak decorations with a confidence badge");
}

 
suite("Memory profiling — real end-to-end", () => {
  let tmpDir = "";

  suiteSetup(async function () {
    this.timeout(60_000);
    const result = await setupLspTestSuite("basilisk-mem-e2e-");
    tmpDir = result.tmpDir;
    assert.ok(fs.existsSync(FIXTURE), `memory fixture must exist: ${FIXTURE}`);
  });

  suiteTeardown(async function () {
    this.timeout(30_000);
    vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
    clearMemoryDecorations();
    await closeAllEditors();
    teardownLspTestSuite(tmpDir);
  });

  teardown(async () => {
    if (vscode.debug.activeDebugSession !== undefined) {
      await vscode.debug.stopDebugging();
      await waitForSessionEnd();
    }
    await vscode.commands.executeCommand("basilisk.memoryStop");
  });

  test("courier round-trip: inject tracemalloc, snapshot real allocations, diff growth into leaks, paint the purple track", async function () {
    this.timeout(90_000);
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(FIXTURE));
    await vscode.window.showTextDocument(doc, { preview: false });

    setBreakpoints(FIXTURE, [BP_AFTER_CHUNK1, BP_AFTER_CHUNK2, BP_AFTER_CHUNK3]);
    const started = await vscode.debug.startDebugging(undefined, {
      name: "Memory E2E",
      type: "basilisk-debug",
      request: "launch",
      program: FIXTURE,
      stopOnEntry: false,
      justMyCode: true,
      console: "internalConsole",
    });
    assert.ok(started, "the debug session must launch");

    // Pause 1 (chunk 1 allocated): start tracking + seed the diff baseline.
    let frameId = await waitForPause();
    const start = await memoryRoundTrip("basilisk.memory.start", undefined, frameId);
    assert.strictEqual(start.kind, "ack", "memory.start ingest must acknowledge");
    const memorySessionId = start.memorySessionId;
    assert.ok(typeof memorySessionId === "string" && memorySessionId.length > 0, "a memory session must be minted");
    const seeded = await memoryRoundTrip("basilisk.memory.diff", memorySessionId, frameId);
    assert.strictEqual(seeded.kind, "diff", "the first diff must self-seed its baseline");
    await resume();

    // Pause 2 (chunk 2 allocated under tracemalloc): snapshot + diff.
    frameId = await waitForPause();
    const snapshot = await memoryRoundTrip<MemorySnapshotResult & IngestResult>(
      "basilisk.memory.snapshot",
      memorySessionId,
      frameId,
    );
    assert.strictEqual(snapshot.kind, "snapshot", "snapshot ingest must be kind-tagged");
    assertSnapshotSurface(snapshot);

    const diff = await memoryRoundTrip<MemoryDiffResult & IngestResult>(
      "basilisk.memory.diff",
      memorySessionId,
      frameId,
    );
    assert.strictEqual(diff.kind, "diff", "diff ingest must be kind-tagged");
    assertLeakSurface(diff);
    await resume();

    // Pause 3 (chunk 3): consecutive growth keeps the site suspected.
    frameId = await waitForPause();
    const diff2 = await memoryRoundTrip<MemoryDiffResult & IngestResult>(
      "basilisk.memory.diff",
      memorySessionId,
      frameId,
    );
    assert.ok(diff2.totalGrowth > 0, "the third chunk must register growth too");
    assert.ok(diff2.suspectedLeaks.length > 0, "consecutive growth must keep the leak suspected");

    await resume();
    await waitForSessionEnd();
  });

  test("a running (not paused) debuggee yields no evaluable frame — snapshots route to 'pause first'", async function () {
    this.timeout(60_000);
    // debugpy answers `stackTrace` for a RUNNING thread with a sampled frame
    // whose id is NOT evaluable — `evaluate` then fails with "Unable to find
    // thread for evaluation" and the user sees a misleading generic error.
    // currentStoppedFrameId must therefore refuse to mint a frame id unless a
    // `stopped` event marked the thread paused; callers that can pause use
    // acquireStoppedFrame's transparent pause/resume instead.
    await probeRunningDebuggeeForFrames(5);
  });

  test("memory ops on a RUNNING program: auto-pause, snapshot real allocations, auto-resume", async function () {
    this.timeout(60_000);
    // IDE-grade behavior: clicking Start/Snapshot while the program runs must
    // transparently pause → evaluate → resume, never demand a manual
    // breakpoint. This drives the REAL command handlers, not the raw courier.
    await trackAndSnapshotRunningProgram();
  });

  test("Run & Track Memory (Current File): stop-on-entry inject, auto-resume, program completes (#82)", async function () {
    this.timeout(60_000);
    vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
    await openPythonFile(tmpDir, "noop.py", "x = 1\n");

    const started = await vscode.debug.startDebugging(
      undefined,
      buildProfileLaunchConfig("memory", FIXTURE),
    );
    assert.ok(started, "the metric-explicit memory launch must start");

    // The auto-flow must mint a memory session at the entry pause…
    await pollUntilResult({
      fn: async () => activeMemorySession(),
      predicate: (sessionId) => sessionId !== undefined,
      timeoutMs: SESSION_WAIT_MS,
      intervalMs: POLL_MS,
    });

    // …and resume the debuggee so the program actually runs to completion.
    await waitForSessionEnd();
    assert.ok(activeMemorySession() !== undefined, "tracking must survive until explicitly stopped");
  });
});
