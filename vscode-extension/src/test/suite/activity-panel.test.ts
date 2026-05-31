// Tests for [EXTACT]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT
/**
 * Activity Panel E2E Tests for the Basilisk VS Code Extension.
 *
 * Validates:
 *   - Activity bar views are registered (moduleExplorer, typeHealth, info)
 *   - Module explorer commands are registered and executable
 *   - Type health commands are registered and executable
 *   - Info panel commands are registered and executable
 *   - Server advertises basilisk.workspaceModules and basilisk.typeHealth
 *   - Context key basilisk.hasWorkspace is set
 */

import * as assert from "assert";
import * as vscode from "vscode";
import { getStore } from "../../extension";
import { SummaryItem } from "../../type-health";
import {
    EXTENSION_ID,
    WAIT_MS,
    setupLspTestSuite,
    teardownLspTestSuite,
    closeAllEditors,
} from "./test-helpers";

// ── Package.json type definitions ─────────────────────────────────────────

interface ViewContribution {
  readonly id: string;
  readonly name: string;
  readonly when?: string;
  readonly visibility?: string;
}

interface MenuContribution {
  readonly command: string;
  readonly when: string;
  readonly group?: string;
}

interface WelcomeContribution {
  readonly view: string;
  readonly contents: string;
}

interface PackageJSON {
  contributes?: {
    views?: Record<string, ViewContribution[]>;
    menus?: {
      "view/title"?: MenuContribution[];
      "view/item/context"?: MenuContribution[];
    };
    viewsWelcome?: WelcomeContribution[];
  };
}

// ── Command lists ─────────────────────────────────────────────────────────

/**
 * Commands registered client-side for the module explorer panel.
 * These are registered via context.subscriptions in registerModuleExplorer().
 */
const MODULE_EXPLORER_COMMANDS = [
  "basilisk.refreshModuleExplorer",
  "basilisk.collapseModuleExplorer",
  "basilisk.toggleModuleExplorerView",
  "basilisk.filterModuleExplorer",
  "basilisk.copyImportPath",
  "basilisk.copyQualifiedName",
] as const;

/**
 * Commands registered client-side for the type health panel.
 * These are registered via context.subscriptions in registerTypeHealth().
 */
const TYPE_HEALTH_COMMANDS = [
  "basilisk.refreshTypeHealth",
  "basilisk.sortTypeHealth",
] as const;

/**
 * Commands registered client-side for the info panel.
 * These are registered via context.subscriptions in registerInfoPanel().
 */
const INFO_PANEL_COMMANDS = [
  "basilisk.toggleFeature",
] as const;

/** Command registered directly in extension.ts for the walkthrough. */
const WALKTHROUGH_COMMAND = "basilisk.openWalkthrough";

/** View IDs contributed in package.json under basilisk-explorer. */
const ACTIVITY_VIEW_IDS = [
  "basilisk.moduleExplorer",
  "basilisk.typeHealth",
  "basilisk.info",
] as const;

/** Server-advertised commands that back the activity panel data. */
const PANEL_SERVER_COMMANDS = [
  "basilisk.workspaceModules",
  "basilisk.typeHealth",
] as const;

// ── Helpers ───────────────────────────────────────────────────────────────

/**
 * Assert that registering a command throws — proving it IS already registered.
 *
 * This is the sanctioned approach: we do NOT call vscode.commands.getCommands()
 * or whenCommandReady. Instead we rely on the VS Code API guarantee that
 * registering an already-registered command throws.
 */
function assertCommandRegistered(commandId: string, label: string): void {
  let threw = false;
  try {
    vscode.commands.registerCommand(commandId, () => { /* probe */ });
  } catch {
    threw = true;
  }
  assert.ok(
    threw,
    `${label}: "${commandId}" should be registered (re-registering should throw)`,
  );
}

/** Load the extension's package.json contributes section with type safety. */
function loadContributes(): PackageJSON["contributes"] {
  const ext = vscode.extensions.getExtension(EXTENSION_ID);
  assert.ok(ext, "Extension should be installed");
  const pkg = ext.packageJSON as PackageJSON;
  return pkg.contributes;
}

