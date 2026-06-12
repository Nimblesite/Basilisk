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
import {
  PythonProcessesProvider,
  memoryTrackProcess,
  profileProcess,
  type ProcessInfo,
} from "../../process-explorer";
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

/** Build a Store whose LSP client returns the given process table. */
function storeWith(processes: readonly ProcessInfo[]): Store {
  const store = createStore();
  const client = {
    isRunning: (): boolean => true,
    onDidChangeState: (): vscode.Disposable => ({ dispose: (): undefined => undefined }),
    sendRequest: async (): Promise<{ processes: readonly ProcessInfo[] }> => ({ processes }),
  } as unknown as LanguageClient;
  store.setClient({ subscriptions: [] } as unknown as vscode.ExtensionContext, client);
  return store;
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
// Clicking the inline flame / database icon on a process row must profile
// THAT row. At runtime VS Code has been observed to invoke the command with
// `item === undefined`; the handler must fall back to the tree view's current
// selection before warning "Select a Python process to profile."

suite("Python Processes Panel — inline action target (issue #79)", () => {
  /** Capture LSP executeCommand calls made through the store's client. */
  function storeCapturing(
    processes: readonly ProcessInfo[],
    calls: { command: string; pid: number | undefined }[],
  ): Store {
    const store = createStore();
    const client = {
      isRunning: (): boolean => true,
      onDidChangeState: (): vscode.Disposable => ({ dispose: (): undefined => undefined }),
      sendRequest: async (
        _method: string,
        params: { command?: string; arguments?: { pid?: number }[] },
      ): Promise<unknown> => {
        // The panel's own row fetch is also an executeCommand — serve it.
        if (params?.command?.endsWith(".processes") === true) {
          return { processes };
        }
        if (params?.command !== undefined) {
          calls.push({ command: params.command, pid: params.arguments?.[0]?.pid });
        }
        return undefined;
      },
    } as unknown as LanguageClient;
    store.setClient({ subscriptions: [] } as unknown as vscode.ExtensionContext, client);
    return store;
  }

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

  test("undefined item falls back to the tree selection and profiles that PID", async () => {
    const calls: { command: string; pid: number | undefined }[] = [];
    const store = storeCapturing(STUB_PROCESSES, calls);
    const provider = new PythonProcessesProvider(store);
    try {
      const rows = await provider.getChildren();
      const selected = rows.find((row) => pidOf(row) === 100);
      assert.ok(selected, "expected the PID 100 row");

      const fakeView = { selection: [selected] } as unknown as vscode.TreeView<vscode.TreeItem>;
      const warnings = await captureWarnings(async () => {
        await profileProcess(store, undefined, fakeView);
      });

      assert.deepStrictEqual(warnings, [], "must not warn when a row is selected");
      const start = calls.find((c) => c.command.includes("profiler"));
      assert.ok(start, `profiler start must be requested, got: ${JSON.stringify(calls)}`);
      assert.strictEqual(start.pid, 100, "must profile the selected row's PID");
    } finally {
      provider.dispose();
    }
  });

  test("memory tracking falls back to the tree selection the same way", async () => {
    const calls: { command: string; pid: number | undefined }[] = [];
    const store = storeCapturing(STUB_PROCESSES, calls);
    const provider = new PythonProcessesProvider(store);
    try {
      const rows = await provider.getChildren();
      const selected = rows.find((row) => pidOf(row) === 200);
      assert.ok(selected, "expected the PID 200 row");

      const fakeView = { selection: [selected] } as unknown as vscode.TreeView<vscode.TreeItem>;
      const warnings = await captureWarnings(async () => {
        await memoryTrackProcess(store, undefined, fakeView);
      });

      assert.deepStrictEqual(warnings, [], "must not warn when a row is selected");
      const start = calls.find((c) => c.command.includes("profiler"));
      assert.ok(start, `profiler start must be requested, got: ${JSON.stringify(calls)}`);
      assert.strictEqual(start.pid, 200, "must memory-track the selected row's PID");
    } finally {
      provider.dispose();
    }
  });

  test("warns only when there is neither an item nor a selection", async () => {
    const calls: { command: string; pid: number | undefined }[] = [];
    const store = storeCapturing(STUB_PROCESSES, calls);

    const fakeView = { selection: [] } as unknown as vscode.TreeView<vscode.TreeItem>;
    const warnings = await captureWarnings(async () => {
      await profileProcess(store, undefined, fakeView);
    });

    assert.strictEqual(warnings.length, 1, "must warn exactly once");
    assert.deepStrictEqual(calls, [], "must not start profiling without a target");
  });
});
