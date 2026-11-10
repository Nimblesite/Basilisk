// Tests for [PROFILE-MEMORY-HOWTO] + [PROFILE-MEMORY-INGEST].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-HOWTO
//
// REAL memory-introspection end-to-end: the two courier round-trips that ship a
// UI command but had no real-debuggee coverage — the reference-graph walk
// (basilisk.memory.references) and gc-collect cycle detection
// (basilisk.memory.gcCollect). A real basilisk-debug session pauses a real
// program; the editor injects the gc-introspection script, evaluates it in the
// paused frame, and posts the genuine `__BASILISK_MEM_REFS__`/`__BASILISK_MEM_GC__`
// output back through basilisk.memory.ingest. No mocks: the assertions are over
// the actual retained object graph and the actually-collected finalizer cycle.

import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import {
  setBreakpoints,
  waitForPause,
  resume,
  waitForSessionEnd,
  memoryCourier,
  memoryRoundTrip,
  type IngestResult,
} from "./debug-e2e-helpers";
import { setupLspTestSuite, teardownLspTestSuite, closeAllEditors } from "./test-helpers";

/** The introspection fixture (retained Widgets + a dropped finalizer cycle). */
const FIXTURE = path.resolve(__dirname, "../../src/test/fixtures/memory_introspect.py");
/** `ready = True` — the registry is built; the cycle has NOT been made yet. */
const BP_TRACK = 52;
/** `done = ready` — the finalizer cycle has been built and dropped. */
const BP_GC = 54;

/** The reference-graph ingest payload. */
interface RefsResult extends IngestResult {
  graph: {
    nodes: { type: string; isTarget?: boolean; repr?: string }[];
    edges: unknown[];
    cycles: unknown[];
  };
}

/** The gc-collect ingest payload. */
interface GcResult extends IngestResult {
  collected: number;
  uncollectable: number;
  uncollectableObjects: { typeName: string; reason: string }[];
}

/** Launch the introspection fixture under the Basilisk debug adapter. */
async function launchIntrospectSession(): Promise<void> {
  const started = await vscode.debug.startDebugging(undefined, {
    name: "Memory introspection E2E",
    type: "basilisk-debug",
    request: "launch",
    program: FIXTURE,
    stopOnEntry: false,
    justMyCode: true,
    console: "internalConsole",
  });
  assert.ok(started, "the debug session must launch");
}

suite("Memory introspection — real end-to-end", () => {
  let tmpDir = "";

  suiteSetup(async function () {
    this.timeout(60_000);
    const result = await setupLspTestSuite("basilisk-mem-introspect-");
    tmpDir = result.tmpDir;
    assert.ok(fs.existsSync(FIXTURE), `introspection fixture must exist: ${FIXTURE}`);
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(FIXTURE));
    await vscode.window.showTextDocument(doc, { preview: false });
  });

  suiteTeardown(async function () {
    this.timeout(30_000);
    vscode.debug.removeBreakpoints(vscode.debug.breakpoints);
    await closeAllEditors();
    teardownLspTestSuite(tmpDir);
  });

  teardown(async () => {
    if (vscode.debug.activeDebugSession !== undefined) {
      await vscode.debug.stopDebugging();
      await waitForSessionEnd();
    }
  });

  test("reference graph: walking a retained custom type yields real target nodes + edges", async function () {
    this.timeout(60_000);
    setBreakpoints(FIXTURE, [BP_TRACK]);
    await launchIntrospectSession();
    const frameId = await waitForPause();

    // Mint a session (start also injects tracemalloc — harmless for a refs walk).
    const start = await memoryRoundTrip("basilisk.memory.start", undefined, frameId);
    assert.strictEqual(start.kind, "ack", "memory.start ingest must acknowledge");
    const session = start.memorySessionId;
    assert.ok(typeof session === "string" && session.length > 0, "a memory session must be minted");

    // Walk the retainers of the module-global REGISTRY's Widgets.
    const refs = await memoryCourier<RefsResult>({
      command: "basilisk.memory.references",
      leg1Args: { targetType: "Widget", maxDepth: 4, maxNodes: 200 },
      frameId,
      ingestSessionId: session,
    });
    assert.strictEqual(refs.kind, "refs", "references ingest must be kind-tagged refs");

    const nodes = refs.graph.nodes;
    assert.ok(
      Array.isArray(nodes) && nodes.length > 0,
      `the walk must return real nodes, got: ${JSON.stringify(refs.graph)}`,
    );
    assert.ok(
      nodes.some((node) => node.type === "Widget" && node.isTarget === true),
      `the retained Widget instances must appear as target nodes, got types: ${
        nodes.map((node) => node.type).join(", ")}`,
    );
    assert.ok(Array.isArray(refs.graph.edges), "the graph must carry an edges array");

    await resume();
    await waitForSessionEnd();
  });

  test("gc collect: a dropped finalizer cycle is really collected and surfaced", async function () {
    this.timeout(60_000);
    setBreakpoints(FIXTURE, [BP_TRACK, BP_GC]);
    await launchIntrospectSession();

    // Pause 1 (registry built, cycle not yet made): start tracking. The start
    // script sets gc DEBUG_SAVEALL, so the cycle a later collect reclaims is
    // retained for inspection instead of vanishing.
    let frameId = await waitForPause();
    const start = await memoryRoundTrip("basilisk.memory.start", undefined, frameId);
    assert.strictEqual(start.kind, "ack", "memory.start ingest must acknowledge");
    const session = start.memorySessionId;
    assert.ok(typeof session === "string" && session.length > 0, "a memory session must be minted");
    await resume();

    // Pause 2 (the finalizer cycle has been built and dropped; the fixture
    // disabled automatic gc, so it is still on the heap): force a collection.
    frameId = await waitForPause();
    const collected = await memoryCourier<GcResult>({
      command: "basilisk.memory.gcCollect",
      leg1Args: {},
      frameId,
      ingestSessionId: session,
    });
    assert.strictEqual(collected.kind, "gc", "gcCollect ingest must be kind-tagged gc");
    assert.ok(
      collected.collected > 0,
      `gc.collect() must report reclaiming the dropped cycle, got collected=${collected.collected}`,
    );
    assert.ok(
      collected.uncollectable > 0,
      `DEBUG_SAVEALL must retain the reclaimed cycle for inspection, got uncollectable=${collected.uncollectable}`,
    );

    await resume();
    await waitForSessionEnd();
  });
});
