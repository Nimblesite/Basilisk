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
import { ProcessDecorationProvider, PythonProcessesProvider, type ProcessInfo } from "../../process-explorer";
import { createProcessRowActions, memoryTrackRoute } from "../../process-launch";
import { createStore, type Store } from "../../store";

const MB = 1024 * 1024;

/** A representative process table covering launchers, users, and versions. */
const STUB_PROCESSES: readonly ProcessInfo[] = [
  {
    pid: 100, ppid: 1, name: "python3.12", interpreterPath: "/usr/bin/python3.12",
    script: "/app/web.py", pythonVersion: "3.12.1", cpuPercent: 5, memoryBytes: 50 * MB,
    runtimeSecs: 10, user: "alice", requiresElevation: false,
    inWorkspace: true, launcher: null, debuggable: true, undebuggableReason: null,
  },
  {
    pid: 200, ppid: 1, name: "python3.11", interpreterPath: "/usr/bin/python3.11",
    script: "/app/worker.py", pythonVersion: "3.11.7", cpuPercent: 42, memoryBytes: 10 * MB,
    runtimeSecs: 99, user: "bob", requiresElevation: true,
    inWorkspace: false, launcher: null, debuggable: true, undebuggableReason: null,
  },
  {
    pid: 300, ppid: 200, name: "python3.12", interpreterPath: "/usr/bin/python3.12",
    script: "/app/uvicorn.py", pythonVersion: "3.12.1", cpuPercent: 1, memoryBytes: 99 * MB,
    runtimeSecs: 5, user: "alice", requiresElevation: false,
    inWorkspace: false, launcher: "uvicorn", debuggable: true, undebuggableReason: null,
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

// ── Display-cue helpers ([PROFILE-PROCESSES-DISPLAY]) ──────────────────────

/** The vscode.ThemeColor / ThemeIcon `id` a row or decoration resolves to. */
function colorId(value: { id?: string } | undefined): string | undefined {
  return value?.id;
}
function iconId(item: vscode.TreeItem): string | undefined {
  return (item.iconPath as vscode.ThemeIcon | undefined)?.id;
}
function resourceUri(item: vscode.TreeItem): vscode.Uri | undefined {
  return (item as unknown as { resourceUri?: vscode.Uri }).resourceUri;
}
/** The row's tooltip narrowed to its string form (rowTooltip always returns one). */
function tooltipText(item: vscode.TreeItem): string {
  return typeof item.tooltip === "string" ? item.tooltip : "";
}

/** Drop the pinned "Run & …(Current File)" launch-action rows from a root listing. */
function processRows(rows: vscode.TreeItem[]): vscode.TreeItem[] {
  return rows.filter((r) => r.contextValue !== "launchAction");
}
/** Just the pinned launch-action rows. */
function actionRows(rows: vscode.TreeItem[]): vscode.TreeItem[] {
  return rows.filter((r) => r.contextValue === "launchAction");
}

/** A debugger-machinery row: listed, but non-debuggable (the 🚫 / grey / sunk case). */
const MACHINERY: ProcessInfo = {
  pid: 900, ppid: 1, name: "python3.12", interpreterPath: "/usr/bin/python3.12",
  script: null, pythonVersion: "3.12.1", cpuPercent: 99, memoryBytes: 5 * MB,
  runtimeSecs: 3, user: "alice", requiresElevation: false,
  inWorkspace: false, launcher: null, debuggable: false,
  undebuggableReason: "debugger machinery",
};

suite("Python Processes Panel", () => {
  let provider: PythonProcessesProvider;

  teardown(() => {
    provider.dispose();
  });

  test("lists every process sorted by CPU descending by default", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const rows = processRows(await provider.getChildren());
    assert.deepStrictEqual(rows.map(pidOf), [200, 100, 300], "CPU 42 > 5 > 1");
  });

  test("each row carries its PID so inline Profile starts with no input box", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const rows = processRows(await provider.getChildren());
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
    const rows = processRows(await provider.getChildren());
    assert.deepStrictEqual(rows.map(pidOf), [300, 100, 200], "memory 99 > 50 > 10 MB");
  });

  test("group by Python version buckets processes under collapsible headers", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    provider.cycleGroupMode(); // none → version
    const groups = processRows(await provider.getChildren());
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
    let rows = processRows(await provider.getChildren());
    assert.deepStrictEqual(rows.map(pidOf), [200], "only worker.py matches");

    provider.setFilter("300");
    rows = processRows(await provider.getChildren());
    assert.deepStrictEqual(rows.map(pidOf), [300], "PID substring matches");
  });

  // procexp-2: VS Code shows the "No Python processes running" welcome whenever
  // getChildren returns []. When a filter hides a NON-empty process list, the
  // tree must NOT be empty — it must say processes are running but filtered
  // (the pinned launch rows stay too).
  test("a filter that hides every running process shows an honest placeholder, not 'no processes' (procexp-2)", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    provider.setFilter("nonexistent-zzz");
    const nonAction = processRows(await provider.getChildren());
    assert.strictEqual(nonAction.length, 1, "must return a placeholder row, not an empty list that triggers the welcome");
    assert.strictEqual(nonAction[0].contextValue, "processesMessage", "the row is a non-process placeholder");
    const label = labelText(nonAction[0]);
    assert.ok(
      label.includes("nonexistent-zzz") && label.includes("3 running"),
      `the placeholder must explain the filter hid running processes: ${label}`,
    );
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

// Tests for [PROFILE-PROCESSES-DISPLAY] / [PROFILE-PROCESSES-SCOPE]: the panel
// shows EVERY process (zero filters) and renders cues — launcher chips, a green
// workspace row, and a 🚫 / greyed / sunk row for anything it can't profile.
suite("Python Processes Panel — zero-filter display cues", () => {
  let provider: PythonProcessesProvider;
  teardown(() => { provider.dispose(); });

  test("launchers are always listed and carry a framework chip (zero filters)", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const rows = await provider.getChildren();
    const uvicorn = rows.find((r) => pidOf(r) === 300);
    assert.ok(uvicorn, "the uvicorn launcher must always be listed — nothing is hidden");
    assert.ok(
      String(uvicorn.description).includes("[uvicorn]"),
      `the launcher framework must render as a chip: ${String(uvicorn.description)}`,
    );
  });

  test("a workspace process resolves to a green decoration; an outside one does not", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const decorations = new ProcessDecorationProvider(provider);
    try {
      const rows = await provider.getChildren();
      const inside = rows.find((r) => pidOf(r) === 100); // inWorkspace: true
      const outside = rows.find((r) => pidOf(r) === 300); // inWorkspace: false, debuggable
      assert.ok(inside && outside, "both the workspace and outside rows must exist");
      const insideUri = resourceUri(inside);
      const outsideUri = resourceUri(outside);
      assert.ok(insideUri && outsideUri, "process rows must carry a resourceUri for decoration");

      const insideDeco = decorations.provideFileDecoration(insideUri);
      assert.strictEqual(colorId(insideDeco?.color), "charts.green", "a workspace row must be green");
      const outsideDeco = decorations.provideFileDecoration(outsideUri);
      assert.strictEqual(outsideDeco, undefined, "a non-workspace debuggable row keeps the default colour");

      // A non-process URI is ignored, and a tree refresh re-fires decorations.
      assert.strictEqual(
        decorations.provideFileDecoration(vscode.Uri.file("/tmp/unrelated")),
        undefined,
        "URIs from other schemes are not decorated",
      );
      let fired = false;
      const sub = decorations.onDidChangeFileDecorations(() => { fired = true; });
      provider.refresh();
      sub.dispose();
      assert.ok(fired, "a tree refresh must re-fire decorations so colours never go stale");
    } finally {
      decorations.dispose();
    }
  });

  test("a non-debuggable process is 🚫-marked, greyed, and sorted to the bottom", async () => {
    // MACHINERY has the highest CPU (99%) but must still sink below the others.
    provider = new PythonProcessesProvider(storeWith([MACHINERY, ...STUB_PROCESSES]));
    const decorations = new ProcessDecorationProvider(provider);
    try {
      const rows = await provider.getChildren();
      assert.strictEqual(
        pidOf(rows[rows.length - 1]),
        900,
        "the non-debuggable row sinks to the bottom despite the highest CPU",
      );
      const machineryRow = rows.find((r) => pidOf(r) === 900);
      assert.ok(machineryRow, "the machinery process must still be LISTED, not hidden");
      assert.ok(
        labelText(machineryRow).startsWith("🚫"),
        `a non-debuggable row must be prefixed with 🚫: ${labelText(machineryRow)}`,
      );
      assert.strictEqual(iconId(machineryRow), "circle-slash", "non-debuggable icon is circle-slash");

      const machineryUri = resourceUri(machineryRow);
      assert.ok(machineryUri, "the machinery row must carry a resourceUri");
      const deco = decorations.provideFileDecoration(machineryUri);
      assert.strictEqual(colorId(deco?.color), "disabledForeground", "a non-debuggable row is greyed");
      assert.ok(
        tooltipText(machineryRow).includes("debugger machinery"),
        `the tooltip must explain why it can't be profiled: ${tooltipText(machineryRow)}`,
      );
    } finally {
      decorations.dispose();
    }
  });
});

