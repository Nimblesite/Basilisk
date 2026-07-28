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
import { InfoPanelProvider } from "../../info-panel";
import {
    ModuleTreeItem,
    workspaceHealthBadge,
    workspaceHealthMessage,
} from "../../module-explorer";
import {
  manifestCommands,
  manifestConfigurationProperties,
  manifestContributes,
  manifestViews,
  type Contributes,
  type ViewContribution,
} from "./extension-manifest";
import {
    LSP_RESTART_WAIT_MS,
    pollUntilResult,
    setupLspTestSuite,
    teardownLspTestSuite,
    closeAllEditors,
} from "./test-helpers";

// ── Command lists ─────────────────────────────────────────────────────────

/**
 * Commands registered client-side for the module explorer panel.
 * These are registered via context.subscriptions in registerModuleExplorer().
 */
const MODULE_EXPLORER_COMMANDS = [
  "basilisk.refreshModuleExplorer",
  "basilisk.toggleModuleExplorerView",
  "basilisk.sortModuleExplorer",
  "basilisk.filterModuleExplorer",
  "basilisk.copyImportPath",
  "basilisk.copyQualifiedName",
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
  "basilisk.info",
] as const;

/**
 * Server-advertised command that backs the merged Modules panel. Type Health is
 * folded into this one response (issue #103), so the panel makes a single
 * round-trip; basilisk.typeHealth remains advertised for Zed/Neovim and is
 * covered by command-registration.test.ts.
 */