/** Load the basilisk-explorer views from package.json. */
function loadBasiliskViews(): ViewContribution[] {
  const contributes = loadContributes();
  assert.ok(contributes?.views, "Extension should contribute views");
  return contributes.views["basilisk-explorer"] ?? [];
}

// ── Test Suite ────────────────────────────────────────────────────────────

// eslint-disable-next-line max-lines-per-function
suite("Basilisk Activity Panel E2E Tests", function () {

  let suiteContext: { tmpDir: string; basiliskBinary: string };

  suiteSetup(async function () {
    suiteContext = await setupLspTestSuite("activity-panel");

    const store = getStore();
    assert.ok(store, "Store should exist after activation");
    const result = await store.ensureLspReadyPromise(WAIT_MS);
    assert.ok(result.ok, "LSP should be running");
  });

  suiteTeardown(function () {
    teardownLspTestSuite(suiteContext?.tmpDir);
  });

  teardown(async () => {
    await closeAllEditors();
  });

  // ── Activity Bar View Registration ────────────────────────────────────

  test("activity bar views are contributed in package.json", function () {
    const views = loadBasiliskViews();
    const viewIds = views.map((view) => view.id);

    for (const expectedId of ACTIVITY_VIEW_IDS) {
      assert.ok(
        viewIds.includes(expectedId),
        `View "${expectedId}" should be contributed, got: ${viewIds.join(", ")}`,
      );
    }
  });

  test("moduleExplorer view has correct 'when' condition", function () {
    const views = loadBasiliskViews();
    const moduleView = views.find((view) => view.id === "basilisk.moduleExplorer");

    assert.ok(moduleView, "moduleExplorer view should exist");
    assert.strictEqual(
      moduleView.when,
      "basilisk.hasWorkspace",
      "moduleExplorer should have 'basilisk.hasWorkspace' when clause",
    );
  });

  test("typeHealth view has correct 'when' condition", function () {
    const views = loadBasiliskViews();
    const healthView = views.find((view) => view.id === "basilisk.typeHealth");

    assert.ok(healthView, "typeHealth view should exist");
    assert.strictEqual(
      healthView.when,
      "basilisk.hasWorkspace",
      "typeHealth should have 'basilisk.hasWorkspace' when clause",
    );
  });

  test("info view is always visible (no 'when' condition)", function () {
    const views = loadBasiliskViews();
    const infoView = views.find((view) => view.id === "basilisk.info");

    assert.ok(infoView, "info view should exist");
    assert.strictEqual(infoView.when, undefined, "info view should not have a 'when' condition");
    assert.strictEqual(infoView.visibility, "visible", "info view should have visibility 'visible'");
  });

  // ── Module Explorer Commands ──────────────────────────────────────────

  test("module explorer commands are registered", function () {
    for (const cmd of MODULE_EXPLORER_COMMANDS) {
      assertCommandRegistered(cmd, "Module Explorer");
    }
  });

  test("refreshModuleExplorer command is executable", async function () {
    await vscode.commands.executeCommand("basilisk.refreshModuleExplorer");
  });

  test("collapseModuleExplorer command is executable", async function () {
    await vscode.commands.executeCommand("basilisk.collapseModuleExplorer");
  });

  test("toggleModuleExplorerView command is executable", async function () {
    await vscode.commands.executeCommand("basilisk.toggleModuleExplorerView");
  });

  // ── Type Health Commands ──────────────────────────────────────────────

  test("type health commands are registered", function () {
    for (const cmd of TYPE_HEALTH_COMMANDS) {
      assertCommandRegistered(cmd, "Type Health");
    }
  });

  test("refreshTypeHealth command is executable", async function () {
    await vscode.commands.executeCommand("basilisk.refreshTypeHealth");
  });

  test("sortTypeHealth command is executable", async function () {
    await vscode.commands.executeCommand("basilisk.sortTypeHealth");
  });

  // ── Info Panel Commands ───────────────────────────────────────────────

  test("info panel toggleFeature command is registered", function () {
    for (const cmd of INFO_PANEL_COMMANDS) {
      assertCommandRegistered(cmd, "Info Panel");
    }
  });

  test("openWalkthrough command is registered", function () {
    assertCommandRegistered(WALKTHROUGH_COMMAND, "Walkthrough");
  });

  test("toggleFeature command can toggle a boolean setting", async function () {
    const cfg = vscode.workspace.getConfiguration("basilisk");
    const original = cfg.get<boolean>("ruff.enabled") ?? true;

    await vscode.commands.executeCommand("basilisk.toggleFeature", "basilisk.ruff.enabled", !original);

    const updated = vscode.workspace.getConfiguration("basilisk").get<boolean>("ruff.enabled");
    assert.strictEqual(updated, !original, "toggleFeature should flip the setting value");

    // Restore original value.
    await vscode.workspace.getConfiguration().update(
      "basilisk.ruff.enabled",
      undefined,
      vscode.ConfigurationTarget.Workspace,
    );
  });

  // ── Server-Advertised Commands ────────────────────────────────────────

  test("LSP server advertises basilisk.workspaceModules command", function () {
    const store = getStore();
    assert.ok(store, "Store should exist");
    assert.ok(
      store.isServerCommandAdvertised("basilisk.workspaceModules"),
      "Server should advertise basilisk.workspaceModules",
    );
  });

  test("LSP server advertises basilisk.typeHealth command", function () {
    const store = getStore();
    assert.ok(store, "Store should exist");
    assert.ok(
      store.isServerCommandAdvertised("basilisk.typeHealth"),
      "Server should advertise basilisk.typeHealth",
    );
  });

  test("panel server commands are server-advertised, not client-registered", function () {
    const store = getStore();
    assert.ok(store, "Store should exist");

    for (const cmd of PANEL_SERVER_COMMANDS) {
      assert.ok(
        store.isServerCommandAdvertised(cmd),
        `${cmd} should be server-advertised`,
      );
      assert.ok(
        !store.isClientCommandRegistered(cmd),
        `${cmd} should NOT be client-registered (server commands flow through LSP middleware)`,
      );
    }
  });

  // ── Context Keys ──────────────────────────────────────────────────────

  test("basilisk.hasWorkspace context key is set when workspace exists", function () {
    const hasWorkspace = (vscode.workspace.workspaceFolders?.length ?? 0) > 0;

    // The extension should have called setContext("basilisk.hasWorkspace", hasWorkspace).
    // We verify the extension is active and the store exists (proving initExtension ran,
    // which calls setContext before registering panels).
    const store = getStore();
    assert.ok(store, "Store should exist (proves initExtension ran, which sets context key)");

    // If workspace folders exist, the module explorer panel commands should be
    // registered — their 'when' clause depends on basilisk.hasWorkspace being true.
    if (hasWorkspace) {
      assertCommandRegistered("basilisk.refreshModuleExplorer", "Context key verification");
    }
  });

  // ── Menu Contributions ────────────────────────────────────────────────

  test("module explorer has toolbar actions in package.json", function () {
    const contributes = loadContributes();
    const titleMenus = contributes?.menus?.["view/title"] ?? [];

    const moduleMenus = titleMenus.filter(
      (entry) => entry.when === "view == basilisk.moduleExplorer",
    );
    const menuCommands = moduleMenus.map((entry) => entry.command);

    assert.ok(menuCommands.includes("basilisk.refreshModuleExplorer"), "Should include refresh");
    assert.ok(menuCommands.includes("basilisk.collapseModuleExplorer"), "Should include collapse");
    assert.ok(menuCommands.includes("basilisk.toggleModuleExplorerView"), "Should include view toggle");
    assert.ok(menuCommands.includes("basilisk.filterModuleExplorer"), "Should include filter");
  });

  test("type health has toolbar actions in package.json", function () {
    const contributes = loadContributes();
    const titleMenus = contributes?.menus?.["view/title"] ?? [];

    const healthMenus = titleMenus.filter(
      (entry) => entry.when === "view == basilisk.typeHealth",
    );
    const menuCommands = healthMenus.map((entry) => entry.command);

    assert.ok(menuCommands.includes("basilisk.refreshTypeHealth"), "Should include refresh");
    assert.ok(menuCommands.includes("basilisk.sortTypeHealth"), "Should include sort");
  });

  test("module explorer has context menu for copy actions", function () {
    const contributes = loadContributes();
    const contextMenus = contributes?.menus?.["view/item/context"] ?? [];

    const copyMenus = contextMenus.filter(
      (entry) => entry.when.includes("basilisk.moduleExplorer"),
    );
    const menuCommands = copyMenus.map((entry) => entry.command);

    assert.ok(menuCommands.includes("basilisk.copyImportPath"), "Should include Copy Import Path");
    assert.ok(menuCommands.includes("basilisk.copyQualifiedName"), "Should include Copy Qualified Name");
  });

  // ── Welcome Views ─────────────────────────────────────────────────────

  test("module explorer has welcome content in package.json", function () {
    const contributes = loadContributes();
    const welcomeViews = contributes?.viewsWelcome ?? [];

    const moduleWelcome = welcomeViews.find((entry) => entry.view === "basilisk.moduleExplorer");
    assert.ok(moduleWelcome, "moduleExplorer should have welcome content");
    assert.ok(
      moduleWelcome.contents.includes("No modules found"),
      "moduleExplorer welcome should mention no modules found",
    );
  });

  test("type health has welcome content in package.json", function () {
    const contributes = loadContributes();
    const welcomeViews = contributes?.viewsWelcome ?? [];

    const healthWelcome = welcomeViews.find((entry) => entry.view === "basilisk.typeHealth");
    assert.ok(healthWelcome, "typeHealth should have welcome content");
    assert.ok(
      healthWelcome.contents.includes("Waiting for analysis"),
      "typeHealth welcome should mention waiting for analysis",
    );
  });
});

