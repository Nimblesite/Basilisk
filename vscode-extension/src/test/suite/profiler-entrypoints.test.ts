// Tests for [PROFILE-PROCESSES-LAUNCH-FILE]. See
// docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-LAUNCH-FILE
//
// Issue #82: the PYTHON PROCESSES title-bar entry point must state WHAT it
// profiles (CPU vs memory) and let the user choose either metric, with icons
// and labels consistent with the per-row inline actions ("Profile CPU" 🔥 /
// "Track Memory" 🗄️). These tests assert the declarative VSIX surface in
// package.json — the same panel must speak one language at the row level and
// the title level.

import * as assert from "assert";
import * as vscode from "vscode";
import { buildProfileLaunchConfig } from "../../process-launch";
import { shouldProfileOnLaunch } from "../../profiler";
import { PythonProcessesProvider } from "../../process-explorer";
import { type Store } from "../../store";
import {
  getPackageJsonCommands,
  getPackageJsonMenu,
  getPackageJsonViewsWelcome,
  type PackageJsonCommandEntry,
  type PackageJsonViewsWelcomeEntry,
} from "./profiler-test-constants";

/** The metric-explicit run-and-profile entry points (#82). */
const CPU_LAUNCH_COMMAND = "basilisk.profileCurrentFileCpu";
const MEMORY_LAUNCH_COMMAND = "basilisk.trackMemoryCurrentFile";
/** The ambiguous, metric-less command issue #82 retires. */
const AMBIGUOUS_LAUNCH_COMMAND = "basilisk.profileCurrentFile";

/** The view/title entries scoped to the Python Processes panel. */
function pythonProcessesTitleCommands(): string[] {
  return getPackageJsonMenu("view/title")
    .filter((entry) => entry.when?.includes("basilisk.pythonProcesses") === true)
    .map((entry) => entry.command);
}

/** Look up a command's declaration, asserting it exists. */
function commandEntry(commandId: string): PackageJsonCommandEntry {
  const entry = getPackageJsonCommands().find((cmd) => cmd.command === commandId);
  assert.ok(entry, `command "${commandId}" must be declared in package.json`);
  return entry;
}

suite("Python Processes — title-bar entry points state their metric (#82)", () => {
  test("the title bar offers a CPU launch and a memory launch, not one ambiguous button", () => {
    const titleCommands = pythonProcessesTitleCommands();
    assert.ok(
      titleCommands.includes(CPU_LAUNCH_COMMAND),
      `the title bar must offer a CPU-explicit launch (#82); got: ${titleCommands.join(", ")}`,
    );
    assert.ok(
      titleCommands.includes(MEMORY_LAUNCH_COMMAND),
      `the title bar must offer a memory-explicit launch (#82); got: ${titleCommands.join(", ")}`,
    );
    assert.ok(
      !titleCommands.includes(AMBIGUOUS_LAUNCH_COMMAND),
      "the metric-less 'Run & Profile Current File' button must be gone (#82)",
    );
  });

  test("the CPU launch is labelled CPU and wears the flame icon, matching the row action", () => {
    const cpu = commandEntry(CPU_LAUNCH_COMMAND);
    assert.ok(
      cpu.title?.includes("CPU") === true,
      `the CPU launch title must say CPU (#82); got: ${String(cpu.title)}`,
    );
    assert.strictEqual(cpu.icon, "$(flame)", "the CPU launch must reuse the Profile CPU row icon");
  });

  test("the memory launch is labelled Memory and wears the database icon, matching the row action", () => {
    const memory = commandEntry(MEMORY_LAUNCH_COMMAND);
    assert.ok(
      memory.title?.includes("Memory") === true,
      `the memory launch title must say Memory (#82); got: ${String(memory.title)}`,
    );
    assert.strictEqual(
      memory.icon,
      "$(database)",
      "the memory launch must reuse the Track Memory row icon",
    );
  });

  test("the empty-state welcome offers both metric-explicit launches", () => {
    // [PROFILE-UX-PROGRESS] split the welcome into connecting/stopped/running
    // states; the launch buttons live on the server-running empty state.
    const welcome = getPackageJsonViewsWelcome().find(
      (entry) =>
        entry.view === "basilisk.pythonProcesses" &&
        entry.contents.includes("No Python processes running"),
    );
    assert.ok(welcome, "the Python Processes panel must declare a server-running empty state");
    assert.ok(
      welcome.contents.includes(`command:${CPU_LAUNCH_COMMAND}`),
      `the welcome view must link the CPU launch (#82); got: ${welcome.contents}`,
    );
    assert.ok(
      welcome.contents.includes(`command:${MEMORY_LAUNCH_COMMAND}`),
      `the welcome view must link the memory launch (#82); got: ${welcome.contents}`,
    );
    // Anchored with the markdown link's closing paren — `…CurrentFileCpu`
    // legitimately contains `…CurrentFile` as a prefix.
    assert.ok(
      !welcome.contents.includes(`command:${AMBIGUOUS_LAUNCH_COMMAND})`),
      "the welcome view must not link the retired ambiguous command (#82)",
    );
  });

  test("both launches stay palette-gated behind the profiling UI switch", () => {
    const palette = getPackageJsonMenu("commandPalette");
    for (const command of [CPU_LAUNCH_COMMAND, MEMORY_LAUNCH_COMMAND]) {
      const entry = palette.find((item) => item.command === command);
      assert.ok(entry, `${command} must have a commandPalette entry`);
      assert.strictEqual(
        entry.when,
        "basilisk.profilingEnabled",
        `${command} must stay behind the [PROFILE-UI-GATE] context key`,
      );
    }
  });
});

