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
import { evaluateInDebugSession, waitForStoppedFrame } from "../../dap-evaluate";
import { buildProfileLaunchConfig } from "../../process-launch";
import { profilerStatusText, startProfilingForPid } from "../../profiler";
import { pythonProcessesViewState } from "../../process-reactivity";
import { recordedOperations } from "../../progress-ops";
import {
  applyProfileDecorations,
  clearProfileDecorations,
  appliedProfileDecorations,
  type ProfileResult,
} from "../../profiler-decorations";
import {
  buildFlamegraphHtml,
  disposeFlamegraphPanel,
  flamegraphPanelOpen,
  profileHasNoUsableData,
} from "../../profiler-flamegraph-html";
import {
  openPythonFile,
  pollUntilResult,
  setupLspTestSuite,
  teardownLspTestSuite,
  closeAllEditors,
  waitForLspReady,
} from "./test-helpers";

/** How long the burner keeps spinning (covers the whole suite). */
const BURNER_LIFETIME_SECS = 120;
/** Sampling window before stopping a profile. */
const SAMPLE_WINDOW_MS = 2_500;
/**
 * Ceiling for the cooperative sampler to attribute the hot loop. That path runs
 * the debuggee under debugpy line-tracing, so on a slow/contended CI runner the
 * interpreter can spend several seconds in Python/debugpy startup before the hot
 * loop dominates the samples. Poll snapshots up to this budget instead of
 * assuming a fixed window — keeping the assertion strict without being
 * timing-fragile. The burner spins for BURNER_LIFETIME_SECS, well beyond this.
 */
const HOT_ATTRIBUTION_TIMEOUT_MS = 30_000;
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
function assertCpuProfileArtifact(cpuProfilePath: string | undefined, expectedFunction?: string): void {
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
  if (expectedFunction !== undefined) {
    assert.ok(
      cpuprofile.nodes?.some((node) => node.callFrame?.functionName === expectedFunction),
      `.cpuprofile call tree must include ${expectedFunction}`,
    );
  }
}

function hasBurnerHotFunction(result: Pick<ProfileResult, "hotFunctions">, burnerPath: string): boolean {
  const expected = path.resolve(burnerPath);
  return result.hotFunctions.some((fn) => path.resolve(fn.file) === expected);
}

function hotFunctionSummary(result: Pick<ProfileResult, "hotFunctions">): string {
  return result.hotFunctions
    .map((fn) => `${fn.name}@${fn.file}:${fn.line}`)
    .join(", ");
}

