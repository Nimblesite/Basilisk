// Tests for [PROFILE-PROCESSES-PANEL]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES-PANEL
//
// Component tests for the Python Processes panel (#62). Per CLAUDE.md these
// assert behavior through internal VSIX state — instantiate the provider, feed
// it a stubbed ProcessInfo[] via a fake LSP client, and assert getChildren()
// yields the expected sorted/grouped/filtered rows and that each row carries
// the PID a one-click profiling action needs. No getCommands()/whenCommandReady.

import * as assert from "assert";
import * as vscode from "vscode";
import { type LanguageClient } from "vscode-languageclient/node";
import { PythonProcessesProvider, type ProcessInfo } from "../../process-explorer";
import { createProcessRowActions, memoryTrackRoute } from "../../process-launch";
import { createStore, type Store } from "../../store";

const MB = 1024 * 1024;

/** A representative process table covering both kinds, users, and versions. */
const STUB_PROCESSES: readonly ProcessInfo[] = [
  {
    pid: 100, ppid: 1, name: "python3.12", interpreterPath: "/usr/bin/python3.12",
    script: "/app/web.py", pythonVersion: "3.12.1", cpuPercent: 5, memoryBytes: 50 * MB,
    runtimeSecs: 10, user: "alice", requiresElevation: false, kind: "interpreter",
  },
  {
    pid: 200, ppid: 1, name: "python3.11", interpreterPath: "/usr/bin/python3.11",
    script: "/app/worker.py", pythonVersion: "3.11.7", cpuPercent: 42, memoryBytes: 10 * MB,
    runtimeSecs: 99, user: "bob", requiresElevation: true, kind: "interpreter",
  },
  {
    pid: 300, ppid: 200, name: "python3.12", interpreterPath: "/usr/bin/python3.12",
    script: "/app/uvicorn.py", pythonVersion: "3.12.1", cpuPercent: 1, memoryBytes: 99 * MB,
    runtimeSecs: 5, user: "alice", requiresElevation: false, kind: "launcher",
  },
];

/** One `workspace/executeCommand` request captured by the recording client. */
interface RecordedRequest {
  readonly command: string;
  readonly arguments: readonly unknown[];
}

/**
 * Build a Store whose LSP client returns the given process table and records
 * every executeCommand request so tests can assert what was sent (e.g. that an
 * inline action really issued `basilisk.profiler.start` with the row's PID).
 */
function storeWith(processes: readonly ProcessInfo[], requests?: RecordedRequest[]): Store {
  const store = createStore();
  const client = {
    isRunning: (): boolean => true,
    onDidChangeState: (): vscode.Disposable => ({ dispose: (): undefined => undefined }),
    sendRequest: async (_method: string, param?: RecordedRequest): Promise<unknown> => {
      if (param !== undefined) { requests?.push(param); }
      if (param?.command === "basilisk.profiler.start") { return undefined; }
      return { processes };
    },
  } as unknown as LanguageClient;
  store.setClient({ subscriptions: [] } as unknown as vscode.ExtensionContext, client);
  return store;
}

/** The `basilisk.profiler.start` requests among the recorded ones. */
function profilerStarts(requests: readonly RecordedRequest[]): RecordedRequest[] {
  return requests.filter((req) => req.command === "basilisk.profiler.start");
}

/** The pid argument of a recorded `basilisk.profiler.start` request. */
function startPid(request: RecordedRequest): unknown {
  return (request.arguments[0] as { pid?: unknown } | undefined)?.pid;
}

/** Read the PID a process row carries (the arg passed to inline commands). */
function pidOf(item: vscode.TreeItem): number | undefined {
  return (item as unknown as { process?: ProcessInfo }).process?.pid;
}

/** Read the members a group row carries. */
function membersOf(item: vscode.TreeItem): readonly ProcessInfo[] {
  return (item as unknown as { members?: readonly ProcessInfo[] }).members ?? [];
}

/** Read a group header's label (a plain string at runtime). */
function labelText(item: vscode.TreeItem): string {
  return (item as unknown as { label?: string }).label ?? "";
}

