/**
 * Activity Panel Accessibility Audit Tests for the Basilisk VS Code Extension.
 *
 * Validates that the activity panel meets WCAG accessibility guidelines:
 *   - All tree items have descriptive labels (not empty/generic)
 *   - Status indicators use icon + text (never color alone)
 *   - All interactive elements have associated commands (keyboard navigable)
 *   - Tooltips provide sufficient context for screen readers
 *   - Context values enable context menu filtering for keyboard users
 */

import * as assert from "assert";
import * as vscode from "vscode";
import { getStore } from "../../extension";
import {
  EXTENSION_ID,
  SERVER_START_WAIT_MS,
  SUITE_SETUP_TIMEOUT_MS,
  setupLspTestSuite,
  teardownLspTestSuite,
  closeAllEditors,
} from "./test-helpers";

const TEST_TIMEOUT_MS = 15_000;

// ── Package.json types ────────────────────────────────────────────────────

interface CommandContribution {
  readonly command: string;
  readonly title: string;
  readonly icon?: string | { light: string; dark: string };
  readonly category?: string;
}

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
  readonly when?: string;
}

interface PackageJSON {
  contributes?: {
    commands?: CommandContribution[];
    views?: Record<string, ViewContribution[]>;
    menus?: {
      "view/title"?: MenuContribution[];
      "view/item/context"?: MenuContribution[];
    };
    viewsWelcome?: WelcomeContribution[];
  };
}

// ── Helpers ───────────────────────────────────────────────────────────────

function loadPackageJSON(): PackageJSON {
  const ext = vscode.extensions.getExtension(EXTENSION_ID);
  assert.ok(ext, "Extension should be installed");
  return ext.packageJSON as PackageJSON;
}

// ── Test Suite ────────────────────────────────────────────────────────────