// Tests for [PROFILE-PROCESSES-PANEL]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-PANEL
//
// Issue #147 (scoped to empty-state honesty; the Signals refactor is #148): the
// Python Processes empty state must not float a redundant Refresh button, and
// "No Python processes running" must appear ONLY after a fetch has actually
// succeeded — loading / couldn't-load states must say so honestly instead of
// asserting the definitive negative.
const REFRESH_COMMAND = "basilisk.refreshProcesses";

/** The Python Processes panel's welcome entries. */
function processWelcomes(): PackageJsonViewsWelcomeEntry[] {
  return getPackageJsonViewsWelcome().filter((entry) => entry.view === "basilisk.pythonProcesses");
}

suite("Python Processes — empty state honesty (#147)", () => {
  test("no standalone Refresh button floats in the empty/welcome state", () => {
    // The panel auto-refreshes and a view-title Refresh already exists; a Refresh
    // button in the welcome copy is redundant and self-contradictory (#147).
    for (const welcome of processWelcomes()) {
      assert.ok(
        !welcome.contents.includes(`command:${REFRESH_COMMAND}`),
        `the empty state must not carry a standalone Refresh button (#147); got: ${welcome.contents}`,
      );
    }
  });

  test('"No Python processes running" appears only after a fetch has succeeded', () => {
    // The definitive negative must be gated on a SUCCEEDED fetch
    // (processesState == loaded), not merely a running server — else a loading /
    // errored / disconnected panel lies that there are no processes (#147).
    const genuinelyEmpty = processWelcomes().find((entry) =>
      entry.contents.includes("No Python processes running"),
    );
    assert.ok(genuinelyEmpty, "a genuinely-empty welcome must exist");
    assert.ok(
      genuinelyEmpty.when?.includes("basilisk.processesState == loaded") === true,
      `"No Python processes running" must be gated on a successful fetch (processesState == loaded), not just a running server (#147); got: ${String(genuinelyEmpty.when)}`,
    );
  });

  test("loading and couldn't-load states are declared, so the panel never lies", () => {
    const whens = processWelcomes().map((entry) => entry.when ?? "");
    assert.ok(
      whens.some((when) => when.includes("basilisk.processesState == loading")),
      `a loading state must be declared so the panel never asserts "no processes" mid-load (#147); got: ${whens.join(" | ")}`,
    );
    assert.ok(
      whens.some((when) => when.includes("basilisk.processesState == error")),
      `a couldn't-load state must be declared so a fetch error doesn't masquerade as "no processes" (#147); got: ${whens.join(" | ")}`,
    );
  });
});

/** Build a provider over a fake store whose client behaves as given (#147 seam). */
function providerWithClient(client: unknown): PythonProcessesProvider {
  return new PythonProcessesProvider({ client: { value: client } } as unknown as Store);
}

suite("Python Processes — fetch-state drives the welcome (#147)", () => {
  // The empty welcome only tells the truth if the provider publishes the right
  // processesState. A still-loading or errored fetch must NEVER read as "loaded"
  // (the genuinely-empty state), or "No Python processes running" lies again.
  test("a freshly created panel is 'loading', never 'no processes'", () => {
    const provider = providerWithClient({ isRunning: () => false });
    assert.strictEqual(provider.processesState, "loading");
    provider.dispose();
  });

  test("a successful fetch settles to 'loaded' — only then is empty genuine", async () => {
    const provider = providerWithClient({ isRunning: () => true, sendRequest: async () => ({ processes: [] }) });
    await provider.refreshNow();
    assert.strictEqual(provider.processesState, "loaded");
    provider.dispose();
  });

  test("a failed fetch settles to 'error', not masquerading as 'no processes'", async () => {
    const provider = providerWithClient({
      isRunning: () => true,
      sendRequest: async () => { throw new Error("disconnected"); },
    });
    await provider.refreshNow();
    assert.strictEqual(provider.processesState, "error");
    provider.dispose();
  });

  test("no running client stays 'loading' (the serverState welcome owns the copy)", async () => {
    const provider = providerWithClient(undefined);
    await provider.refreshNow();
    assert.strictEqual(provider.processesState, "loading");
    provider.dispose();
  });
});

