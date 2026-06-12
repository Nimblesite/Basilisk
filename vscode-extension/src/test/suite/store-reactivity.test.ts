// Tests for [EXTACT-REACTIVE-STATE]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-REACTIVE-STATE
//
// Issue #58: the Modules panel must update automatically — no manual Refresh —
// when (a) the server reaches Running, (b) re-analysis completes
// (`basilisk/moduleChanged`), and (c) diagnostics change. Reactivity is
// centralized in the store as the `analysisRevision` signal; panels subscribe
// via a signals effect instead of hand-rolled polling.

import * as assert from "assert";
import * as vscode from "vscode";
import { State, type LanguageClient } from "vscode-languageclient/node";
import { ModuleExplorerProvider, wireReactiveRefresh } from "../../module-explorer";
import { createStore, type Store } from "../../store";

type StateListener = (event: { newState: State; oldState: State }) => void;
type NotificationListener = () => void;

/** A fake client that exposes its state + notification listeners to the test. */
interface FakeClientHandles {
  client: LanguageClient;
  fireState: (state: State) => void;
  fireNotification: (method: string) => void;
}

function fakeClient(): FakeClientHandles {
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
    initializeResult: { capabilities: {} },
  } as unknown as LanguageClient;
  return {
    client,
    fireState: (state: State): void => {
      for (const listener of stateListeners) {
        listener({ newState: state, oldState: State.Starting });
      }
    },
    fireNotification: (method: string): void => {
      notificationListeners.get(method)?.();
    },
  };
}

function storeWithFakeClient(): { store: Store; handles: FakeClientHandles } {
  const store = createStore();
  const handles = fakeClient();
  store.setClient({ subscriptions: [] } as unknown as vscode.ExtensionContext, handles.client);
  return { store, handles };
}

suite("Centralized analysis reactivity (issue #58)", () => {
  /**
   * Run fn with command registration stubbed: the store's Running handler
   * registers real client commands, which the already-activated extension
   * owns — the stub keeps this component test from colliding with it.
   */
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
        await new Promise((resolve) => setTimeout(resolve, 100));
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
