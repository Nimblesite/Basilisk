// Tests for [PROFILE-VIS-HEATMAP] + [PROFILE-NATIVE] + [PROFILE-NOTIFICATIONS-PROGRESS].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-VIS-HEATMAP
//
// REAL CPU-profiling end-to-end: spawn an actual CPU-bound Python process,
// attach through the real LSP, and assert the artifacts the user actually
// sees — the inline heat map (via the applied-decoration ledger), the live
// status-bar progress, the hot-function attribution, the `.cpuprofile` for
// VS Code's built-in viewer, the speedscope JSON, and the flamegraph webview
// HTML. Attach assertions are Linux-gated (CI runs ubuntu; macOS requires
// root for py-spy), and every platform asserts the actionable #81 error path.

import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { execFileSync, spawn, type ChildProcess } from "child_process";
import { getStore } from "../../extension";
import { profilerStatusText, startProfilingForPid } from "../../profiler";
import {
  applyProfileDecorations,
  clearProfileDecorations,
  appliedProfileDecorations,
  type ProfileResult,
} from "../../profiler-decorations";
import { buildFlamegraphHtml } from "../../profiler-flamegraph-html";
import {
  openPythonFile,
  pollUntilResult,
  setupLspTestSuite,
  teardownLspTestSuite,
  closeAllEditors,
} from "./test-helpers";

/** How long the burner keeps spinning (covers the whole suite). */
const BURNER_LIFETIME_SECS = 120;
/** Sampling window before stopping a profile. */
const SAMPLE_WINDOW_MS = 2_500;
/** Budget for the LSP attach + first progress notification. */
const PROGRESS_WAIT_MS = 10_000;
/** Budget for profiler diagnostics to be published after stop. */
const DIAGNOSTICS_WAIT_MS = 10_000;
/** The CPU heat-map palette ([PROFILE-VIS-PALETTE]). */
const HEAT_PALETTE = ["#e8500a", "#f97316", "#fbbf24", "#4a5468"];

/** 1-based line of `def hot_function` in the burner source below. */
const HOT_FUNCTION_DEF_LINE = 12;

/**
 * CPU burner: ~all samples land in hot_function. `PR_SET_PTRACER_ANY` lets a
 * non-ancestor LSP attach under Linux Yama ptrace_scope=1 (same trick as the
 * Rust e2e suites).
 */
const BURNER_SOURCE = `import sys
import time

try:
    import ctypes
    _libc = ctypes.CDLL("libc.so.6", use_errno=True)
    _libc.prctl(0x59616D61, ctypes.c_ulong(0xFFFFFFFFFFFFFFFF), 0, 0, 0)
except Exception:
    pass


def hot_function():
    total = 0
    for i in range(1_000_000):
        total += i * i
    return total


def main():
    print("READY", flush=True)
    deadline = time.time() + ${BURNER_LIFETIME_SECS}
    while time.time() < deadline:
        hot_function()


if __name__ == "__main__":
    main()
`;

/** The python interpreter for spawning helper processes. */
const PYTHON = process.platform === "win32" ? "python" : "python3";

/** Spawn the burner and resolve once it prints READY. */
async function spawnBurner(scriptPath: string): Promise<ChildProcess> {
  const child = spawn(PYTHON, [scriptPath], { stdio: ["ignore", "pipe", "ignore"] });
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("burner never printed READY")), PROGRESS_WAIT_MS);
    child.stdout?.on("data", (chunk: Buffer) => {
      if (chunk.toString().includes("READY")) {
        clearTimeout(timer);
        resolve();
      }
    });
    child.on("exit", () => reject(new Error("burner exited before READY")));
  });
  return child;
}

/** The shape `basilisk.profiler.start` resolves to. */
interface StartResult {
  sessionId: string;
  pid: number;
  pythonVersion: string;
}