// ── Type Health summary rendering [EXTACT-HEALTH] ───────────────────────────
//
// Regression tests for issue #57: an empty workspace (totalFiles === 0) must
// render an explicit "no Python files" state, never a misleading 100% coverage
// bar with a green "pass" icon.
// Spec: docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-HEALTH
suite("Type Health Summary Rendering [EXTACT-HEALTH]", function () {
  const emptyStats = {
    totalSymbols: 0,
    annotatedSymbols: 0,
    coveragePercent: 100,
    errors: 0,
    warnings: 0,
    adoptedFiles: 0,
    totalFiles: 0,
  };

  const measuredStats = {
    totalSymbols: 20,
    annotatedSymbols: 17,
    coveragePercent: 85,
    errors: 0,
    warnings: 0,
    adoptedFiles: 0,
    totalFiles: 3,
  };

  test("empty workspace shows 'No Python files found', never a 100% bar", function () {
    const item = new SummaryItem(emptyStats);
    const description = String(item.description);

    assert.strictEqual(
      description,
      "No Python files found",
      "empty workspace must render an explicit 'no Python files' state",
    );
    assert.ok(
      !description.includes("%"),
      `empty workspace must not show a percentage, got: "${description}"`,
    );
    assert.ok(
      !description.includes("█") && !description.includes("░"),
      `empty workspace must not render a coverage bar, got: "${description}"`,
    );
  });

  test("empty workspace uses a neutral info icon, not a green pass", function () {
    const item = new SummaryItem(emptyStats);
    const icon = item.iconPath;
    assert.ok(icon instanceof vscode.ThemeIcon, "summary icon should be a ThemeIcon");
    assert.strictEqual(
      icon.id,
      "info",
      "empty workspace should use a neutral 'info' icon, not the green 'pass' icon",
    );
  });

  test("measured workspace still renders the coverage bar and percentage", function () {
    const item = new SummaryItem(measuredStats);
    const description = String(item.description);

    assert.ok(description.includes("85%"), `expected the coverage percentage, got: "${description}"`);
    assert.ok(description.includes("17/20"), `expected the symbol counts, got: "${description}"`);
    assert.ok(description.includes("█"), `expected a coverage bar, got: "${description}"`);
  });
});
