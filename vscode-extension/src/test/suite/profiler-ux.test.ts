// Tests for [PROFILE-UX-PROGRESS]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-UX-PROGRESS
//
// Loading & progress states: the processes panel must never claim "no
// processes" while the language server is still connecting, a manual refresh
// must visibly run under the view's progress bar, and progress operations
// must open AND close (no zombie notifications). The heavier flow-level
// progress assertions live inside the CPU/memory e2e suites where the real
// flows run; this suite covers the panel chrome and the manifest gating.

import * as assert from "assert";
import * as vscode from "vscode";
import { recordedOperations } from "../../progress-ops";
import {
  EXTENSION_ID,
  setupLspTestSuite,
  teardownLspTestSuite,
} from "./test-helpers";

/** One viewsWelcome contribution. */
interface WelcomeEntry {
  readonly view: string;
  readonly contents: string;
  readonly when?: string;
}

/** The pythonProcesses panel's welcome entries from the live manifest. */
function pythonProcessesWelcome(): WelcomeEntry[] {
  const extension = vscode.extensions.getExtension(EXTENSION_ID);
  assert.ok(extension, "the Basilisk extension must be present");
  const pkg = extension.packageJSON as {
    contributes?: { viewsWelcome?: WelcomeEntry[] };
  };
  return (pkg.contributes?.viewsWelcome ?? []).filter(
    (entry) => entry.view === "basilisk.pythonProcesses",
  );
}

suite("Profiler UX — loading & progress states", () => {
  let tmpDir = "";

  suiteSetup(async function () {
    this.timeout(60_000);
    const result = await setupLspTestSuite("basilisk-ux-");
    tmpDir = result.tmpDir;
  });

  suiteTeardown(() => {
    teardownLspTestSuite(tmpDir);
  });

  test("panel empty state is server-state aware — never 'no processes' while connecting", () => {
    const entries = pythonProcessesWelcome();
    assert.strictEqual(
      entries.length,
      5,
      `the panel needs connecting/stopped/loading/couldn't-load/empty welcome states (#147), got ${entries.length}`,
    );

    const connecting = entries.find((entry) => entry.contents.includes("Connecting"));
    assert.ok(connecting !== undefined, "a 'connecting' state must exist");
    assert.ok(
      connecting.when?.includes("basilisk.serverState != running") === true,
      `the connecting state must hide once the server runs, when: ${String(connecting.when)}`,
    );

    const stopped = entries.find((entry) => entry.contents.includes("not running"));
    assert.ok(stopped !== undefined, "a 'server stopped' state must exist");
    assert.strictEqual(stopped.when, "basilisk.serverState == stopped");
    assert.ok(
      stopped.contents.includes("command:basilisk.restartServer"),
      "the stopped state must offer a one-click restart",
    );

    const running = entries.find((entry) =>
      entry.contents.includes("No Python processes running"),
    );
    assert.ok(running !== undefined, "the true empty state must exist");
    const runningWhen = running.when ?? "";
    assert.ok(
      runningWhen.includes("basilisk.serverState == running") &&
        runningWhen.includes("basilisk.processesState == loaded"),
      `'No Python processes' may only show when the server runs AND a fetch actually succeeded (#147); when: ${runningWhen}`,
    );
    assert.ok(
      running.contents.includes("command:basilisk.profileCurrentFileCpu") &&
        running.contents.includes("command:basilisk.trackMemoryCurrentFile"),
      "the empty state must keep the metric-explicit launch buttons",
    );
  });

  test("manual panel refresh runs under the view's progress bar and completes", async () => {
    const before = recordedOperations().length;
    await vscode.commands.executeCommand("basilisk.refreshProcesses");
    const ops = recordedOperations().slice(before);
    const begin = ops.indexOf("begin:Refresh Python processes");
    const end = ops.indexOf("end:Refresh Python processes");
    assert.ok(begin !== -1, `refresh must show view progress, ops: ${ops.join(" | ")}`);
    assert.ok(end > begin, "the refresh progress must close when the fetch completes");
  });

  test("every recorded progress operation closes — no zombie notifications", () => {
    // Cumulative invariant across everything the suite run has done so far:
    // each begin has a matching end (failed flows close via finally).
    const ops = recordedOperations();
    const begins = ops.filter((entry) => entry.startsWith("begin:")).length;
    const ends = ops.filter((entry) => entry.startsWith("end:")).length;
    assert.strictEqual(
      begins,
      ends,
      `unbalanced progress operations — a notification leaked: ${ops.join(" | ")}`,
    );
  });
});