suite("Python Processes Panel", () => {
  let provider: PythonProcessesProvider;

  teardown(() => {
    provider.dispose();
  });

  test("lists every process sorted by CPU descending by default", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const rows = await provider.getChildren();
    assert.deepStrictEqual(rows.map(pidOf), [200, 100, 300], "CPU 42 > 5 > 1");
  });

  test("each row carries its PID so inline Profile starts with no input box", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const rows = await provider.getChildren();
    for (const row of rows) {
      assert.strictEqual(typeof pidOf(row), "number", "row must carry a numeric pid for the command arg");
    }
    const worker = rows.find((r) => pidOf(r) === 200);
    assert.ok(worker, "the worker process row must exist");
    assert.ok(
      String(worker.description).includes("PID 200"),
      `row description should surface the PID: ${String(worker.description)}`,
    );
  });

  test("rows needing elevation get a distinct contextValue for the lock affordance", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const rows = await provider.getChildren();
    const elevated = rows.find((r) => pidOf(r) === 200);
    const normal = rows.find((r) => pidOf(r) === 100);
    assert.strictEqual(elevated?.contextValue, "pythonProcessElevated");
    assert.strictEqual(normal?.contextValue, "pythonProcess");
  });

  test("sort by memory orders rows by resident size descending", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    provider.cycleSortMode(); // cpu → memory
    const rows = await provider.getChildren();
    assert.deepStrictEqual(rows.map(pidOf), [300, 100, 200], "memory 99 > 50 > 10 MB");
  });

  test("group by Python version buckets processes under collapsible headers", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    provider.cycleGroupMode(); // none → version
    const groups = await provider.getChildren();
    assert.deepStrictEqual(
      groups.map(labelText),
      ["3.11.7", "3.12.1"],
      "groups are sorted by version label",
    );
    const twelve = groups.find((g) => labelText(g) === "3.12.1");
    assert.ok(twelve, "3.12.1 group must exist");
    assert.strictEqual(String(twelve.description), "2", "group shows its member count");

    const members = await provider.getChildren(twelve);
    assert.deepStrictEqual(members.map(pidOf), [100, 300], "both 3.12 processes, CPU-ordered");
  });

  test("filter narrows rows by name, script, or PID substring", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    provider.setFilter("worker");
    let rows = await provider.getChildren();
    assert.deepStrictEqual(rows.map(pidOf), [200], "only worker.py matches");

    provider.setFilter("300");
    rows = await provider.getChildren();
    assert.deepStrictEqual(rows.map(pidOf), [300], "PID substring matches");
  });

  test("group members expose the full member set for the count badge", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    provider.cycleGroupMode();
    const groups = await provider.getChildren();
    const eleven = groups.find((g) => labelText(g) === "3.11.7");
    assert.ok(eleven, "3.11.7 group must exist");
    assert.deepStrictEqual(membersOf(eleven).map((p) => p.pid), [200]);
  });
});

// Tests for [PROFILE-PROCESSES-LAUNCH] issue #79: the inline flame/database
// buttons arrive with `item === undefined` at runtime and must still profile
// the row the user clicked instead of warning "Select a Python process".
suite("Python Processes Panel — inline launch actions (#79)", () => {
  let provider: PythonProcessesProvider;

  teardown(() => {
    provider.dispose();
  });

  test("rows keep a stable id across refreshes so inline buttons survive the auto-refresh", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const before = await provider.getChildren();
    provider.refresh();
    const after = await provider.getChildren();

    const beforeRow = before.find((row) => pidOf(row) === 200);
    const afterRow = after.find((row) => pidOf(row) === 200);
    assert.ok(beforeRow !== undefined && afterRow !== undefined, "PID 200 row must exist in both passes");
    assert.ok(
      typeof beforeRow.id === "string" && beforeRow.id.length > 0,
      "process rows must carry a stable TreeItem.id so VS Code can map an inline click " +
        `back to the element after a 2s auto-refresh (#79); got: ${String(beforeRow.id)}`,
    );
    assert.strictEqual(beforeRow.id, afterRow.id, "the id must be identical across refreshes");
  });

  test("inline Profile CPU invoked without an argument profiles the selected row", async () => {
    const requests: RecordedRequest[] = [];
    const store = storeWith(STUB_PROCESSES, requests);
    provider = new PythonProcessesProvider(store);
    const rows = await provider.getChildren();
    const selectedRow = rows.find((row) => pidOf(row) === 200);
    assert.ok(selectedRow !== undefined, "PID 200 row must exist");

    const actions = createProcessRowActions(store, { selection: [selectedRow] });
    // VS Code passed no argument — the runtime shape of issue #79.
    await actions.profileProcess(undefined);

    const starts = profilerStarts(requests);
    assert.strictEqual(
      starts.length,
      1,
      "clicking the inline flame button must start profiling (not warn) when a row is selected (#79)",
    );
    assert.strictEqual(startPid(starts[0]), 200, "profiling must target the selected row's PID");
  });

  test("an explicitly passed row wins over a different selection", async () => {
    const requests: RecordedRequest[] = [];
    const store = storeWith(STUB_PROCESSES, requests);
    provider = new PythonProcessesProvider(store);
    const rows = await provider.getChildren();
    const clicked = rows.find((row) => pidOf(row) === 300);
    const selected = rows.find((row) => pidOf(row) === 200);
    assert.ok(clicked !== undefined && selected !== undefined, "both rows must exist");

    const actions = createProcessRowActions(store, { selection: [selected] });
    await actions.profileProcess(clicked);

    const starts = profilerStarts(requests);
    assert.strictEqual(starts.length, 1, "the clicked row must be profiled");
    assert.strictEqual(startPid(starts[0]), 300, "the explicit item must win over the selection");
  });

  test("with no item and no selection, nothing is profiled", async () => {
    const requests: RecordedRequest[] = [];
    const store = storeWith(STUB_PROCESSES, requests);
    provider = new PythonProcessesProvider(store);
    await provider.getChildren();

    const actions = createProcessRowActions(store, { selection: [] });
    await actions.profileProcess(undefined);

    assert.strictEqual(
      profilerStarts(requests).length,
      0,
      "without any resolvable target the action must not fire a profiler.start",
    );
  });
});

