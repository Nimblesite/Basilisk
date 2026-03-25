/**
 * Test Explorer integration for Basilisk.
 *
 * Creates a TestController that listens for `basilisk/testDiscoveryResult`
 * notifications from the LSP server and populates VS Code's native Test
 * Explorer. Test execution flows through `workspace/executeCommand` to the
 * server's `basilisk.runTests` / `basilisk.debugTest` / `basilisk.runTestsCoverage` handlers.
 *
 * Architecture follows LSP-TEST-INTEGRATION-SPEC.md:
 * - Discovery: LSP server parses AST, sends notification
 * - Execution: LSP server spawns pytest subprocess
 * - This module only handles the VS Code UI wiring
 */

import * as vscode from "vscode";
import { type LanguageClient } from "vscode-languageclient/node";
import { applyCoverageDecorations, type LspCoverageResult } from "./coverage-decorations";
import { Logger } from "./logger";
import type { Store } from "./store";

/** Test item kind — mirrors the Rust `TestItemKind` enum. */
type TestItemKind = "file" | "function" | "class" | "method";

/** Shape of a test item received from the LSP server. */
interface LspTestItem {
  name: string;
  id: string;
  file: string;
  line: number;
  kind: TestItemKind;
  children: LspTestItem[];
}

/** Per-test result status. */
type TestStatus = "passed" | "failed" | "skipped" | "error";

/** Per-test result from pytest output parsing. */
interface LspPerTestResult {
  testId: string;
  status: TestStatus;
  message: string;
}

/** Shape of test run results from the LSP server. */
interface LspTestRunResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  passed: boolean;
  perTest: LspPerTestResult[];
}

/** Polling interval (ms) for checking LSP client changes. */
const CLIENT_POLL_INTERVAL_MS = 1000;

/**
 * Register the Basilisk test explorer.
 *
 * Creates a `TestController`, wires up notification listeners, and registers
 * run/debug/coverage profiles. Call this from `activate()` when LSP mode is active.
 */
export function registerTestExplorer(
  context: vscode.ExtensionContext,
  store: Store
): vscode.TestController {
  const controller = vscode.tests.createTestController(
    "basilisk-tests",
    "Basilisk Tests"
  );

  // Run profile: execute tests via pytest.
  controller.createRunProfile(
    "Run",
    vscode.TestRunProfileKind.Run,
    async (request, token) => runTests({ controller, store, request, token, debug: false }),
    true
  );

  // Debug profile: start debug session targeting a test.
  controller.createRunProfile(
    "Debug",
    vscode.TestRunProfileKind.Debug,
    async (request, token) => runTests({ controller, store, request, token, debug: true }),
    false
  );

  // Coverage profile: run tests with pytest-cov and show gutter decorations.
  controller.createRunProfile(
    "Coverage",
    vscode.TestRunProfileKind.Coverage,
    async (request, token) => runTests({ controller, store, request, token, debug: false, coverage: true }),
    false
  );

  // Resolve handler: when the user expands a test item, discover its children.
  controller.resolveHandler = async (item) => {
    if (item === undefined) {
      // Root resolve — request full workspace discovery.
      await requestDiscovery(store);
      return;
    }
    // Individual items are already populated from notifications.
  };

  // Listen for discovery notifications from the LSP server.
  wireNotificationListener(controller, store);

  Logger.info("Test explorer registered");
  return controller;
}

/** Wire up the `basilisk/testDiscoveryResult` notification listener. */
function wireNotificationListener(
  controller: vscode.TestController,
  store: Store
): void {
  // Re-wire whenever the LSP client changes (restart, reconnect).
  let currentClient: LanguageClient | undefined;

  function checkClient(): void {
    const client = store.client.value;
    if (client === currentClient) { return; }
    currentClient = client;

    if (client === undefined) { return; }

    client.onNotification(
      "basilisk/testDiscoveryResult",
      (params: { items: LspTestItem[] }) => {
        Logger.info(`Test discovery: received ${params.items.length} item(s)`);
        populateTestItems(controller, params.items);
      }
    );

    client.onNotification(
      "basilisk/coverageResult",
      (params: LspCoverageResult) => {
        Logger.info(`Coverage: received ${params.files.length} file(s), ${params.totalPct.toFixed(1)}%`);
        applyCoverageDecorations(params);
      }
    );

    // Request discovery now that the notification handler is wired up.
    // The initial notification from `initialized` may have been sent
    // before this handler was registered, so we request a fresh one.
    if (client.isRunning()) {
      requestDiscovery(store).catch(() => {});
    }
  }

  // Check immediately and on state changes.
  checkClient();
  // Poll on a short interval since store.client is a signal but we can't
  // subscribe to it directly from here. The effect runs in lsp-client.ts.
  const interval = setInterval(checkClient, CLIENT_POLL_INTERVAL_MS);
  const disposable = new vscode.Disposable(() => { clearInterval(interval); });
  const originalDispose = controller.dispose.bind(controller);
  controller.dispose = () => {
    disposable.dispose();
    originalDispose();
  };
}

