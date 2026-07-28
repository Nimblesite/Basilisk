// Tests for [EXTACT-REACTIVE-STATE]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-REACTIVE-STATE
//
// Issue #58: the Modules panel must update automatically — no manual Refresh —
// when (a) the server reaches Running, (b) re-analysis completes
// (`basilisk/moduleChanged`), and (c) diagnostics change. Reactivity is
// centralized in the store as the `analysisRevision` signal; panels subscribe
// via a signals effect instead of hand-rolled polling.

import { delay } from "../../timeouts";
import * as assert from "assert";
import * as vscode from "vscode";
import { State, type LanguageClient } from "vscode-languageclient/node";
import { ModuleExplorerProvider, wireReactiveRefresh } from "../../module-explorer";
import { createStore, type Store } from "../../store";

type StateListener = (event: { newState: State; oldState: State }) => void;
type NotificationListener = (value?: unknown) => void;

/** A fake client that exposes its state + notification listeners to the test. */
interface FakeClientHandles {
  client: LanguageClient;
  fireState: (state: State) => void;
  fireNotification: (method: string, value?: unknown) => void;
}

function fakeClient(typeshedStatuses: readonly unknown[] = []): FakeClientHandles {
  const stateListeners: StateListener[] = [];
  const notificationListeners = new Map<string, NotificationListener>();
  const client = {
    isRunning: (): boolean => true,
    onDidChangeState: (listener: StateListener): vscode.Disposable => {
      stateListeners.push(listener);
      return { dispose: (): undefined => undefined };
    },
    onNotification: (method: string, listener: NotificationListener): vscode.Disposable => {
      notificationListeners.set(method, listener);
      return { dispose: (): undefined => undefined };
    },
    sendRequest: async (): Promise<unknown> => ({ modules: [], workspace: undefined }),
    initializeResult: {
      capabilities: { experimental: { basilisk: { typeshedStatuses } } },
    },
  } as unknown as LanguageClient;
  return {
    client,
    fireState: (state: State): void => {
      for (const listener of stateListeners) {
        listener({ newState: state, oldState: State.Starting });
      }
    },
    fireNotification: (method: string, value?: unknown): void => {
      notificationListeners.get(method)?.(value);
    },
  };
}

function storeWithFakeClient(
  typeshedStatuses: readonly unknown[] = [],
): { store: Store; handles: FakeClientHandles } {
  const store = createStore();
  const handles = fakeClient(typeshedStatuses);
  store.setClient({ subscriptions: [] } as unknown as vscode.ExtensionContext, handles.client);
  return { store, handles };
}

/** Avoid colliding with commands owned by the already-activated extension. */
function withStubbedCommands(fn: () => void): void {
  const original = vscode.commands.registerCommand;
  (vscode.commands as { registerCommand: unknown }).registerCommand = (): vscode.Disposable => ({
    dispose: (): undefined => undefined,
  });
  try {
    fn();
  } finally {
    (vscode.commands as { registerCommand: unknown }).registerCommand = original;
  }
}

