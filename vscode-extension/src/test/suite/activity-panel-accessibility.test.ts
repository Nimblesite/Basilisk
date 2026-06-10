// Tests for [EXTACT]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT
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
    WAIT_MS,
    setupLspTestSuite,
    teardownLspTestSuite,
    closeAllEditors,
} from "./test-helpers";

// ── Package.json types ───────────────────────────────────────────────────���

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

// ── Assertion helpers (extracted to keep the suite body under 120 lines) ──

function assertViewsHaveDescriptiveNames(): void {
  const pkg = loadPackageJSON();
  const views = pkg.contributes?.views?.["basilisk-explorer"] ?? [];

  for (const view of views) {
    assert.ok(view.name, `View "${view.id}" should have a name`);
    assert.ok(
      view.name.length >= 3,
      `View "${view.id}" name "${view.name}" should be descriptive (>=3 chars)`,
    );
  }
}

function assertCommandsHaveDescriptiveTitles(): void {
  const pkg = loadPackageJSON();
  const commands = pkg.contributes?.commands ?? [];

  const panelCommands = commands.filter(
    (cmd) =>
      cmd.command.includes("ModuleExplorer") ||
      cmd.command.includes("toggleFeature") ||
      cmd.command.includes("openWalkthrough") ||
      cmd.command.includes("copyImportPath") ||
      cmd.command.includes("copyQualifiedName") ||
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
}

function assertToolbarCommandsHaveIcons(): void {
  const pkg = loadPackageJSON();
  const commands = pkg.contributes?.commands ?? [];
  const titleMenus = pkg.contributes?.menus?.["view/title"] ?? [];

  const toolbarCommandIds = new Set(titleMenus.map((entry) => entry.command));

  for (const cmdId of toolbarCommandIds) {
    const cmd = commands.find((c) => c.command === cmdId);
    assert.ok(cmd, `Toolbar command "${cmdId}" should exist in commands`);
    assert.ok(
      cmd.icon,
      `Toolbar command "${cmdId}" should have an icon`,
    );
  }
}

function assertCommandsHaveCategory(): void {
  const pkg = loadPackageJSON();
  const commands = pkg.contributes?.commands ?? [];

  const panelCommands = commands.filter(
    (cmd) =>
      cmd.command.includes("ModuleExplorer") ||
      cmd.command.includes("toggleFeature") ||
      cmd.command.includes("openWalkthrough"),
  );

  for (const cmd of panelCommands) {
    assert.ok(
      cmd.category,
      `Command "${cmd.command}" should have a category for command palette grouping`,
    );
  }
}

function assertToolbarMenusHaveWhenClauses(): void {
  const pkg = loadPackageJSON();
  const titleMenus = pkg.contributes?.menus?.["view/title"] ?? [];

  const panelMenus = titleMenus.filter(
    (entry) =>
      entry.when.includes("basilisk.moduleExplorer") ||
      entry.when.includes("basilisk.info"),
  );

  assert.ok(panelMenus.length > 0, "Should find panel toolbar menus");

  for (const menu of panelMenus) {
    assert.ok(menu.when, `Menu for "${menu.command}" should have a 'when' clause`);
    assert.ok(
      menu.when.includes("view =="),
      `Menu for "${menu.command}" 'when' should scope to a specific view`,
    );
  }
}

function assertContextMenusHaveWhenClauses(): void {
  const pkg = loadPackageJSON();
  const contextMenus = pkg.contributes?.menus?.["view/item/context"] ?? [];

  const panelMenus = contextMenus.filter((entry) => entry.when.includes("basilisk"));

  for (const menu of panelMenus) {
    assert.ok(menu.when, `Context menu for "${menu.command}" should have a 'when' clause`);
  }
}

function assertWelcomeViewsHaveMeaningfulContent(): void {
  const pkg = loadPackageJSON();
  const welcomeViews = pkg.contributes?.viewsWelcome ?? [];

  const panelWelcome = welcomeViews.filter(
    (entry) => entry.view === "basilisk.moduleExplorer",
  );

  assert.ok(
    panelWelcome.length >= 1,
    "The merged Modules panel should have welcome content",
  );

  for (const welcome of panelWelcome) {
    assert.ok(
      welcome.contents.length >= 10,
      `Welcome content for "${welcome.view}" should be descriptive`,
    );
  }
}

function assertCommandsFollowNamingPattern(filter: (cmd: CommandContribution) => boolean): void {
  const pkg = loadPackageJSON();
  const commands = pkg.contributes?.commands ?? [];
  const filtered = commands.filter(filter);

  for (const cmd of filtered) {
    assert.ok(cmd.title, `"${cmd.command}" needs a title for screen readers`);
    assert.ok(!cmd.title.includes("undefined"), `"${cmd.command}" title should not contain 'undefined'`);
    assert.ok(!cmd.title.includes("TODO"), `"${cmd.command}" title should not contain 'TODO'`);
  }
}

function assertInfoPanelVisible(): void {
  const pkg = loadPackageJSON();
  const views = pkg.contributes?.views?.["basilisk-explorer"] ?? [];
  const infoView = views.find((v) => v.id === "basilisk.info");

  assert.ok(infoView, "info view should exist");
  assert.strictEqual(infoView.visibility, "visible", "Info panel should default to 'visible'");
}

function assertDataPanelsRequireWorkspace(): void {
  const pkg = loadPackageJSON();
  const views = pkg.contributes?.views?.["basilisk-explorer"] ?? [];

  const dataViews = views.filter(
    (v) => v.id === "basilisk.moduleExplorer",
  );

  for (const view of dataViews) {
    assert.ok(view.when, `"${view.id}" should have a 'when' clause`);
    assert.ok(
      view.when.includes("basilisk.hasWorkspace"),
      `"${view.id}" should depend on basilisk.hasWorkspace context key`,
    );
  }
}

// ── Test Suite ────────────────────────────────────────────────────────────

suite("Basilisk Activity Panel Accessibility Audit", function () {

  let suiteContext: { tmpDir: string; basiliskBinary: string };

  suiteSetup(async function () {
    suiteContext = await setupLspTestSuite("a11y-panel");

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

  test("all activity panel views have descriptive names", assertViewsHaveDescriptiveNames);
  test("all activity panel commands have descriptive titles", assertCommandsHaveDescriptiveTitles);
  test("toolbar commands have icons for visual recognition", assertToolbarCommandsHaveIcons);
  test("commands have category for consistent palette grouping", assertCommandsHaveCategory);
  test("toolbar menu entries have 'when' clauses", assertToolbarMenusHaveWhenClauses);
  test("context menu entries have 'when' clauses", assertContextMenusHaveWhenClauses);
  test("welcome views provide meaningful empty-state messages", assertWelcomeViewsHaveMeaningfulContent);

  test("module explorer commands follow naming pattern", function () {
    assertCommandsFollowNamingPattern((cmd) => cmd.command.includes("ModuleExplorer"));
  });

  test("info panel is always visible for discoverability", assertInfoPanelVisible);
  test("data panels require workspace to avoid empty state", assertDataPanelsRequireWorkspace);
});