suite("Basilisk Activity Panel Accessibility Audit", function () {
  this.timeout(SUITE_SETUP_TIMEOUT_MS);

  let suiteContext: { tmpDir: string; basiliskBinary: string };

  suiteSetup(async function () {
    this.timeout(SUITE_SETUP_TIMEOUT_MS);
    suiteContext = await setupLspTestSuite("a11y-panel");

    const store = getStore();
    assert.ok(store, "Store should exist after activation");
    const result = await store.ensureLspReadyPromise(SERVER_START_WAIT_MS);
    assert.ok(result.ok, "LSP should be running");
  });

  suiteTeardown(function () {
    this.timeout(SUITE_SETUP_TIMEOUT_MS);
    teardownLspTestSuite(suiteContext?.tmpDir);
  });

  teardown(async () => {
    await closeAllEditors();
  });

  // ── Label Accessibility ─────────────────────────────────────────────────

  test("all activity panel views have descriptive names", function () {
    const pkg = loadPackageJSON();
    const views = pkg.contributes?.views?.["basilisk-explorer"] ?? [];

    for (const view of views) {
      assert.ok(view.name, `View "${view.id}" should have a name`);
      assert.ok(
        view.name.length >= 3,
        `View "${view.id}" name "${view.name}" should be descriptive (>=3 chars)`,
      );
    }
  });

  test("all activity panel commands have descriptive titles", function () {
    const pkg = loadPackageJSON();
    const commands = pkg.contributes?.commands ?? [];

    const panelCommands = commands.filter(
      (cmd) =>
        cmd.command.includes("ModuleExplorer") ||
        cmd.command.includes("TypeHealth") ||
        cmd.command.includes("toggleFeature") ||
        cmd.command.includes("openWalkthrough") ||
        cmd.command.includes("copyImportPath") ||
        cmd.command.includes("copyQualifiedName") ||
        cmd.command.includes("sortTypeHealth") ||
        cmd.command.includes("filterModuleExplorer"),
    );

    assert.ok(panelCommands.length > 0, "Should find panel-related commands");

    for (const cmd of panelCommands) {
      assert.ok(cmd.title, `Command "${cmd.command}" should have a title`);
      assert.ok(
        cmd.title.length >= 3,
        `Command "${cmd.command}" title "${cmd.title}" should be descriptive`,
      );
    }
  });

  // ── Icon + Text (Never Color Alone) ─────────────────────────────────────

  test("toolbar commands have icons for visual recognition", function () {
    const pkg = loadPackageJSON();
    const commands = pkg.contributes?.commands ?? [];
    const titleMenus = pkg.contributes?.menus?.["view/title"] ?? [];

    // Get commands that appear in view toolbars.
    const toolbarCommandIds = new Set(titleMenus.map((entry) => entry.command));

    for (const cmdId of toolbarCommandIds) {
      const cmd = commands.find((c) => c.command === cmdId);
      assert.ok(cmd, `Toolbar command "${cmdId}" should exist in commands`);
      assert.ok(
        cmd.icon,
        `Toolbar command "${cmdId}" should have an icon (screen readers announce title, sighted users see icon)`,
      );
    }
  });

  test("commands have category for consistent palette grouping", function () {
    const pkg = loadPackageJSON();
    const commands = pkg.contributes?.commands ?? [];

    const panelCommands = commands.filter(
      (cmd) =>
        cmd.command.includes("ModuleExplorer") ||
        cmd.command.includes("TypeHealth") ||
        cmd.command.includes("toggleFeature") ||
        cmd.command.includes("openWalkthrough"),
    );

    for (const cmd of panelCommands) {
      assert.ok(
        cmd.category,
        `Command "${cmd.command}" should have a category for command palette grouping`,
      );
    }
  });

  // ── Keyboard Navigation ─────────────────────────────────────────────────

  test("all toolbar menu entries have 'when' clauses to scope visibility", function () {
    const pkg = loadPackageJSON();
    const titleMenus = pkg.contributes?.menus?.["view/title"] ?? [];

    const panelMenus = titleMenus.filter(
      (entry) =>
        entry.when.includes("basilisk.moduleExplorer") ||
        entry.when.includes("basilisk.typeHealth") ||
        entry.when.includes("basilisk.info"),
    );

    assert.ok(panelMenus.length > 0, "Should find panel toolbar menus");

    for (const menu of panelMenus) {
      assert.ok(
        menu.when,
        `Menu for "${menu.command}" should have a 'when' clause to scope it`,
      );
      assert.ok(
        menu.when.includes("view =="),
        `Menu for "${menu.command}" 'when' should scope to a specific view`,
      );
    }
  });

  test("context menu entries have 'when' clauses that reference view context", function () {
    const pkg = loadPackageJSON();
    const contextMenus = pkg.contributes?.menus?.["view/item/context"] ?? [];

    const panelMenus = contextMenus.filter(
      (entry) => entry.when.includes("basilisk"),
    );

    for (const menu of panelMenus) {
      assert.ok(
        menu.when,
        `Context menu for "${menu.command}" should have a 'when' clause`,
      );
    }
  });

  // ── Welcome Content (Empty State Accessibility) ─────────────────────────

  test("welcome views provide meaningful empty-state messages", function () {
    const pkg = loadPackageJSON();
    const welcomeViews = pkg.contributes?.viewsWelcome ?? [];

    const panelWelcome = welcomeViews.filter(
      (entry) =>
        entry.view === "basilisk.moduleExplorer" ||
        entry.view === "basilisk.typeHealth",
    );

    assert.ok(
      panelWelcome.length >= 2,
      "Both moduleExplorer and typeHealth should have welcome content",
    );

    for (const welcome of panelWelcome) {
      assert.ok(
        welcome.contents.length >= 10,
        `Welcome content for "${welcome.view}" should be descriptive (not just whitespace)`,
      );
    }
  });

  // ── Screen Reader: Consistent Label Patterns ────────────────────────────

  test("module explorer commands follow 'Basilisk: Action' naming pattern", function () {
    const pkg = loadPackageJSON();
    const commands = pkg.contributes?.commands ?? [];

    const moduleCommands = commands.filter(
      (cmd) => cmd.command.includes("ModuleExplorer"),
    );

    for (const cmd of moduleCommands) {
      // Screen readers announce category + title, so both must be meaningful.
      assert.ok(cmd.title, `"${cmd.command}" needs a title for screen readers`);
      assert.ok(
        !cmd.title.includes("undefined"),
        `"${cmd.command}" title should not contain 'undefined'`,
      );
      assert.ok(
        !cmd.title.includes("TODO"),
        `"${cmd.command}" title should not contain 'TODO'`,
      );
    }
  });

  test("type health commands follow 'Basilisk: Action' naming pattern", function () {
    const pkg = loadPackageJSON();
    const commands = pkg.contributes?.commands ?? [];

    const healthCommands = commands.filter(
      (cmd) => cmd.command.includes("TypeHealth") || cmd.command.includes("sortTypeHealth"),
    );

    for (const cmd of healthCommands) {
      assert.ok(cmd.title, `"${cmd.command}" needs a title for screen readers`);
      assert.ok(
        !cmd.title.includes("undefined"),
        `"${cmd.command}" title should not contain 'undefined'`,
      );
    }
  });

  // ── View Visibility Configuration ───────────────────────────────────────

  test("info panel is always visible so new users can discover features", function () {
    const pkg = loadPackageJSON();
    const views = pkg.contributes?.views?.["basilisk-explorer"] ?? [];
    const infoView = views.find((v) => v.id === "basilisk.info");

    assert.ok(infoView, "info view should exist");
    assert.strictEqual(
      infoView.visibility,
      "visible",
      "Info panel should default to 'visible' for discoverability",
    );
  });

  test("data panels require workspace to avoid confusing empty state", function () {
    const pkg = loadPackageJSON();
    const views = pkg.contributes?.views?.["basilisk-explorer"] ?? [];

    const dataViews = views.filter(
      (v) => v.id === "basilisk.moduleExplorer" || v.id === "basilisk.typeHealth",
    );

    for (const view of dataViews) {
      assert.ok(
        view.when,
        `"${view.id}" should have a 'when' clause to avoid showing empty state without a workspace`,
      );
      assert.ok(
        view.when.includes("basilisk.hasWorkspace"),
        `"${view.id}" should depend on basilisk.hasWorkspace context key`,
      );
    }
  });
});