/** Stop any session left behind so suites stay independent. */
async function stopAllProfilerSessions(): Promise<void> {
  const list = await vscode.commands.executeCommand<{ sessions?: { sessionId: string }[] }>(
    "basilisk.profiler.list",
  );
  for (const session of list?.sessions ?? []) {
    try {
      await vscode.commands.executeCommand("basilisk.profiler.stop", { sessionId: session.sessionId });
    } catch {
      // already gone
    }
  }
}

/** Assert the speedscope JSON artifact exists and attributes hot_function. */
function assertSpeedscopeArtifact(outputFile: string): void {
  assert.ok(fs.existsSync(outputFile), `speedscope file must exist: ${outputFile}`);
  const speedscope = JSON.parse(fs.readFileSync(outputFile, "utf8")) as {
    shared?: { frames?: { name: string }[] };
    profiles?: unknown[];
  };
  assert.ok(
    speedscope.shared?.frames?.some((frame) => frame.name === "hot_function"),
    "speedscope frames must include hot_function",
  );
  assert.ok((speedscope.profiles?.length ?? 0) > 0, "speedscope must contain at least one profile");
}

/** Assert the V8 `.cpuprofile` exists and opens as a valid call tree ([PROFILE-NATIVE]). */
function assertCpuProfileArtifact(cpuProfilePath: string | undefined): void {
  assert.ok(typeof cpuProfilePath === "string" && cpuProfilePath !== "", "cpuProfilePath returned");
  assert.ok(fs.existsSync(cpuProfilePath), ".cpuprofile must be written to disk");
  const cpuprofile = JSON.parse(fs.readFileSync(cpuProfilePath, "utf8")) as {
    nodes?: { callFrame?: { functionName?: string } }[];
    samples?: number[];
    timeDeltas?: number[];
  };
  assert.ok((cpuprofile.nodes?.length ?? 0) > 0, ".cpuprofile must have a call tree");
  assert.ok((cpuprofile.samples?.length ?? 0) > 0, ".cpuprofile must have samples");
  assert.strictEqual(
    cpuprofile.samples?.length,
    cpuprofile.timeDeltas?.length,
    ".cpuprofile samples and timeDeltas must be parallel arrays",
  );
  assert.ok(
    cpuprofile.nodes?.some((node) => node.callFrame?.functionName === "hot_function"),
    ".cpuprofile call tree must include hot_function",
  );
}

/** Assert the hottest line wears the correctly-tiered palette color. */
function assertHottestLineTier(result: ProfileResult, burnerPath: string): void {
  applyProfileDecorations(result);
  const applied = appliedProfileDecorations().filter((entry) => entry.file === burnerPath);
  assert.ok(applied.length > 0, "real profile data must paint the open hot file");
  const topLine = [...result.hotLines].sort((a, b) => b.percentage - a.percentage)[0];
  const expectedColor =
    topLine.percentage >= 20 ? "#e8500a"
    : topLine.percentage >= 10 ? "#f97316"
    : topLine.percentage >= 5 ? "#fbbf24"
    : "#4a5468";
  assert.ok(
    applied.some((entry) => entry.line === topLine.line && entry.color === expectedColor),
    `hottest line ${topLine.line} (${topLine.percentage.toFixed(1)}%) must wear ${expectedColor}`,
  );
}

/** Assert [PROFILE-NOTIFICATIONS-DIAG]: Hint diagnostics from basilisk-profiler. */
async function assertProfilerDiagnosticsPublished(uri: vscode.Uri): Promise<void> {
  const diagnostics = await pollUntilResult({
    fn: async () => vscode.languages.getDiagnostics(uri),
    predicate: (diags) => diags.some((diag) => diag.source === "basilisk-profiler"),
    timeoutMs: DIAGNOSTICS_WAIT_MS,
  });
  const profDiag = diagnostics.find((diag) => diag.source === "basilisk-profiler");
  assert.ok(profDiag, "profiler diagnostics must be published for the hot file");
  assert.strictEqual(profDiag.severity, vscode.DiagnosticSeverity.Hint, "profiler diagnostics are Hints");
}