// Icons, decoration precedence, tooltip detail, and within-group sinking —
// [PROFILE-PROCESSES-DISPLAY] (R4, R5, R6, R8).
suite("Python Processes Panel — display cues: icons, precedence, tooltip, grouping", () => {
  let provider: PythonProcessesProvider;
  teardown(() => { provider.dispose(); });

  test("each process state renders its own info icon (R4)", async () => {
    const plain: ProcessInfo = { ...STUB_PROCESSES[0], pid: 111, inWorkspace: false };
    provider = new PythonProcessesProvider(storeWith([MACHINERY, plain, ...STUB_PROCESSES]));
    provider.setActiveProfilingPid(100); // mark PID 100 as actively profiled
    const rows = await provider.getChildren();
    const icons = new Map(rows.map((r) => [pidOf(r), iconId(r)]));
    assert.strictEqual(icons.get(100), "flame", "the actively-profiled row shows the flame");
    assert.strictEqual(icons.get(200), "lock", "an elevation row stays debuggable but shows the lock");
    assert.strictEqual(icons.get(300), "rocket", "a launcher row shows the rocket");
    assert.strictEqual(icons.get(111), "vm-running", "a plain interpreter shows the running-VM glyph");
    assert.strictEqual(icons.get(900), "circle-slash", "a non-debuggable row shows circle-slash");
  });

  test("greying wins over green for a non-debuggable workspace process (R5 > R6)", async () => {
    const wsMachinery: ProcessInfo = { ...MACHINERY, inWorkspace: true };
    provider = new PythonProcessesProvider(storeWith([wsMachinery]));
    const decorations = new ProcessDecorationProvider(provider);
    try {
      const row = processRows(await provider.getChildren()).find((r) => pidOf(r) === 900);
      assert.ok(row, "the process row must exist");
      const uri = resourceUri(row);
      assert.ok(uri, "the row must carry a resourceUri");
      const deco = decorations.provideFileDecoration(uri);
      assert.strictEqual(
        colorId(deco?.color),
        "disabledForeground",
        "a workspace process you can't debug must be greyed, not green",
      );
    } finally {
      decorations.dispose();
    }
  });

  test("the tooltip surfaces every resolved detail (R8)", async () => {
    const rich: ProcessInfo = {
      pid: 555, ppid: 1, name: "python3.12", interpreterPath: "/usr/bin/python3.12",
      script: "/app/svc.py", pythonVersion: "3.12.1", cpuPercent: 7, memoryBytes: 12 * MB,
      runtimeSecs: 65, user: "carol", requiresElevation: false,
      inWorkspace: true, launcher: "gunicorn", debuggable: true, undebuggableReason: null,
    };
    provider = new PythonProcessesProvider(storeWith([rich]));
    const row = processRows(await provider.getChildren()).find((r) => pidOf(r) === 555);
    assert.ok(row, "the process row must exist");
    const tip = tooltipText(row);
    for (const needle of [
      "PID 555", "Interpreter: /usr/bin/python3.12", "Script: /app/svc.py",
      "Python: 3.12.1", "Runtime:", "User: carol", "Launcher: gunicorn", "Workspace",
    ]) {
      assert.ok(tip.includes(needle), `tooltip must surface "${needle}": ${tip}`);
    }
  });

  test("when grouped, a non-debuggable process sinks within its group (R5)", async () => {
    const machinery: ProcessInfo = { ...MACHINERY, pythonVersion: "3.12.1" }; // shares 100 & 300's group
    provider = new PythonProcessesProvider(storeWith([machinery, ...STUB_PROCESSES]));
    provider.cycleGroupMode(); // none → version
    const groups = await provider.getChildren();
    const twelve = groups.find((g) => labelText(g) === "3.12.1");
    assert.ok(twelve, "the 3.12.1 group must exist");
    const members = await provider.getChildren(twelve);
    assert.deepStrictEqual(
      members.map(pidOf),
      [100, 300, 900],
      "debuggable rows first (CPU-ordered), the non-debuggable one sinks last within the group",
    );
  });
});