/** Assert the burner's hottest line wears the correctly-tiered palette color. */
function assertHottestLineTier(result: ProfileResult, burnerPath: string): void {
  applyProfileDecorations(result);
  const applied = appliedProfileDecorations().filter((entry) => entry.file === burnerPath);
  assert.ok(applied.length > 0, "real profile data must paint the open hot file");
  // Tier-check the hottest line OF THE BURNER — under debugpy, tracer
  // machinery can own the globally hottest line in a file that isn't open.
  const topLine = [...result.hotLines]
    .filter((line) => line.file === burnerPath)
    .sort((a, b) => b.percentage - a.percentage)[0];
  assert.ok(topLine !== undefined, `the burner must have hot lines, got: ${JSON.stringify(result.hotLines)}`);
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

/** Wait for the active debug session to terminate. */
async function waitForDebugSessionEnd(): Promise<void> {
  await pollUntilResult({
    fn: async () => vscode.debug.activeDebugSession,
    predicate: (session) => session === undefined,
    timeoutMs: 20_000,
    intervalMs: 100,
  });
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

// eslint-disable-next-line max-lines-per-function -- e2e suite: six sequential journeys over one shared burner
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
    assertCpuProfileArtifact(result.cpuProfilePath, "hot_function");
    assertHottestLineTier(result, burnerPath);
  });

  test("panel one-click flow: attach → live progress in status bar → stop paints the heat map", async function () {
    if (process.platform !== "linux") { this.skip(); }
    this.timeout(40_000);
    const store = getStore();
    assert.ok(store, "store must be initialized");
    const pid = burner?.pid;
    assert.ok(pid !== undefined && pid > 0, "burner must be running");

    const opsBefore = recordedOperations().length;
    await startProfilingForPid(store, pid, "default");
    assert.ok(profilerStatusText() !== undefined, "status bar must show a profiling state after start");

    // [PROFILE-PROCESSES-REACTIVE] The real session must drive the reactive
    // store + panel: busy (so the launch buttons gate off) and the panel names
    // the profiled PID.
    assert.strictEqual(store.profiler.value.cpu, "active", "the store must mark the CPU profile active");
    assert.strictEqual(store.profilerBusy.value, true, "an active profile makes the panel busy");
    assert.ok(
      pythonProcessesViewState().message?.includes(`PID ${pid}`) === true,
      `the panel must show the profiled PID live: ${String(pythonProcessesViewState().message)}`,
    );

    // [PROFILE-UX-PROGRESS] The attach must run under a progress notification.
    const startOps = recordedOperations().slice(opsBefore);
    assert.ok(
      startOps.includes("begin:Basilisk: Starting CPU profiler") &&
        startOps.includes("end:Basilisk: Starting CPU profiler"),
      `panel attach must show progress, ops: ${startOps.join(" | ")}`,
    );

    // [PROFILE-NOTIFICATIONS-PROGRESS]: a NON-ZERO live sample count reaches
    // the status bar — "0 samples" would mean sampling is silently broken.
    await pollUntilResult({
      fn: async () => profilerStatusText() ?? "",
      predicate: (text) => /[1-9][\d.]* ?K? samples/.test(text),
      timeoutMs: PROGRESS_WAIT_MS,
    });

    await new Promise<void>((resolve) => setTimeout(resolve, SAMPLE_WINDOW_MS));
    await vscode.commands.executeCommand("basilisk.profileStop");

    // [PROFILE-PROCESSES-REACTIVE] Stop must clear the reactive state so the
    // panel re-offers the launches and drops its live chrome.
    assert.strictEqual(store.profiler.value.cpu, "idle", "stop must clear the store session");
    assert.strictEqual(store.profilerBusy.value, false, "stop must clear busy");
    assert.strictEqual(pythonProcessesViewState().message, undefined, "stop must clear the panel chrome");

    assertHeatMapPainted(burnerPath);

    const uri = burnerUri;
    assert.ok(uri, "burner uri must exist");
    await assertProfilerDiagnosticsPublished(uri);
  });

  // Runs on EVERY platform — the cooperative sampler needs no task ports, so
  // this is the real CPU e2e that macOS can execute too ([PROFILE-COOPERATIVE]).
  test("cooperative sampler: OOTB CPU profile of a debug-launched session", async function () {
    this.timeout(60_000);
    try {
      const started = await vscode.debug.startDebugging(undefined, {
        name: "Cooperative CPU E2E",
        type: "basilisk-debug",
        request: "launch",
        program: burnerPath,
        stopOnEntry: true,
        justMyCode: true,
        console: "internalConsole",
      });
      assert.ok(started, "the debug session must launch");

      const frameId = await waitForStoppedFrame();
      assert.ok(frameId !== null, "the debuggee must pause at entry for injection");

      const leg1 = await vscode.commands.executeCommand<{ script: string; sampleFile: string }>(
        "basilisk.profiler.cooperativeScript",
        { sampleRate: 100 },
      );
      assert.ok(leg1.script.includes("sys._current_frames"), "the script must be the in-process sampler");

      const ack = await evaluateInDebugSession(leg1.script, frameId);
      await vscode.commands.executeCommand("workbench.action.debug.continue");
      assert.ok(ack?.includes("__BASILISK_CPU_ACK__") === true, `injection must ack, got: ${String(ack)}`);

      const session = await vscode.commands.executeCommand<StartResult>(
        "basilisk.profiler.cooperativeAttach",
        { sampleFile: leg1.sampleFile, sampleRate: 100 },
      );
      assert.ok(session.sessionId.length > 0, "cooperative attach must mint a session");
      assert.ok(session.pythonVersion.startsWith("3."), `expected Python 3.x, got ${session.pythonVersion}`);

      // The debuggee runs under debugpy line-tracing, so attribution may land on
      // `main`/`<module>` instead of the nested hot function on CI. Poll until
      // the opened burner.py is attributed rather than accepting debugpy-only
      // frames or assuming a fixed window.
      await pollUntilResult({
        fn: () =>
          vscode.commands.executeCommand<ProfileResult>("basilisk.profiler.snapshot", {
            sessionId: session.sessionId,
            format: "speedscope",
          }),
        predicate: (snap) => hasBurnerHotFunction(snap, burnerPath),
        timeoutMs: HOT_ATTRIBUTION_TIMEOUT_MS,
        intervalMs: SAMPLE_WINDOW_MS,
      });

      const result = await vscode.commands.executeCommand<ProfileResult>("basilisk.profiler.stop", {
        sessionId: session.sessionId,
        format: "speedscope",
      });
      assert.ok(result.totalSamples > 0, "the in-process sampler must collect real ticks");
      assert.ok(
        hasBurnerHotFunction(result, burnerPath),
        `burner.py must be attributed, got: ${hotFunctionSummary(result)}`,
      );
      assertCpuProfileArtifact(result.cpuProfilePath);
      assertHottestLineTier(result, burnerPath);
    } finally {
      await vscode.debug.stopDebugging();
      await waitForDebugSessionEnd();
    }
  });

  test("one-click 'Run & Profile CPU' is OOTB on macOS via the cooperative sampler (#82)", async function () {
    if (process.platform !== "darwin") { this.skip(); }
    this.timeout(60_000);
    const opsBefore = recordedOperations().length;
    try {
      const started = await vscode.debug.startDebugging(
        undefined,
        buildProfileLaunchConfig("cpu", burnerPath),
      );
      assert.ok(started, "the metric-explicit CPU launch must start");

      // [PROFILE-UX-PROGRESS] No silence between the click and live data: the
      // status bar must show SOMETHING (starting spinner or sample counter)
      // almost immediately after the launch.
      await pollUntilResult({
        fn: async () => profilerStatusText(),
        predicate: (text) => text !== undefined,
        timeoutMs: 10_000,
      });

      // The cooperative flow injects at entry, resumes, and adopts the
      // session — live NON-ZERO sample counts must reach the status bar
      // without any elevation prompt.
      await pollUntilResult({
        fn: async () => profilerStatusText() ?? "",
        predicate: (text) => /[1-9][\d.]* ?K? samples/.test(text),
        timeoutMs: 20_000,
      });

      // Let the sampler accumulate a real window before stopping, exactly like
      // the sibling heat-map journeys: stopping at the first observed sample
      // can leave zero samples attributed to the burner's lines (all in
      // bootstrap/injection frames), so no heat decorations would paint.
      await new Promise<void>((resolve) => setTimeout(resolve, SAMPLE_WINDOW_MS));

      // [PROFILE-PROCESSES-REACTIVE] The OOTB one-click flow must drive the
      // reactive panel on macOS too: busy + a live "Profiling PID …" readout.
      const store = getStore();
      assert.ok(store, "store must be initialized");
      assert.strictEqual(store.profiler.value.cpu, "active", "the cooperative session must mark the store active");
      assert.ok(
        pythonProcessesViewState().message?.includes("Profiling PID") === true,
        `the panel must show the live profile: ${String(pythonProcessesViewState().message)}`,
      );

      await vscode.commands.executeCommand("basilisk.profileStop");
      assert.strictEqual(store.profiler.value.cpu, "idle", "stop must clear the reactive store state");
      assert.strictEqual(pythonProcessesViewState().message, undefined, "stop must clear the panel chrome");
      assertHeatMapPainted(burnerPath);

      // [PROFILE-UX-PROGRESS] Both the start and the stop must have run under
      // progress notifications that opened and closed.
      const ops = recordedOperations().slice(opsBefore);
      for (const title of ["Basilisk: Starting CPU profiler", "Basilisk: Stopping profiler"]) {
        const begin = ops.indexOf(`begin:${title}`);
        const end = ops.indexOf(`end:${title}`);
        assert.ok(begin !== -1, `"${title}" must show progress, ops: ${ops.join(" | ")}`);
        assert.ok(end > begin, `"${title}" progress must close on completion`);
      }
      assert.strictEqual(
        profilerStatusText(),
        undefined,
        "the status bar must clear after stop — no zombie spinner",
      );
    } finally {
      await vscode.debug.stopDebugging();
      await waitForDebugSessionEnd();
    }
  });

  // The viewability flow (#145): a completed CPU profile must hand the user a
  // working flame chart. The built-in `.cpuprofile` viewer can refuse to render
  // (viewer unavailable, or a profile it rejects), and `vscode.open` doesn't
  // reject when that happens — so the completion notification itself MUST offer
  // an action that lands on the self-contained flamegraph webview, never a
  // dead-end on "the editor could not be opened". Covers [PROFILE-NATIVE-FALLBACK]
  // (docs/specs/LSP-PROFILING-SPEC.md#PROFILE-NATIVE-FALLBACK) + acceptance
  // criteria 2/3/4 of the issue.
  test("run → profile → view: completion notification opens a working flame chart, never a dead-end (#145)", async function () {
    if (process.platform === "win32") { this.skip(); }
    this.timeout(60_000);
    const store = getStore();
    assert.ok(store, "store must be initialized");
    const burnerPid = burner?.pid;
    assert.ok(burnerPid !== undefined && burnerPid > 0, "burner must be running");

    // Adopt a REAL active CPU session through each platform's proven path:
    // the cooperative auto-launch on macOS, the panel py-spy attach on Linux.
    if (process.platform === "darwin") {
      const launched = await vscode.debug.startDebugging(
        undefined,
        buildProfileLaunchConfig("cpu", burnerPath),
      );
      assert.ok(launched, "the metric-explicit CPU launch must start");
      await pollUntilResult({
        fn: async () => store.profiler.value.cpu,
        predicate: (state) => state === "active",
        timeoutMs: 30_000,
      });
    } else {
      await startProfilingForPid(store, burnerPid, "default");
      assert.strictEqual(store.profiler.value.cpu, "active", "the panel attach must activate the session");
    }

    // Capture the completion notification's actions and simulate the user
    // taking the flame-chart action when it is offered.
    const toasts: { message: string; actions: string[] }[] = [];
    const win = vscode.window as { showInformationMessage: typeof vscode.window.showInformationMessage };
    const originalShow = win.showInformationMessage;
    win.showInformationMessage = async (message: string, ...items: unknown[]) => {
      const actions = items.filter((item): item is string => typeof item === "string");
      toasts.push({ message, actions });
      return actions.find((action) => /flame|view|open/i.test(action));
    };

    disposeFlamegraphPanel(); // known-closed baseline so the post-stop check is meaningful
    try {
      await new Promise<void>((resolve) => setTimeout(resolve, SAMPLE_WINDOW_MS));
      await vscode.commands.executeCommand("basilisk.profileStop");
    } finally {
      win.showInformationMessage = originalShow;
    }

    const completion = toasts.find((toast) => /Profile complete/i.test(toast.message));
    assert.ok(
      completion !== undefined,
      `a "Profile complete" notification must be shown, got: ${JSON.stringify(toasts)}`,
    );
    assert.ok(
      completion.actions.length > 0,
      `the "Profile complete" notification must offer an action to reach the result (#145) — ` +
        `the toast currently dead-ends with no way to open or reveal the trace`,
    );
    // The toast is fired-and-forget (sticky notifications must not block the
    // stop handler), so the action's view opens on a microtask — poll for it.
    await pollUntilResult({
      fn: async () => flamegraphPanelOpen(),
      predicate: (open) => open,
      timeoutMs: 5_000,
    }).catch(() => {
      assert.fail(
        "taking the completion action must open a working flame chart, never leave the user " +
          "on the built-in viewer's \"could not be opened\" error (#145)",
      );
    });

    if (vscode.debug.activeDebugSession !== undefined) {
      await vscode.debug.stopDebugging();
      await waitForDebugSessionEnd();
    }
    disposeFlamegraphPanel();
  });

  // The LSP runtime can be re-created within one extension session (store
  // reset → a brand-new LanguageClient). The regression: the profiler's
  // progress listener was registered once, on the first client only, so after
  // a runtime re-creation the live sample counter silently died — the status
  // bar sat on "Profiling..." with no data forever. The listener must follow
  // the store's client signal ([PROFILE-NOTIFICATIONS-PROGRESS],
  // [PROFILE-PROCESSES-REACTIVE]).
  test("live progress survives an LSP client re-creation — the sample counter never goes silently dead", async function () {
    if (process.platform === "win32") { this.skip(); }
    this.timeout(120_000);
    const store = getStore();
    assert.ok(store, "store must be initialized");
    const oldClient = store.client.value;
    assert.ok(oldClient, "a running client must exist before the re-creation");

    // Recreate the runtime: reset() → onReset → startRuntime → NEW LanguageClient.
    store.reset();
    await pollUntilResult({
      fn: async () => store.client.value,
      predicate: (client) => client !== undefined && client !== oldClient,
      timeoutMs: 30_000,
    });
    await waitForLspReady();

    try {
      // One-click profile on the fresh runtime (cooperative on macOS, py-spy
      // attach to the debuggee elsewhere — the same [PROFILE-PROCESSES-LAUNCH-FILE]
      // routing users get).
      const launched = await vscode.debug.startDebugging(
        undefined,
        buildProfileLaunchConfig("cpu", burnerPath),
      );
      assert.ok(launched, "the CPU launch must start on the re-created runtime");

      // The live NON-ZERO sample counter must reach the status bar — with a
      // dead listener this sits on "Profiling..." until the timeout.
      await pollUntilResult({
        fn: async () => profilerStatusText() ?? "",
        predicate: (text) => /[1-9][\d.]* ?K? samples/.test(text),
        timeoutMs: 30_000,
      });

      await vscode.commands.executeCommand("basilisk.profileStop");
      assert.strictEqual(store.profiler.value.cpu, "idle", "stop must clear the session");
    } finally {
      await vscode.debug.stopDebugging();
      await waitForDebugSessionEnd();
    }
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

  // [PROFILE-SHORT-PROGRAM] #145: a sub-tick program (debug_demo.py runs ~1ms)
  // finishes before its work can be sampled, so the session can capture dozens
  // of samples that resolve to ZERO user-code attribution (the real observed
  // case: 48 samples, 0 hot functions, 0 hot lines). The launch flow flags
  // "no usable data" honestly instead of presenting an empty chart — keying off
  // attribution, not raw sample count.
  test("a profile with no hot functions/lines is flagged unusable; one with hotspots is not (#145)", () => {
    assert.ok(
      profileHasNoUsableData({ hotFunctions: [], hotLines: [] }),
      "the observed 48-sample/0-function idle case has nothing to show",
    );
    const withFunction: Pick<ProfileResult, "hotFunctions" | "hotLines"> = {
      hotFunctions: [{ name: "f", file: "/a.py", line: 1, samples: 3, percentage: 100, selfPercentage: 100 }],
      hotLines: [],
    };
    assert.ok(!profileHasNoUsableData(withFunction), "a real hot function is usable data, even if sparse");
    const withLine: Pick<ProfileResult, "hotFunctions" | "hotLines"> = {
      hotFunctions: [],
      hotLines: [{ file: "/a.py", line: 2, samples: 3, percentage: 100 }],
    };
    assert.ok(!profileHasNoUsableData(withLine), "a real hot line is usable data");
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

  // [PROFILE-VIEWER-DELIVERY]: speedscope.app is https and cannot read file://
  // URLs, so a `#profileURL=file://` link always fails ("Something went wrong").
  // The "Open in Speedscope" link must instead post a message the extension
  // actually handles (reveal the JSON + open the app for drag-and-drop import).
  test("the flamegraph's Speedscope link uses a working import path, not a dead file:// URL", () => {
    const result: ProfileResult = {
      sessionId: "s-test",
      duration: 3,
      totalSamples: 600,
      outputFile: "/tmp/profile.speedscope.json",
      hotFunctions: [],
      hotLines: [],
    };
    const html = buildFlamegraphHtml(result);
    assert.ok(
      !html.includes("speedscope.app/#profileURL=file://"),
      "must not emit the always-failing speedscope file:// URL ([PROFILE-VIEWER-DELIVERY])",
    );
    assert.ok(
      html.includes("openSpeedscope"),
      "the Speedscope link must post an openSpeedscope message the extension handles",
    );
  });

  // [PROFILE-NATIVE-FALLBACK]: frame names/paths come from the profiled (possibly
  // third-party) program. CPython emits synthetic names like <module>/<lambda>,
  // and a hostile program can name a function `</script>…`. The webview must
  // escape them before innerHTML, must not let the embedded JSON close the
  // inline <script>, and gates that script behind a per-render CSP nonce.
  test("flamegraph HTML escapes untrusted frame names/paths and nonce-gates its inline script", () => {
    const hostile = "</script><img src=x onerror=alert(1)>";
    const result: ProfileResult = {
      sessionId: "s-test",
      duration: 3,
      totalSamples: 600,
      outputFile: "/tmp/profile.speedscope.json",
      hotFunctions: [
        { name: hostile, file: hostile, line: 1, samples: 540, percentage: 90, selfPercentage: 85 },
        { name: "<module>", file: "/app/main.py", line: 1, samples: 60, percentage: 10, selfPercentage: 10 },
      ],
      hotLines: [{ file: hostile, line: 2, samples: 500, percentage: 83 }],
    };
    const html = buildFlamegraphHtml(result);
    assert.ok(
      !html.includes("</script><img"),
      "embedded profile data must not close the inline <script> element early",
    );
    assert.ok(
      html.includes("escapeHtml(fn.name)"),
      "frame names must be escaped before innerHTML",
    );
    assert.ok(
      html.includes("escapeHtml(basename(fn.file))"),
      "frame file paths must be escaped before innerHTML",
    );
    assert.ok(html.includes("Content-Security-Policy"), "the webview must declare a CSP");
    assert.ok(html.includes('<script nonce="'), "the inline script must carry the CSP nonce");
  });
});