/** Assert the heat map painted on the burner with palette colors and % text. */
function assertHeatMapPainted(burnerPath: string): void {
  const applied = appliedProfileDecorations().filter((entry) => entry.file === burnerPath);
  const visible = vscode.window.visibleTextEditors.map((e) => e.document.uri.fsPath).join(", ");
  assert.ok(
    applied.length > 0,
    `stopping must paint heat decorations on the open hot file ${burnerPath}; ` +
      `ledger: ${JSON.stringify(appliedProfileDecorations())}; visible editors: [${visible}]; ` +
      `status: ${String(profilerStatusText())}`,
  );
  for (const decoration of applied) {
    assert.ok(
      HEAT_PALETTE.includes(decoration.color),
      `heat colors must come from the brand palette, got ${decoration.color}`,
    );
  }
  assert.ok(
    applied.some((entry) => /\d+(\.\d+)?%/.test(entry.contentText)),
    `decorations must show CPU percentages, got: ${applied.map((entry) => entry.contentText).join(" | ")}`,
  );
  assert.ok(
    applied.some((entry) => entry.line >= HOT_FUNCTION_DEF_LINE),
    "the hot_function body must carry heat decorations",
  );
}

 
suite("CPU profiling — real end-to-end", () => {
  let tmpDir = "";
  let burner: ChildProcess | undefined;
  let burnerPath = "";
  let burnerUri: vscode.Uri | undefined;

  suiteSetup(async function () {
    this.timeout(60_000);
    const result = await setupLspTestSuite("basilisk-cpu-e2e-");
    tmpDir = result.tmpDir;
    burnerPath = path.join(tmpDir, "burner.py");
    const opened = await openPythonFile(tmpDir, "burner.py", BURNER_SOURCE);
    burnerUri = opened.uri;
    burner = await spawnBurner(burnerPath);
  });

  suiteTeardown(async function () {
    this.timeout(30_000);
    await stopAllProfilerSessions();
    burner?.kill("SIGKILL");
    clearProfileDecorations();
    await closeAllEditors();
    teardownLspTestSuite(tmpDir);
  });

  // Runs FIRST (the suite is fail-fast): it asserts the raw data, so a
  // sampling failure surfaces with the profile contents instead of a bare
  // "no decorations" from the UI-flow test below.
  test("raw pipeline: hot function attributed, speedscope + .cpuprofile artifacts written and parseable", async function () {
    if (process.platform !== "linux") { this.skip(); }
    this.timeout(40_000);
    const pid = burner?.pid;
    assert.ok(pid !== undefined && pid > 0, "burner must be running");

    const started = await vscode.commands.executeCommand<StartResult>("basilisk.profiler.start", {
      pid,
      sampleRate: 200,
    });
    assert.ok(started.sessionId.length > 0, "start must mint a session");
    assert.ok(started.pythonVersion.startsWith("3."), `expected Python 3.x, got ${started.pythonVersion}`);

    await new Promise<void>((resolve) => setTimeout(resolve, SAMPLE_WINDOW_MS));

    const result = await vscode.commands.executeCommand<ProfileResult>("basilisk.profiler.stop", {
      sessionId: started.sessionId,
      format: "speedscope",
    });

    assert.ok(result.totalSamples > 0, "real sampling must collect samples");
    assert.ok(
      result.hotFunctions.some((fn) => fn.name === "hot_function"),
      `hot_function must be attributed, got: ${JSON.stringify(result.hotFunctions)}`,
    );
    assert.ok(
      result.hotLines.length > 0,
      `hot lines must be detected; result: ${JSON.stringify(result)}`,
    );
    assert.ok(
      result.hotLines.some((line) => line.file === burnerPath),
      `hot lines must carry the editor's exact path ${burnerPath}, got: ${ 
        JSON.stringify(result.hotLines.map((line) => line.file))}`,
    );

    assertSpeedscopeArtifact(result.outputFile);
    assertCpuProfileArtifact(result.cpuProfilePath);
    assertHottestLineTier(result, burnerPath);
  });

  test("panel one-click flow: attach → live progress in status bar → stop paints the heat map", async function () {
    if (process.platform !== "linux") { this.skip(); }
    this.timeout(40_000);
    const store = getStore();
    assert.ok(store, "store must be initialized");
    const pid = burner?.pid;
    assert.ok(pid !== undefined && pid > 0, "burner must be running");

    await startProfilingForPid(store, pid, "default");
    assert.ok(profilerStatusText() !== undefined, "status bar must show a profiling state after start");

    // [PROFILE-NOTIFICATIONS-PROGRESS]: a NON-ZERO live sample count reaches
    // the status bar — "0 samples" would mean sampling is silently broken.
    await pollUntilResult({
      fn: async () => profilerStatusText() ?? "",
      predicate: (text) => /[1-9][\d.]* ?K? samples/.test(text),
      timeoutMs: PROGRESS_WAIT_MS,
    });

    await new Promise<void>((resolve) => setTimeout(resolve, SAMPLE_WINDOW_MS));
    await vscode.commands.executeCommand("basilisk.profileStop");

    assertHeatMapPainted(burnerPath);

    const uri = burnerUri;
    assert.ok(uri, "burner uri must exist");
    await assertProfilerDiagnosticsPublished(uri);
  });

  test("attaching to an exited process fails with a distinct, classified cause (#81)", async function () {
    this.timeout(40_000);
    // A guaranteed-dead PID exercises the classified failure path on every
    // platform WITHOUT touching the macOS osascript elevation prompt — a GUI
    // dialog no automated suite may trigger (the live-target helper paths are
    // covered by the Rust profiler_helper_socket e2e suite).
    const deadPid = Number(
      execFileSync(PYTHON, ["-c", "import os; print(os.getpid())"], { encoding: "utf8" }).trim(),
    );
    assert.ok(deadPid > 0, "must obtain a freshly exited PID");

    let message = "";
    try {
      await vscode.commands.executeCommand("basilisk.profiler.start", { pid: deadPid });
      assert.fail("attach to an exited process must fail");
    } catch (err: unknown) {
      message = err instanceof Error ? err.message : String(err);
    }
    assert.ok(
      /not found|No such process|py-spy attach failed/i.test(message),
      `the failure must name the real cause distinctly (#81), got: ${message}`,
    );
    assert.ok(
      !message.trim().endsWith("helper closed the connection before confirming attach"),
      `the bare EOF message must never be the whole story (#81): ${message}`,
    );
  });

  test("flamegraph webview HTML renders the dashboard from a profile result", () => {
    const result: ProfileResult = {
      sessionId: "s-test",
      duration: 3,
      totalSamples: 600,
      outputFile: "/tmp/profile.speedscope.json",
      hotFunctions: [
        { name: "hot_function", file: burnerPath, line: HOT_FUNCTION_DEF_LINE, samples: 540, percentage: 90, selfPercentage: 85 },
      ],
      hotLines: [{ file: burnerPath, line: HOT_FUNCTION_DEF_LINE + 2, samples: 500, percentage: 83 }],
    };
    const html = buildFlamegraphHtml(result);
    assert.ok(html.includes("hot_function"), "the profile data (hot functions) must be embedded");
    assert.ok(html.toLowerCase().includes("#e8500a"), "the Basilisk orange palette must be used");
    assert.ok(html.includes("navigateToSource"), "rows must navigate to source");
    assert.ok(html.includes("fn-body"), "the hot-functions table must render");
    assert.ok(html.includes(String(result.totalSamples)), "the summary must show the real sample count");
  });
});