suite("Python Processes Panel — Track Memory routing", () => {
  // Tests for the memory leg of [PROFILE-PROCESSES-LAUNCH]: tracemalloc rides
  // the DAP courier, so the row action may only ever target the live debuggee.

  /** Drive the row's Track Memory action against PID 100 with the given session. */
  async function trackMemoryOnPid100(
    session: { id: string; type: string } | undefined,
    arrange: (store: Store) => void = () => undefined,
  ): Promise<{ requests: RecordedRequest[]; executed: string[] }> {
    const requests: RecordedRequest[] = [];
    const executed: string[] = [];
    const store = storeWith(STUB_PROCESSES, requests);
    arrange(store);
    const rows = await new PythonProcessesProvider(store).getChildren();
    const selectedRow = rows.find((row) => pidOf(row) === 100);
    assert.ok(selectedRow !== undefined, "PID 100 row must exist");

    const actions = createProcessRowActions(store, { selection: [selectedRow] }, {
      runCommand: async (command) => { executed.push(command); },
      activeSession: () => session,
    });
    await actions.memoryTrackProcess(undefined);
    return { requests, executed };
  }

  test("on the live debuggee it routes to real memory tracking — never a CPU start", async () => {
    const { requests, executed } = await trackMemoryOnPid100(
      { id: "session-1", type: "basilisk-debug" },
      (store) => { store.setDebuggeeProcessId("session-1", 100); },
    );

    assert.deepStrictEqual(
      executed,
      ["basilisk.memoryStart"],
      "Track Memory on the debuggee row must start tracemalloc tracking",
    );
    assert.strictEqual(
      profilerStarts(requests).length,
      0,
      "Track Memory must NEVER start a CPU profiling session (the preset:'memory' defect)",
    );
  });

  test("on an external process it starts nothing and offers the launch flow", async () => {
    // No debug session at all — PID 100 is a foreign process.
    const { requests, executed } = await trackMemoryOnPid100(undefined);

    assert.deepStrictEqual(executed, [], "no memory command can run against a foreign PID");
    assert.strictEqual(
      profilerStarts(requests).length,
      0,
      "an external row must not silently fall back to CPU profiling",
    );
  });

  test("memoryTrackRoute targets the debuggee only when session and PID both match", () => {
    const store = storeWith(STUB_PROCESSES);
    store.setDebuggeeProcessId("session-1", 100);
    const basilisk = { id: "session-1", type: "basilisk-debug" };

    assert.strictEqual(memoryTrackRoute(store, 100, basilisk), "start-tracking");
    assert.strictEqual(memoryTrackRoute(store, 200, basilisk), "offer-launch", "PID mismatch");
    assert.strictEqual(memoryTrackRoute(store, 100, undefined), "offer-launch", "no session");
    assert.strictEqual(
      memoryTrackRoute(store, 100, { id: "session-1", type: "python" }),
      "offer-launch",
      "foreign debug adapter",
    );
  });
});