suite("Centralized analysis reactivity (issue #58)", () => {
  test("analysisRevision bumps when the server reaches Running", () => {
    withStubbedCommands(() => {
      const { store, handles } = storeWithFakeClient();
      const before = store.analysisRevision.value;
      handles.fireState(State.Running);
      assert.ok(
        store.analysisRevision.value > before,
        "reaching Running must bump analysisRevision so panels leave 'Waiting for analysis...'",
      );
    });
  });

  // Tests the client side of [EXTACT-LSP-COMMANDS-MODULE-CHANGED]: the
  // `basilisk/moduleChanged` server notification bumps analysisRevision.
  test("analysisRevision bumps on basilisk/moduleChanged", () => {
    withStubbedCommands(() => {
      const { store, handles } = storeWithFakeClient();
      handles.fireState(State.Running);
      const before = store.analysisRevision.value;
      handles.fireNotification("basilisk/moduleChanged");
      assert.ok(
        store.analysisRevision.value > before,
        "re-analysis completion must bump analysisRevision",
      );
    });
  });

  // [EXTACT-REACTIVE-STATE] / [LSPARCH-CONFIG]: applying a rule-severity change
  // in the configuration editor rewrites every affected diagnostic's severity,
  // so the Modules panel's health rollup is stale the moment the server
  // confirms the change. `basilisk/configurationChanged` must therefore bump
  // analysisRevision — relying on the debounced diagnostics listener alone
  // leaves the panel rendering the PREVIOUS configuration's severities whenever
  // the recheck republishes nothing the client can observe.
  test("analysisRevision bumps on basilisk/configurationChanged", () => {
    withStubbedCommands(() => {
      const { store, handles } = storeWithFakeClient();
      handles.fireState(State.Running);
      const before = store.analysisRevision.value;
      handles.fireNotification("basilisk/configurationChanged", {
        rootUri: "file:///workspace",
        revision: "fnv1a64:0000000000000001",
      });
      assert.ok(
        store.analysisRevision.value > before,
        "an applied configuration change must bump analysisRevision so the "
          + "Modules panel stops showing the previous configuration's severities",
      );
    });
  });

  test("Modules panel refreshes automatically when analysisRevision bumps", async () => {
    const { store } = storeWithFakeClient();
    const provider = new ModuleExplorerProvider(store);
    try {
      wireReactiveRefresh(store, provider);

      const fired = new Promise<void>((resolve) => {
        const sub = provider.onDidChangeTreeData(() => {
          sub.dispose();
          resolve();
        });
      });
      store.bumpAnalysisRevision();
      await fired;
    } finally {
      provider.dispose();
    }
  });

  test("diagnostics changes bump analysisRevision (debounced)", async function () {
    this.timeout(10_000);
    const { store } = storeWithFakeClient();
    const before = store.analysisRevision.value;

    // Drive a REAL diagnostics change through the VS Code API.
    const collection = vscode.languages.createDiagnosticCollection("bsk-issue58-test");
    try {
      collection.set(vscode.Uri.parse("untitled:issue58-test.py"), [
        new vscode.Diagnostic(new vscode.Range(0, 0, 0, 1), "issue58 probe"),
      ]);
      // The bump is debounced — poll briefly.
      const deadline = Date.now() + 5000;
      while (store.analysisRevision.value === before && Date.now() < deadline) {
        await delay(100);
      }
      assert.ok(
        store.analysisRevision.value > before,
        "a diagnostics change must bump analysisRevision per EXTACT-HEALTH-REFRESH",
      );
    } finally {
      collection.dispose();
    }
  });
});

// Typeshed status is a distinct reactive channel from analysisRevision: it
// targets a single root rather than repainting every panel, so it lives in its
// own suite ([EXTACT-REACTIVE-STATE]).
suite("Typeshed status reactivity (issue #58)", () => {
  test("Typeshed status changes refresh only the matching open root", () => {
    withStubbedCommands(() => {
      const { store, handles } = storeWithFakeClient();
      handles.fireState(State.Running);
      store.beginConfigurationLoad("file:///workspace");
      handles.fireNotification("basilisk/typeshedStatusChanged", {
        rootUri: "file:///other",
        status: {
          lifecycle: { kind: "Ready" }, licenseStatus: { kind: "Approved" }, warnings: [],
        },
      });
      assert.strictEqual(store.typeshedStatuses.value.has("file:///other"), true);
      assert.strictEqual(store.configurationEditor.value.refreshRequested, false);
      handles.fireNotification("basilisk/typeshedStatusChanged", {
        rootUri: "file:///workspace",
        status: {
          lifecycle: { kind: "Ready" }, licenseStatus: { kind: "Approved" }, warnings: [],
        },
      });
      assert.strictEqual(
        store.typeshedStatuses.value.get("file:///workspace")?.licenseStatus.kind,
        "Approved",
      );
      assert.strictEqual(store.configurationEditor.value.refreshRequested, true);
    });
  });

  test("Typeshed statuses seed from initialize and invalid notifications cannot replace them", () => {
    withStubbedCommands(() => {
      const initial = {
        rootUri: "file:///workspace",
        status: {
          lifecycle: { kind: "NoSource" }, noSourceReason: "exact unavailable",
          licenseStatus: { kind: "Changed" }, warnings: [],
        },
      };
      const { store, handles } = storeWithFakeClient([initial]);
      handles.fireState(State.Running);
      assert.strictEqual(
        store.typeshedStatuses.value.get("file:///workspace")?.licenseStatus.kind,
        "Changed",
      );
      handles.fireNotification("basilisk/typeshedStatusChanged", {
        rootUri: "file:///workspace", status: { lifecycle: { kind: "invented" } },
      });
      assert.strictEqual(
        store.typeshedStatuses.value.get("file:///workspace")?.noSourceReason,
        "exact unavailable",
      );
    });
  });
});

