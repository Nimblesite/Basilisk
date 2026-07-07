// Tests [EXTACT-MODULES-HEADER] loading state (issue #144).
// See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-MODULES-HEADER
//
// Regression for issue #144: while the analyzer is still starting up or its
// initial background workspace scan is incomplete, basilisk.workspaceModules
// legitimately answers `{ modules: [], workspace: { totalFiles: 0, … } }`.
// The Modules panel must render a loading affordance for that window — it must
// NEVER claim "No Python files found" until the scan has actually finished.
// Per CLAUDE.md the real provider is driven end-to-end: a stubbed LSP response
// is fed through a real store and the tree view's native message chrome is
// asserted.

import * as assert from "assert";
import type * as vscode from "vscode";
import { type LanguageClient } from "vscode-languageclient/node";
import { ModuleExplorerProvider } from "../../module-explorer";
import { createStore, type Store } from "../../store";

/** The panel's terminal empty-state — only valid once the scan has finished. */
const EMPTY_STATE_MESSAGE = "No Python files found";

/** Build a Store whose running LSP client answers workspaceModules with `payload`. */
function storeAnswering(payload: unknown): Store {
  const store = createStore();
  const client = {
    isRunning: (): boolean => true,
    onDidChangeState: (): vscode.Disposable => ({ dispose: (): undefined => undefined }),
    sendRequest: async (): Promise<unknown> => payload,
  } as unknown as LanguageClient;
  store.setClient({ subscriptions: [] } as unknown as vscode.ExtensionContext, client);
  return store;
}

/** Minimal tree-view stub capturing the message/badge chrome the provider drives. */
interface ChromeCapture {
  message: string | undefined;
  badge: vscode.ViewBadge | undefined;
}

function bindChrome(provider: ModuleExplorerProvider): ChromeCapture {
  const view: ChromeCapture = { message: undefined, badge: undefined };
  provider.setTreeView(view as unknown as Parameters<ModuleExplorerProvider["setTreeView"]>[0]);
  return view;
}

suite("Modules panel loading state [EXTACT-MODULES-HEADER] (#144)", () => {

  test("mid-scan zero-file response renders a loading message, never 'No Python files found' (#144)", async () => {
    // The server is Running but its initial background workspace scan has not
    // finished (init.rs run_workspace_scan): the module list is still empty
    // and the workspace rollup reports zero files with scanComplete: false.
    const store = storeAnswering({
      modules: [],
      workspace: {
        typeCheckingEnabled: true,
        totalSymbols: 0,
        annotatedSymbols: 0,
        coveragePercent: 100,
        errors: 0,
        warnings: 0,
        adoptedFiles: 0,
        totalFiles: 0,
        scanComplete: false,
      },
    });
    const provider = new ModuleExplorerProvider(store);
    const chrome = bindChrome(provider);
    try {
      await provider.getChildren();
      assert.notStrictEqual(
        chrome.message,
        EMPTY_STATE_MESSAGE,
        "the panel must never claim zero Python files while the initial scan is incomplete (#144)",
      );
      assert.ok(
        chrome.message?.includes("Analyzing"),
        `mid-scan the panel must show a loading message, got: "${String(chrome.message)}"`,
      );
    } finally {
      provider.dispose();
    }
  });

  test("before the server runs (no stats yet) the panel shows a loading message, not silence (#144)", async () => {
    // lspState idle/starting: there is no client to fetch from, so no workspace
    // stats exist. The panel must still show the loading affordance.
    const provider = new ModuleExplorerProvider(createStore());
    const chrome = bindChrome(provider);
    try {
      await provider.getChildren();
      assert.notStrictEqual(chrome.message, EMPTY_STATE_MESSAGE, "no empty-state before the analyzer ran");
      assert.ok(
        chrome.message?.includes("Analyzing"),
        `while the analyzer is starting the panel must show a loading message, got: "${String(chrome.message)}"`,
      );
    } finally {
      provider.dispose();
    }
  });

  test("a finished scan with genuinely zero files still renders 'No Python files found' (#57)", async () => {
    // The dual guarantee: once the scan HAS finished and there really are no
    // Python files, the explicit empty-state (issue #57) must survive.
    const store = storeAnswering({
      modules: [],
      workspace: {
        typeCheckingEnabled: true,
        totalSymbols: 0,
        annotatedSymbols: 0,
        coveragePercent: 100,
        errors: 0,
        warnings: 0,
        adoptedFiles: 0,
        totalFiles: 0,
        scanComplete: true,
      },
    });
    const provider = new ModuleExplorerProvider(store);
    const chrome = bindChrome(provider);
    try {
      await provider.getChildren();
      assert.strictEqual(
        chrome.message,
        EMPTY_STATE_MESSAGE,
        "a completed scan of a truly empty workspace keeps the explicit #57 empty-state",
      );
    } finally {
      provider.dispose();
    }
  });
});