// The big "Run & …(Current File)" buttons can't live in viewsWelcome once the
// tree is populated (VS Code renders welcome only for an EMPTY view), so they are
// pinned as rows at the top — gated per activity. [PROFILE-PROCESSES-LAUNCH-FILE]
// / [PROFILE-PROCESSES-REACTIVE].
suite("Python Processes Panel — pinned launch buttons", () => {
  let provider: PythonProcessesProvider;
  teardown(() => { provider.dispose(); });

  function commandsOf(rows: vscode.TreeItem[]): (string | undefined)[] {
    return actionRows(rows).map((r) => r.command?.command);
  }

  test("the current-file launches are pinned above the process rows even when a process is listed", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    const rows = await provider.getChildren();
    assert.deepStrictEqual(
      commandsOf(rows),
      ["basilisk.profileCurrentFileCpu", "basilisk.trackMemoryCurrentFile"],
      "both launches must be pinned, CPU then memory",
    );
    assert.strictEqual(rows[0].contextValue, "launchAction", "a launch row is first");
    assert.ok(processRows(rows).length > 0, "the process rows still follow the launches");
  });

  test("a busy metric hides ITS launch row but leaves the other (both: CPU during memory, memory during CPU)", async () => {
    const store = storeWith(STUB_PROCESSES);
    provider = new PythonProcessesProvider(store);

    store.profilerActive(4242, "sess-cpu"); // CPU busy
    assert.deepStrictEqual(
      commandsOf(await provider.getChildren()),
      ["basilisk.trackMemoryCurrentFile"],
      "while CPU profiles, the CPU launch is hidden but the memory launch remains",
    );

    store.profilerStopped();
    store.memoryTrackingActive("sess-mem"); // memory busy
    assert.deepStrictEqual(
      commandsOf(await provider.getChildren()),
      ["basilisk.profileCurrentFileCpu"],
      "while memory tracks, the memory launch is hidden but the CPU launch remains",
    );
    store.memoryTrackingStopped();
  });

  test("with no processes the tree is empty so the viewsWelcome big buttons render", async () => {
    provider = new PythonProcessesProvider(storeWith([]));
    assert.deepStrictEqual(
      await provider.getChildren(),
      [],
      "an empty process list defers to the welcome buttons rather than pinning rows",
    );
  });
});