/** Request test discovery from the LSP server. */
async function requestDiscovery(store: Store): Promise<void> {
  const client = store.client.value;
  if (client?.isRunning() !== true) { return; }

  try {
    await client.sendRequest("workspace/executeCommand", {
      command: "basilisk.discoverTests",
      arguments: [],
    });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Test discovery request failed: ${msg}`);
  }
}

/**
 * Populate the TestController with items from an LSP discovery notification.
 *
 * Replaces the entire tree — items not in the new list are removed.
 */
function populateTestItems(
  controller: vscode.TestController,
  items: LspTestItem[]
): void {
  // Track which top-level IDs we see so we can prune stale entries.
  const seenIds = new Set<string>();

  for (const item of items) {
    seenIds.add(item.id);
    upsertTestItem(controller, controller.items, item);
  }

  // Remove stale top-level items.
  controller.items.forEach((existing) => {
    if (!seenIds.has(existing.id)) {
      controller.items.delete(existing.id);
    }
  });
}

/** Create or update a test item and its children recursively. */
function upsertTestItem(
  controller: vscode.TestController,
  collection: vscode.TestItemCollection,
  lspItem: LspTestItem
): vscode.TestItem {
  const uri = vscode.Uri.file(lspItem.file);
  const range = new vscode.Range(
    new vscode.Position(lspItem.line, 0),
    new vscode.Position(lspItem.line, 0)
  );

  // Always recreate the item to update uri (read-only after creation).
  let testItem = collection.get(lspItem.id);
  if (testItem !== undefined) {
    collection.delete(lspItem.id);
  }
  testItem = controller.createTestItem(lspItem.id, lspItem.name, uri);
  testItem.range = range;
  collection.add(testItem);

  // Recursively populate children.
  const childIds = new Set<string>();
  for (const child of lspItem.children) {
    childIds.add(child.id);
    upsertTestItem(controller, testItem.children, child);
  }

  // Remove stale children.
  testItem.children.forEach((existing) => {
    if (!childIds.has(existing.id)) {
      testItem.children.delete(existing.id);
    }
  });

  return testItem;
}

/** Arguments for running tests. */
interface RunTestsArgs {
  controller: vscode.TestController;
  store: Store;
  request: vscode.TestRunRequest;
  token: vscode.CancellationToken;
  debug: boolean;
  coverage?: boolean;
}

/**
 * Run or debug the requested tests.
 *
 * For run mode, sends `basilisk.runTests` to the LSP server.
 * For debug mode, sends `basilisk.debugTest` and starts a VS Code debug session.
 */
async function runTests(args: RunTestsArgs): Promise<void> {
  const { controller, store, request, token, debug, coverage = false } = args;
  const run = controller.createTestRun(request);
  const client = store.client.value;

  if (client?.isRunning() !== true) {
    run.end();
    return;
  }

  // Collect test IDs to run.
  const testIds = collectTestIds(request, controller);
  if (testIds.length === 0) {
    run.end();
    return;
  }

  // Mark all as started.
  for (const id of testIds) {
    const item = findTestItem(controller, id);
    if (item !== undefined) { run.started(item); }
  }

  if (token.isCancellationRequested) {
    run.end();
    return;
  }

  try {
    if (debug) {
      await runDebugTest({ client, store, run, controller, testId: testIds[0] });
    } else {
      await runNormalTests({ client, run, controller, testIds, coverage });
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    Logger.error(`Test run failed: ${msg}`);
    for (const id of testIds) {
      const item = findTestItem(controller, id);
      if (item !== undefined) {
        run.errored(item, new vscode.TestMessage(msg));
      }
    }
  }

  run.end();
}

/** Arguments for running normal (non-debug) tests. */
interface RunNormalTestsArgs {
  client: LanguageClient;
  run: vscode.TestRun;
  controller: vscode.TestController;
  testIds: string[];
  coverage: boolean;
}

/** Execute tests normally (not debug). */
async function runNormalTests(args: RunNormalTestsArgs): Promise<void> {
  const { client, run, controller, testIds, coverage } = args;
  const command = coverage ? "basilisk.runTestsCoverage" : "basilisk.runTests";
  const result = await client.sendRequest("workspace/executeCommand", {
    command,
    arguments: [{ testIds }],
  });

  if (result === null) { return; }

  const typed = result as LspTestRunResult;
  // Use per-test results when available, fall back to bulk pass/fail.
  if (typed.perTest.length > 0) {
    applyPerTestResults(run, controller, typed.perTest);
  } else {
    // Fallback: mark all tests based on overall pass/fail.
    for (const id of testIds) {
      const item = findTestItem(controller, id);
      if (item === undefined) { continue; }

      if (typed.passed) {
        run.passed(item);
      } else {
        const message = new vscode.TestMessage(
          typed.stderr !== "" ? typed.stderr : typed.stdout
        );
        run.failed(item, message);
      }
    }
  }
}

/** Arguments for running a debug test. */
interface RunDebugTestArgs {
  client: LanguageClient;
  store: Store;
  run: vscode.TestRun;
  controller: vscode.TestController;
  testId: string;
}

/** Start a debug session targeting a specific test. */
async function runDebugTest(args: RunDebugTestArgs): Promise<void> {
  const { client, run, controller, testId } = args;
  const rawResult = await client.sendRequest("workspace/executeCommand", {
    command: "basilisk.debugTest",
    arguments: [{ testId }],
  });

  if (rawResult === null) {
    const item = findTestItem(controller, testId);
    if (item !== undefined) {
      run.errored(item, new vscode.TestMessage("Failed to start debug session"));
    }
    return;
  }

  const result = rawResult as { host: string; port: number };

  // Start a VS Code debug session connecting to the debugpy proxy.
  const debugStarted = await vscode.debug.startDebugging(
    vscode.workspace.workspaceFolders?.[0],
    {
      name: `Debug Test: ${testId}`,
      type: "basilisk-debug",
      request: "attach",
      connect: {
        host: result.host,
        port: result.port,
      },
    }
  );

  const item = findTestItem(controller, testId);
  if (item !== undefined) {
    if (debugStarted) {
      // Debug session started — result will come from debug adapter.
      // Mark as passed for now; the user will see failures in the debugger.
      run.passed(item);
    } else {
      run.errored(item, new vscode.TestMessage("Debug session failed to start"));
    }
  }
}

/** Apply per-test results to the test run. */
function applyPerTestResults(
  run: vscode.TestRun,
  controller: vscode.TestController,
  perTest: LspPerTestResult[]
): void {
  for (const result of perTest) {
    const item = findTestItem(controller, result.testId);
    if (item === undefined) { continue; }

    switch (result.status) {
      case "passed":
        run.passed(item);
        break;
      case "failed":
        run.failed(item, new vscode.TestMessage(result.message === "" ? "Test failed" : result.message));
        break;
      case "skipped":
        run.skipped(item);
        break;
      case "error":
        run.errored(item, new vscode.TestMessage(result.message === "" ? "Test errored" : result.message));
        break;
    }
  }
}

/** Collect test IDs from the run request. */
function collectTestIds(
  request: vscode.TestRunRequest,
  controller: vscode.TestController
): string[] {
  if (request.include !== undefined && request.include.length > 0) {
    return request.include.map((item) => item.id);
  }

  // No specific items — run all.
  const ids: string[] = [];
  controller.items.forEach((item) => { ids.push(item.id); });
  return ids;
}

/** Find a test item by ID anywhere in the tree. */
function findTestItem(
  controller: vscode.TestController,
  id: string
): vscode.TestItem | undefined {
  return findInCollection(controller.items, id);
}

/** Recursively search a TestItemCollection for an item by ID. */
function findInCollection(
  collection: vscode.TestItemCollection,
  id: string
): vscode.TestItem | undefined {
  const direct = collection.get(id);
  if (direct !== undefined) { return direct; }

  let found: vscode.TestItem | undefined;
  collection.forEach((item) => {
    if (found !== undefined) { return; }
    found = findInCollection(item.children, id);
  });
  return found;
}