suite("Python Processes Panel — launcher visibility", () => {
  let provider: PythonProcessesProvider;

  function basiliskConfig(): vscode.WorkspaceConfiguration {
    return vscode.workspace.getConfiguration("basilisk");
  }

  teardown(async () => {
    provider.dispose();
    await basiliskConfig().update("profiler.showLaunchers", undefined, vscode.ConfigurationTarget.Global);
  });

  test("hides launcher processes when profiler.showLaunchers is false", async () => {
    await basiliskConfig().update("profiler.showLaunchers", false, vscode.ConfigurationTarget.Global);
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const rows = await provider.getChildren();
    const pids = rows.map(pidOf);
    assert.ok(!pids.includes(300), "the uvicorn launcher must be hidden");
    assert.ok(pids.includes(100) && pids.includes(200), "bare interpreters remain visible");
  });
});

// ── Inline action target resolution (issue #79) [PROFILE-PROCESSES-PANEL] ──
//
// Clicking the inline flame / database icon on a process row must act on
// THAT row. At runtime VS Code has been observed to invoke the command with
// `item === undefined`; the handler must fall back to the tree view's current
// selection — and only warn when there is truly no target.

suite("Python Processes Panel — inline action target (issue #79)", () => {
  /** Run fn with showWarningMessage stubbed, returning captured warnings. */
  async function captureWarnings(fn: () => Promise<void>): Promise<string[]> {
    const warnings: string[] = [];
    const original = vscode.window.showWarningMessage;
    (vscode.window as { showWarningMessage: unknown }).showWarningMessage = async (
      message: string,
    ): Promise<undefined> => {
      warnings.push(message);
      return Promise.resolve(undefined);
    };
    try {
      await fn();
    } finally {
      (vscode.window as { showWarningMessage: unknown }).showWarningMessage = original;
    }
    return warnings;
  }

  test("undefined item falls back to the tree selection and profiles that PID — without warning", async () => {
    const requests: RecordedRequest[] = [];
    const store = storeWith(STUB_PROCESSES, requests);
    const provider = new PythonProcessesProvider(store);
    try {
      const rows = await provider.getChildren();
      const selected = rows.find((row) => pidOf(row) === 100);
      assert.ok(selected, "expected the PID 100 row");

      const actions = createProcessRowActions(store, { selection: [selected] });
      const warnings = await captureWarnings(async () => actions.profileProcess(undefined));

      assert.deepStrictEqual(warnings, [], "must not warn when a row is selected");
      const starts = profilerStarts(requests);
      assert.strictEqual(starts.length, 1, "profiler start must be requested");
      assert.strictEqual(startPid(starts[0]), 100, "must profile the selected row's PID");
    } finally {
      provider.dispose();
    }
  });

  test("memory tracking falls back to the tree selection the same way — without warning", async () => {
    const requests: RecordedRequest[] = [];
    const executed: string[] = [];
    const store = storeWith(STUB_PROCESSES, requests);
    store.setDebuggeeProcessId("session-1", 200);
    const provider = new PythonProcessesProvider(store);
    try {
      const rows = await provider.getChildren();
      const selected = rows.find((row) => pidOf(row) === 200);
      assert.ok(selected, "expected the PID 200 row");

      const actions = createProcessRowActions(store, { selection: [selected] }, {
        runCommand: async (command) => { executed.push(command); },
        activeSession: () => ({ id: "session-1", type: "basilisk-debug" }),
      });
      const warnings = await captureWarnings(async () => actions.memoryTrackProcess(undefined));

      assert.deepStrictEqual(warnings, [], "must not warn when a row is selected");
      assert.deepStrictEqual(
        executed,
        ["basilisk.memoryStart"],
        "the selection fallback must reach the real memory-tracking flow",
      );
    } finally {
      provider.dispose();
    }
  });

  test("warns exactly once when there is neither an item nor a selection", async () => {
    const requests: RecordedRequest[] = [];
    const store = storeWith(STUB_PROCESSES, requests);

    const actions = createProcessRowActions(store, { selection: [] });
    const warnings = await captureWarnings(async () => actions.profileProcess(undefined));

    assert.strictEqual(warnings.length, 1, "must warn exactly once");
    assert.strictEqual(
      profilerStarts(requests).length,
      0,
      "must not start profiling without a target",
    );
  });
});