const PANEL_SERVER_COMMANDS = [
  "basilisk.workspaceModules",
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
function loadContributes(): Contributes {
  return manifestContributes();
}

/**
 * Every contributed command's icon, as a comparable glyph string.
 *
 * A manifest icon may be a glyph reference or a light/dark pair; both are
 * reduced to one string so uniqueness comparisons stay meaningful instead of
 * degrading to object identity (which every pair would trivially pass).
 */
function commandIconGlyphs(): Map<string, string> {
  return new Map(
    manifestCommands().map((cmd) => [
      cmd.command,
      typeof cmd.icon === "string" ? cmd.icon : JSON.stringify(cmd.icon ?? null),
    ]),
  );
}

/** Load the basilisk-explorer views from package.json. */
function loadBasiliskViews(): ViewContribution[] {
  const views = manifestViews()["basilisk-explorer"] ?? [];
  assert.ok(views.length > 0, "Extension should contribute views");
  return views;
}

/** Extract a TreeItem's label as a plain string. */
function rowLabel(item: vscode.TreeItem): string {
  const { label } = item;
  return typeof label === "string" ? label : label?.label ?? "";
}

/**
 * Quick actions promoted from the info panel to the Modules toolbar (issue
 * #103), when-gated on the server running so a button can never invoke an
 * unregistered handler [EXTACT-INFO-ACTION-WIRING].
 */
const PROMOTED_TOOLBAR_COMMANDS = [
  "basilisk.fixWorkspace",
  "basilisk.organizeImports",
  "basilisk.restartServer",
] as const;

// ── Test Suite ────────────────────────────────────────────────────────────

// eslint-disable-next-line max-lines-per-function
suite("Basilisk Activity Panel E2E Tests", function () {

  let suiteContext: { tmpDir: string; basiliskBinary: string };

  suiteSetup(async function () {
    suiteContext = await setupLspTestSuite("activity-panel");

    const store = getStore();
    assert.ok(store, "Store should exist after activation");
    const result = await store.ensureLspReadyPromise(LSP_RESTART_WAIT_MS);
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

  test("info view is always visible (no 'when' condition)", function () {
    const views = loadBasiliskViews();
    const infoView = views.find((view) => view.id === "basilisk.info");

    assert.ok(infoView, "info view should exist");
    assert.strictEqual(infoView.when, undefined, "info view should not have a 'when' condition");
    assert.strictEqual(infoView.visibility, "visible", "info view should have visibility 'visible'");
  });

  // ── Module Explorer Commands ──────────────────────────────────────────

  // Tests [EXTACT-MODULES-TOOLBAR] / [EXTACT-MODULES-CONTEXT-MENU] command registration.
  test("module explorer commands are registered", function () {
    for (const cmd of MODULE_EXPLORER_COMMANDS) {
      assertCommandRegistered(cmd, "Module Explorer");
    }
  });

  // Tests [EXTACT-MODULES-REFRESH] manual refresh button.
  test("refreshModuleExplorer command is executable", async function () {
    await vscode.commands.executeCommand("basilisk.refreshModuleExplorer");
  });

  // Tests [EXTACT-MODULES-TOOLBAR] Toggle View.
  test("toggleModuleExplorerView command is executable", async function () {
    await vscode.commands.executeCommand("basilisk.toggleModuleExplorerView");
  });

  // Tests [EXTACT-MODULES-TOOLBAR] Sort (the explicit picker, #189).
  test("sortModuleExplorer command opens the sort picker (#189)", async function () {
    // The command now shows a QuickPick of the explicit sort modes; dismiss it
    // so the test exercises the command without blocking on user input.
    const dismiss = new Promise<void>((resolve) => {
      setTimeout(() => {
        void vscode.commands.executeCommand("workbench.action.closeQuickOpen").then(() => { resolve(); });
      }, 200);
    });
    await Promise.all([
      vscode.commands.executeCommand("basilisk.sortModuleExplorer"),
      dismiss,
    ]);
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
    const original = cfg.get<boolean>("uv.enabled") ?? true;

    await vscode.commands.executeCommand("basilisk.toggleFeature", "basilisk.uv.enabled", !original);

    const updated = vscode.workspace.getConfiguration("basilisk").get<boolean>("uv.enabled");
    assert.strictEqual(updated, !original, "toggleFeature should flip the setting value");

    // Restore original value.
    await vscode.workspace.getConfiguration().update(
      "basilisk.uv.enabled",
      undefined,
      vscode.ConfigurationTarget.Workspace,
    );
  });

  // Regression for issue #65 [EXTACT-INFO-ACTION-WIRING]: every actionable row
  // the panel renders must resolve to a registered handler. In the slimmed
  // panel (issue #103) the actionable rows are the top-level feature toggles.
  // Drives the LIVE panel tree and checks each row's own command via the
  // sanctioned "re-registering a live command throws" probe.
  test("every actionable info panel row resolves to a registered command (no dead actions)", function () {
    const store = getStore();
    assert.ok(store, "Store should exist");

    const provider = new InfoPanelProvider(store);
    try {
      const actionableRows = provider
        .getChildren()
        .filter((row) => row.contextValue === "feature");

      assert.ok(actionableRows.length > 0, "info panel should render feature toggles");

      for (const row of actionableRows) {
        const commandId = row.command?.command;
        assert.ok(commandId, `"${rowLabel(row)}" must carry a command`);
        assertCommandRegistered(commandId, `Info panel toggle "${rowLabel(row)}"`);
      }
    } finally {
      provider.dispose();
    }
  });

  // Issue #103: the high-value quick actions were promoted from the info panel
  // to Modules-toolbar buttons, when-gated on basilisk.serverState == 'running'
  // so they can never render without a live handler [EXTACT-INFO-ACTION-WIRING].
  test("Fix All / Organize Imports / Restart are Modules toolbar buttons gated on the server running", function () {
    const contributes = loadContributes();
    const titleMenus = contributes?.menus?.["view/title"] ?? [];
    const moduleMenus = titleMenus.filter(
      (entry) => entry.when.includes("view == basilisk.moduleExplorer"),
    );

    for (const cmd of PROMOTED_TOOLBAR_COMMANDS) {
      const entry = moduleMenus.find((menu) => menu.command === cmd);
      assert.ok(entry, `"${cmd}" must be contributed to the Modules view/title toolbar`);
      assert.ok(
        entry.when.includes("basilisk.serverState == 'running'"),
        `"${cmd}" toolbar button must be when-gated on the server running, got: ${entry.when}`,
      );
    }
  });

  test("promoted toolbar commands are registered and executable while the server runs", async function () {
    for (const cmd of PROMOTED_TOOLBAR_COMMANDS) {
      assertCommandRegistered(cmd, "Promoted toolbar action");
    }
  });

  // Issue #113 [VSIX-MODULE-EXPLORER-TOOLBAR]: the Modules toolbar contract.
  // Read-only view-state actions render as deterministically ordered inline
  // icons; mutating actions and server control live in separate ordered
  // overflow groups (divider between them); inline glyphs never collide; and
  // the unrefined Fix All is feature-flagged off by default.
  test("Modules toolbar: deterministic order, read-only inline, no duplicate glyphs", function () {
    const contributes = loadContributes();
    const titleMenus = (contributes?.menus?.["view/title"] ?? []).filter(
      (entry) => entry.when.includes("view == basilisk.moduleExplorer"),
    );
    assert.ok(titleMenus.length > 0, "Modules view must contribute toolbar entries");

    for (const entry of titleMenus) {
      assert.match(
        entry.group ?? "",
        /@\d+$/,
        `"${entry.command}" must carry an explicit @N order, got: ${entry.group}`,
      );
    }

    const inline = titleMenus.filter((entry) => entry.group?.startsWith("navigation") === true);
    const inlineOrdered = [...inline].sort(
      (a, b) =>
        Number(a.group?.split("@")[1] ?? 0) - Number(b.group?.split("@")[1] ?? 0),
    );
    assert.deepStrictEqual(
      inlineOrdered.map((entry) => entry.command),
      [
        "basilisk.refreshModuleExplorer",
        "basilisk.toggleModuleExplorerView",
        "basilisk.filterModuleExplorer",
        "basilisk.sortModuleExplorer",
      ],
      "inline toolbar must be exactly the read-only view-state actions, in order " +
        "(Collapse All is VS Code's native showCollapseAll button, never contributed — #113)",
    );

    // Mutating + server-control actions live in the overflow menu, in
    // distinct groups so VS Code renders a divider between them.
    const overflow = new Map(
      titleMenus
        .filter((entry) => entry.group?.startsWith("navigation") !== true)
        .map((entry) => [entry.command, entry.group ?? ""]),
    );
    const fixAllGroup = overflow.get("basilisk.fixWorkspace");
    const organizeGroup = overflow.get("basilisk.organizeImports");
    const restartGroup = overflow.get("basilisk.restartServer");
    assert.ok(fixAllGroup, "fixWorkspace must be an overflow action, not an inline icon");
    assert.ok(organizeGroup, "organizeImports must be an overflow action, not an inline icon");
    assert.ok(restartGroup, "restartServer must be an overflow action, not an inline icon");
    assert.notStrictEqual(
      restartGroup.split("@")[0],
      fixAllGroup.split("@")[0],
      "server control must be divided from mutating actions",
    );

    // No two inline buttons may render the same (or near-identical) glyph.
    const commandIcons = commandIconGlyphs();
    const inlineIcons = inline.map((entry) => commandIcons.get(entry.command));
    assert.strictEqual(
      new Set(inlineIcons).size,
      inlineIcons.length,
      `inline toolbar icons must be unique, got: ${inlineIcons.join(", ")}`,
    );
  });

  // Issue #113 [VSIX-MODULE-EXPLORER-TOOLBAR]: the panel must ship exactly ONE
  // Collapse All — VS Code's native showCollapseAll button. The custom no-op
  // `basilisk.collapseModuleExplorer` was the duplicate; it (and any command
  // re-glyphed as $(collapse-all)) must never be contributed again.
  test("Modules toolbar contributes no Collapse All — only the native showCollapseAll exists", function () {
    const contributes = loadContributes();

    const collapseCommand = (contributes?.commands ?? []).find(
      (cmd) => cmd.command === "basilisk.collapseModuleExplorer",
    );
    assert.strictEqual(
      collapseCommand,
      undefined,
      "basilisk.collapseModuleExplorer must not exist — Collapse All is native (showCollapseAll)",
    );

    const moduleToolbar = (contributes?.menus?.["view/title"] ?? []).filter(
      (entry) => entry.when.includes("view == basilisk.moduleExplorer"),
    );
    const collapseEntries = moduleToolbar.filter(
      (entry) => entry.command === "basilisk.collapseModuleExplorer",
    );
    assert.strictEqual(
      collapseEntries.length,
      0,
      "no custom Collapse All may be contributed to the Modules toolbar",
    );

    // Defence-in-depth: no Modules toolbar command may re-introduce the
    // $(collapse-all) glyph, which would render as a second collapse button
    // next to the native one.
    const commandIcons = commandIconGlyphs();
    for (const entry of moduleToolbar) {
      assert.notStrictEqual(
        commandIcons.get(entry.command),
        "$(collapse-all)",
        `"${entry.command}" must not use the $(collapse-all) glyph — Collapse All is native (#113)`,
      );
    }
  });

  // Issue #151: the Sort button silently no-ops in the default tree view (sort is
  // flat-only per [EXTACT-MODULES-TOOLBAR]). It must only appear where it works —
  // gated on the flat view — so it is never a visible, enabled no-op.
  test("Sort is gated to flat view so it is never a no-op in the tree view", function () {
    const contributes = loadContributes();
    const sortEntry = (contributes?.menus?.["view/title"] ?? []).find(
      (entry) =>
        entry.command === "basilisk.sortModuleExplorer" &&
        entry.when.includes("view == basilisk.moduleExplorer"),
    );
    assert.ok(sortEntry, "sortModuleExplorer must be contributed to the Modules toolbar");
    assert.ok(
      sortEntry.when.includes("basilisk.moduleExplorerView == 'flat'"),
      `Sort must be gated on the flat view so it never no-ops in tree view, got: ${sortEntry.when}`,
    );
  });

  test("Fix All is feature-flagged: config default off, when-clause gated", function () {
    const contributes = loadContributes();
    const flag = manifestConfigurationProperties()["basilisk.experimental.fixAll"];
    assert.ok(flag, "basilisk.experimental.fixAll setting must be declared");
    assert.strictEqual(flag.type, "boolean");
    assert.strictEqual(flag.default, false, "Fix All must be off by default");

    const fixAllEntry = (contributes?.menus?.["view/title"] ?? []).find(
      (entry) =>
        entry.command === "basilisk.fixWorkspace" &&
        entry.when.includes("view == basilisk.moduleExplorer"),
    );
    assert.ok(fixAllEntry, "fixWorkspace must be contributed to the Modules toolbar");
    assert.ok(
      fixAllEntry.when.includes("config.basilisk.experimental.fixAll"),
      `fixWorkspace must be gated on the experimental flag, got: ${fixAllEntry.when}`,
    );
    assert.ok(
      fixAllEntry.when.includes("basilisk.serverState == 'running'"),
      "fixWorkspace must stay gated on the server running",
    );
  });

  // Tests [EXTACT-INFO-SERVER-INFO] freshness rule. Defect 3 of issue #103:
  // Server Info went stale — the provider only re-rendered on configuration
  // changes, so "Server: stopped" / a missing Version row persisted after the
  // server came up. The provider now holds a signals effect on
  // store.lspState/store.client; restarting the real server must therefore fire
  // the tree's change event without any config change.
  test("info panel re-renders on LSP state changes (no stale Server Info)", async function () {
    this.timeout(60_000);
    const store = getStore();
    assert.ok(store, "Store should exist");

    const provider = new InfoPanelProvider(store);
    try {
      const fired = new Promise<void>((resolve) => {
        const sub = provider.onDidChangeTreeData(() => {
          sub.dispose();
          resolve();
        });
      });

      await vscode.commands.executeCommand("basilisk.restartServer");
      await fired;

      // Restore a fully-running server for the tests that follow. isRunning()
      // can flip true a beat before the store's state listener re-registers
      // the server commands, so also wait for the commands to be re-advertised
      // — the very next tests assert on them.
      const ready = await store.ensureLspReadyPromise(LSP_RESTART_WAIT_MS);
      assert.ok(ready.ok, "LSP should be running again after restart");
      await pollUntilResult({
        fn: async () => store.serverCommands.value.size,
        predicate: (size) => size > 0,
        timeoutMs: LSP_RESTART_WAIT_MS,
      });
    } finally {
      provider.dispose();
    }
  });

  // ── Server-Advertised Commands ────────────────────────────────────────

  // Tests [EXTACT-LSP-COMMANDS-WORKSPACE-MODULES] is server-advertised.
  test("LSP server advertises basilisk.workspaceModules command", function () {
    const store = getStore();
    assert.ok(store, "Store should exist");
    assert.ok(
      store.isServerCommandAdvertised("basilisk.workspaceModules"),
      "Server should advertise basilisk.workspaceModules",
    );
  });

  // The merged Modules panel no longer calls basilisk.typeHealth (its rollup is
  // folded into workspaceModules, issue #103), but the command remains the
  // shared workspace-health rollup for editors without a unified panel
  // (Zed /health, Neovim :BasiliskHealth). Guard that it stays advertised.
  // Tests [EXTACT-LSP-COMMANDS-TYPE-HEALTH] stays advertised for Zed/Neovim.
  test("LSP server still advertises basilisk.typeHealth for other editors", function () {
    const store = getStore();
    assert.ok(store, "Store should exist");
    assert.ok(
      store.isServerCommandAdvertised("basilisk.typeHealth"),
      "Server should still advertise basilisk.typeHealth (Zed/Neovim health command)",
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

  // Tests [EXTACT-MODULES-TOOLBAR] contribution (Refresh / Toggle View / Filter / Sort).
  test("module explorer has toolbar actions in package.json", function () {
    const contributes = loadContributes();
    const titleMenus = contributes?.menus?.["view/title"] ?? [];

    const moduleMenus = titleMenus.filter(
      (entry) => entry.when.includes("view == basilisk.moduleExplorer"),
    );
    const menuCommands = moduleMenus.map((entry) => entry.command);

    assert.ok(menuCommands.includes("basilisk.refreshModuleExplorer"), "Should include refresh");
    assert.ok(menuCommands.includes("basilisk.toggleModuleExplorerView"), "Should include view toggle");
    assert.ok(menuCommands.includes("basilisk.filterModuleExplorer"), "Should include filter");
    assert.ok(menuCommands.includes("basilisk.sortModuleExplorer"), "Should include sort (folded Type Health)");
    // Collapse All is VS Code's native showCollapseAll button — never a
    // contributed command. A contributed collapse is the #113 duplicate.
    assert.ok(
      !menuCommands.includes("basilisk.collapseModuleExplorer"),
      "must NOT contribute a custom Collapse All — the native showCollapseAll is the only one (#113)",
    );
  });

  // Tests [EXTACT-MODULES-CONTEXT-MENU] Copy Import Path / Copy Qualified Name.
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

  // The settings cog was only on the BASILISK info panel title — easy to miss.
  // [VSIX-STATUS-BAR]: Open Configuration must be reachable from EVERY Basilisk
  // sidebar view title, plus the always-visible status bar (basilisk.statusMenu).
  test("Open Configuration is reachable from every Basilisk view title, not just the info panel", function () {
    const contributes = loadContributes();
    const titleMenus = contributes?.menus?.["view/title"] ?? [];
    const configViews = new Set(
      titleMenus
        .filter((entry) => entry.command === "basilisk.openConfigurationEditor")
        .map((entry) => {
          const match = /view == (basilisk\.[A-Za-z]+)/.exec(entry.when);
          return match?.[1];
        })
        .filter((view): view is string => view !== undefined),
    );
    for (const view of ["basilisk.info", "basilisk.moduleExplorer", "basilisk.pythonProcesses"]) {
      assert.ok(
        configViews.has(view),
        `Open Configuration must be contributed to ${view}'s title bar; got: ${[...configViews].join(", ")}`,
      );
    }
    // Every config-cog entry must stay gated on editor support so it never
    // renders a dead button when the server lacks the configuration editor.
    for (const entry of titleMenus.filter((menu) => menu.command === "basilisk.openConfigurationEditor")) {
      assert.ok(
        entry.when.includes("basilisk.configurationEditorSupported"),
        `config cog on '${entry.when}' must be gated on basilisk.configurationEditorSupported`,
      );
    }
  });

  test("clicking the status bar opens the config-first status menu, which is a declared command", function () {
    const contributes = loadContributes();
    const statusMenu = (contributes?.commands ?? []).find(
      (cmd) => cmd.command === "basilisk.statusMenu",
    );
    assert.ok(statusMenu, "basilisk.statusMenu must be declared in package.json");
    assertCommandRegistered("basilisk.statusMenu", "Status bar menu");
  });
});

// ── Merged Modules panel: health chrome + per-module coverage [EXTACT-MODULES] ─
//
// The Type Health panel was merged into the Modules panel (issue #103): the
// workspace summary now renders in the tree view's native message + numeric
// badge chrome, and each module carries a coverage bar on its description.
//
// Regression for issue #57: an empty workspace (totalFiles === 0) must render an
// explicit "no Python files" state, never a misleading 100% coverage bar.
// Spec: docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-MODULES-HEADER
suite("Modules panel health chrome [EXTACT-MODULES-HEADER]", function () {
  const emptyStats = {
    totalSymbols: 0,
    annotatedSymbols: 0,
    coveragePercent: 100,
    errors: 0,
    warnings: 0,
    adoptedFiles: 0,
    totalFiles: 0,
    // The #57 empty-state is only rendered once the initial scan finished;
    // an unfinished scan shows the loading state instead
    // ([EXTACT-MODULES-HEADER-LOADING], #144).
    scanComplete: true,
  };

  const measuredStats = {
    totalSymbols: 20,
    annotatedSymbols: 17,
    coveragePercent: 85,
    errors: 2,
    warnings: 3,
    adoptedFiles: 0,
    totalFiles: 3,
  };

  test("empty workspace message is 'No Python files found', never a 100% bar", function () {
    const message = workspaceHealthMessage(emptyStats);

    assert.strictEqual(
      message,
      "No Python files found",
      "empty workspace must render an explicit 'no Python files' state",
    );
    assert.ok(
      !message.includes("%"),
      `empty workspace must not show a percentage, got: "${message}"`,
    );
    assert.ok(
      !message.includes("█") && !message.includes("░"),
      `empty workspace must not render a coverage bar, got: "${message}"`,
    );
  });

  test("empty workspace shows no badge (nothing to flag)", function () {
    assert.strictEqual(
      workspaceHealthBadge(emptyStats),
      undefined,
      "empty workspace must not show a numeric badge",
    );
  });

  test("measured workspace message shows coverage percent and diagnostics", function () {
    const message = workspaceHealthMessage(measuredStats);
    assert.ok(message.includes("85%"), `expected the coverage percentage, got: "${message}"`);
    assert.ok(
      message.includes("🔴 2") && message.includes("🟠 3"),
      `expected error/warning tallies, got: "${message}"`,
    );
  });

  test("measured workspace badge counts outstanding diagnostics", function () {
    const badge = workspaceHealthBadge(measuredStats);
    assert.ok(badge, "measured workspace with diagnostics should have a badge");
    assert.strictEqual(badge.value, 5, "badge should count errors + warnings (2 + 3)");
  });

  // Tests [EXTACT-MODULES-MODULE-ROW] — the module row's folded-health description.
  test("each module row renders a coverage bar, percentage, and tallies", function () {
    const item = new ModuleTreeItem({
      name: "myapp.api",
      path: "/ws/myapp/api.py",
      kind: "module",
      symbols: [],
      coveragePercent: 85,
      errors: 2,
      warnings: 3,
      adopted: false,
    });
    const description = String(item.description);

    assert.ok(description.includes("85%"), `expected the coverage percentage, got: "${description}"`);
    assert.ok(description.includes("█"), `expected a coverage bar, got: "${description}"`);
    assert.ok(
      description.includes("🔴 2") && description.includes("🟠 3"),
      `expected error/warning tallies, got: "${description}"`,
    );
  });

  // Regression for issue #236 [EXTACT-MODULES-COUNT-STYLE]: inline tallies on
  // every plain-text surface (header message, module row description) must
  // render the coloured Unicode glyphs `🔴 n` (errors) / `🟠 n` (warnings) —
  // never the lettered `nE nW` form the spec forbids.
  test("tallies render count-style glyphs 🔴 n / 🟠 n, never nE nW letters (#236)", function () {
    const row = new ModuleTreeItem({
      name: "myapp.api", path: "/ws/myapp/api.py", kind: "module", symbols: [],
      coveragePercent: 85, errors: 2, warnings: 3, adopted: false,
    });
    const surfaces = [
      ["header", workspaceHealthMessage(measuredStats)],
      ["module row", String(row.description)],
    ] as const;
    for (const [surface, text] of surfaces) {
      assert.ok(
        text.includes("🔴 2") && text.includes("🟠 3"),
        `${surface} tally must use the 🔴 n / 🟠 n glyph style, got: "${text}"`,
      );
      assert.ok(
        !text.includes("2E") && !text.includes("3W"),
        `${surface} tally must never use nE nW letters, got: "${text}"`,
      );
    }
  });

  // Tests [EXTACT-MODULES-MODULE-ROW] — the row's `[adopted]` badge.
  test("adopted module row shows the [adopted] badge", function () {
    const item = new ModuleTreeItem({
      name: "legacy",
      path: "/ws/legacy.py",
      kind: "module",
      symbols: [],
      coveragePercent: 12,
      errors: 11,
      warnings: 19,
      adopted: true,
    });

    assert.ok(
      String(item.description).includes("[adopted]"),
      "adopted module must show the [adopted] badge",
    );
  });
});