suite("Run & Profile registered commands (#82)", () => {
  test("both registered launches refuse to start a session without a Python file open", async () => {
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    // Driving the REAL registered commands end-to-end: each must exist (or
    // executeCommand rejects) and must decline to launch with no active
    // Python editor instead of starting a broken session.
    await vscode.commands.executeCommand("basilisk.profileCurrentFileCpu");
    await vscode.commands.executeCommand("basilisk.trackMemoryCurrentFile");
    assert.strictEqual(
      vscode.debug.activeDebugSession,
      undefined,
      "no debug session may start when no Python file is open",
    );
  });
});

suite("Run & Profile launch configurations (#82)", () => {
  test("the CPU launch asks for profiling on launch and names its metric", () => {
    const config = buildProfileLaunchConfig("cpu", "/work/app.py");
    assert.strictEqual(config.type, "basilisk-debug");
    assert.strictEqual(config.request, "launch");
    assert.strictEqual(config.program, "/work/app.py");
    assert.strictEqual(
      config.profileOnLaunch,
      true,
      "the CPU launch must actually start the profiler, not just run the file",
    );
    assert.ok(config.name.includes("CPU"), `the session name must state the metric: ${config.name}`);
  });

  test("the memory launch stops on entry so tracemalloc can be injected, then tracked", () => {
    const config = buildProfileLaunchConfig("memory", "/work/app.py");
    assert.strictEqual(config.program, "/work/app.py");
    assert.strictEqual(
      config.stopOnEntry,
      true,
      "memory tracking injects tracemalloc via DAP evaluate, which needs a paused debuggee",
    );
    assert.strictEqual(
      config.memoryTrackOnLaunch,
      true,
      "memory-profiler.ts keys its auto-start flow off this flag",
    );
    assert.ok(config.name.includes("Memory"), `the session name must state the metric: ${config.name}`);
  });

  test("a launch config requesting profileOnLaunch is honoured even with the global setting off", async () => {
    const cfg = vscode.workspace.getConfiguration("basilisk");
    await cfg.update("profiler.profileOnLaunch", undefined, vscode.ConfigurationTarget.Global);

    const fromEntryPoint = {
      type: "basilisk-debug",
      configuration: buildProfileLaunchConfig("cpu", "/work/app.py"),
    };
    assert.strictEqual(
      shouldProfileOnLaunch(fromEntryPoint),
      true,
      "the 'Run & Profile CPU' launch must auto-profile without requiring the global setting (#82)",
    );

    const plainLaunch = {
      type: "basilisk-debug",
      configuration: { type: "basilisk-debug", request: "launch", name: "plain" },
    };
    assert.strictEqual(
      shouldProfileOnLaunch(plainLaunch),
      false,
      "a plain debug session must not be auto-profiled by default",
    );

    const foreignSession = {
      type: "node",
      configuration: buildProfileLaunchConfig("cpu", "/work/app.py"),
    };
    assert.strictEqual(
      shouldProfileOnLaunch(foreignSession),
      false,
      "non-Basilisk sessions are never auto-profiled",
    );
  });

  // dap-1: the global setting is read directly by shouldProfileOnLaunch, so the
  // stamp carve-out in applyDebugConfigDefaults is not enough on its own — the
  // CPU auto-start must itself refuse a memory-tracking session.
  test("a memory-tracking launch is never CPU-auto-profiled, even with the global setting on (dap-1)", async () => {
    const cfg = vscode.workspace.getConfiguration("basilisk");
    await cfg.update("profiler.profileOnLaunch", true, vscode.ConfigurationTarget.Global);
    try {
      const memorySession = {
        type: "basilisk-debug",
        configuration: buildProfileLaunchConfig("memory", "/work/app.py"),
      };
      assert.strictEqual(
        shouldProfileOnLaunch(memorySession),
        false,
        "a memory launch must not auto-start the CPU sampler — it would collide with tracemalloc at the entry pause (dap-1)",
      );
    } finally {
      await cfg.update("profiler.profileOnLaunch", undefined, vscode.ConfigurationTarget.Global);
    }
  });
});