// Memory tracking can only target the active Basilisk debuggee, so the panel
// reveals the inline Track Memory action on that row alone and warns elsewhere —
// answering "why not grey it out beforehand?" ([PROFILE-PROCESSES-LAUNCH]).
suite("Python Processes Panel — Track Memory is debuggee-only", () => {
  let provider: PythonProcessesProvider;
  teardown(() => { provider.dispose(); });

  test("only the active-debuggee row carries the Track-Memory-enabling contextValue", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    provider.setActiveDebuggeePid(100); // PID 100 is the active debuggee
    const rows = processRows(await provider.getChildren());
    function ctxOf(pid: number): string | undefined {
      return rows.find((r) => pidOf(r) === pid)?.contextValue;
    }
    assert.strictEqual(ctxOf(100), "pythonProcessDebuggee", "the debuggee row enables Track Memory");
    assert.strictEqual(ctxOf(200), "pythonProcessElevated", "an external (elevated) row does not");
    assert.strictEqual(ctxOf(300), "pythonProcess", "an external launcher row does not");
  });

  test("non-debuggee rows warn that memory tracking is unavailable here; the debuggee does not", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    provider.setActiveDebuggeePid(100);
    const rows = processRows(await provider.getChildren());
    const debuggee = rows.find((r) => pidOf(r) === 100);
    const external = rows.find((r) => pidOf(r) === 300);
    assert.ok(debuggee && external, "both rows must exist");
    assert.ok(
      !tooltipText(debuggee).includes("Memory tracking needs"),
      "the debuggee row offers tracking, so it shows no caveat",
    );
    assert.ok(
      tooltipText(external).includes("Memory tracking needs"),
      `a non-debuggee row must warn memory tracking is unavailable: ${tooltipText(external)}`,
    );
  });

  test("with no active debuggee, no row enables Track Memory", async () => {
    provider = new PythonProcessesProvider(storeWith(STUB_PROCESSES));
    provider.setActiveDebuggeePid(undefined);
    const rows = processRows(await provider.getChildren());
    assert.ok(
      !rows.some((r) => r.contextValue === "pythonProcessDebuggee"),
      "no row may offer Track Memory when nothing runs under Basilisk",
    );
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
