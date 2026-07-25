// Tests for [EXTACT-INFO]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO
/**
 * Info Panel contents E2E tests — the slimmed panel of issue #103.
 *
 * The panel is exactly: the Diagnostics toggle followed by flat, read-only server
 * details. There is NO Quick Actions section: the high-value
 * actions are Modules-toolbar buttons gated on the server running (see
 * activity-panel.test.ts), the status-bar click opens the basilisk.statusMenu
 * quick-pick (Open Configuration / Show Output / Restart), and everything stays
 * in the command palette.
 *
 * This structure is itself the regression guard for issue #103 defect 1
 * ("command not found" quick actions): with no action rows in the panel at
 * all, a dead shown-but-unregistered action row is structurally impossible —
 * the only commands any row carries are the always-registered
 * basilisk.toggleFeature toggles.
 *
 * Feature toggles: only toggles whose setting has a real, observable effect
 * may appear (a toggle that writes a setting nothing reads is a lie to the
 * user). If someone re-adds a no-op toggle (e.g. "Ruff Integration", whose
 * setting the LSP server silently drops), the toggle-set test fails. See
 * EXTENSION-ACTIVITY-PANEL-PLAN.md#EXTACT-PLAN-FEATURE-TOGGLES.
 */

import * as assert from "assert";
import * as vscode from "vscode";
import { InfoPanelProvider, featureToggleTarget } from "../../info-panel";
import type { TypeshedStatusState } from "../../configuration-editor-model";
import { createStore } from "../../store";
import { EXTENSION_ID, SUITE_SETUP_TIMEOUT_MS, waitForLspReady } from "./test-helpers";

/** Toggles that ship — each has a namesake, observable effect. */
const KEPT_FEATURE_LABELS = ["Diagnostics"] as const;

/** Toggles removed because their setting was a no-op (server dropped it). */
const REMOVED_FEATURE_LABELS = [
  // Removed per GitHub #190: no server code reads basilisk.uv.enabled, so the
  // toggle never disabled uv integration — a no-op affordance.
  "uv Integration",
  "Inlay Hints (Params)",
  "Inlay Hints (Types)",
  "Ruff Integration",
  "Test Explorer",
  "Debugger",
  "AI Typing",
] as const;

/** Extract a TreeItem's label as a plain string. */
function labelOf(item: vscode.TreeItem): string {
  const { label } = item;
  if (typeof label === "string") { return label; }
  return label?.label ?? "";
}

/** Extract a TreeItem's tooltip as a plain string (handles MarkdownString). */
function tooltipOf(item: vscode.TreeItem): string {
  const { tooltip } = item;
  if (typeof tooltip === "string") { return tooltip; }
  if (tooltip instanceof vscode.MarkdownString) { return tooltip.value; }
  return "";
}

function verifyTypeshedInfoRows(): void {
  const store = createStore();
  const writable = store.typeshedStatuses as unknown as {
    value: ReadonlyMap<string, TypeshedStatusState>;
  };
  writable.value = new Map([[
    "file:///workspace",
    {
      lifecycle: { kind: "Ready" }, activeSource: { kind: "Bundled" },
      noSourceReason: undefined,
      commitIdentity: "83c2518a9e6abbda0c44592c3483de459198f887",
      licenseStatus: { kind: "Approved" },
      warnings: [{
        code: "typeshed_source_unpinned", message: "Pin a commit to make this reproducible",
        severity: { kind: "Advisory" },
      }],
    },
  ]]);
  const typeshedProvider = new InfoPanelProvider(store);
  try {
    const rows = typeshedProvider.getChildren().filter((row) => row.contextValue === "info");
    const byLabel = new Map(rows.map((row) => [labelOf(row), row]));
    const source = byLabel.get("Typeshed Source");
    assert.ok(String(source?.description).includes("83c2518a9e6abbda0c44592c3483de459198f887"));
    const sourceTooltip = tooltipOf(source ?? new vscode.TreeItem("missing"));
    assert.ok(sourceTooltip.includes("Commit: 83c2518a9e6abbda0c44592c3483de459198f887"));
    assert.ok(sourceTooltip.includes("Source: Bundled"));
    assert.ok(sourceTooltip.includes("License: Approved"));
    assert.ok(!byLabel.has("Typeshed Transport"), "trust details belong in one source tooltip");
    assert.strictEqual(
      byLabel.get("Typeshed typeshed_source_unpinned")?.description,
      "Pin a commit to make this reproducible",
    );
  } finally {
    typeshedProvider.dispose();
  }
}

function verifyDownloadingTypeshedSpinner(): void {
  const store = createStore();
  const writable = store.typeshedStatuses as unknown as {
    value: ReadonlyMap<string, TypeshedStatusState>;
  };
  writable.value = new Map([[
    "file:///workspace",
    {
      lifecycle: { kind: "Downloading" }, noSourceReason: undefined, activeSource: undefined,
      commitIdentity: undefined,
      licenseStatus: { kind: "Unavailable" },
      warnings: [],
    },
  ]]);
  const typeshedProvider = new InfoPanelProvider(store);
  try {
    const state = typeshedProvider
      .getChildren()
      .find((row) => labelOf(row) === "Typeshed State");
    assert.ok(state?.iconPath instanceof vscode.ThemeIcon);
    assert.strictEqual(state.iconPath.id, "loading~spin");
  } finally {
    typeshedProvider.dispose();
  }
}

/** A store whose single root reports the typeshed_source_unpinned typeshed warning. */
function storeWithUnpinnedWarning(): ReturnType<typeof createStore> {
  const store = createStore();
  const writable = store.typeshedStatuses as unknown as {
    value: ReadonlyMap<string, TypeshedStatusState>;
  };
  writable.value = new Map([[
    "file:///workspace",
    {
      lifecycle: { kind: "Ready" }, activeSource: { kind: "Bundled" },
      noSourceReason: undefined,
      commitIdentity: "6fb14c98ee340a07eea807a4c804e20a849eb92b",
      licenseStatus: { kind: "Approved" },
      warnings: [{
        code: "typeshed_source_unpinned", message: "Pin a commit to make this reproducible",
        severity: { kind: "Advisory" },
      }],
    },
  ]]);
  return store;
}

/**
 * Drive the store into the EXACT state in which
 * configuration-editor-registration.ts registers the open command: a running
 * server that advertises the editor capability. The panel gates the warning
 * row's command on this same pair, so anything less must leave the row inert.
 */
function advertiseConfigurationEditor(
  store: ReturnType<typeof createStore>,
  options: { readonly running: boolean },
): void {
  const writableClient = store.client as unknown as { value: unknown };
  writableClient.value = {
    initializeResult: {
      capabilities: { experimental: { basilisk: { configurationEditor: true } } },
    },
  };
  const writableState = store.lspState as unknown as { value: string };
  writableState.value = options.running ? "running" : "starting";
}

/** The single typeshed_source_unpinned warning row from a flat-root panel. */
function unpinnedRow(store: ReturnType<typeof createStore>): vscode.TreeItem {
  const typeshedProvider = new InfoPanelProvider(store);
  try {
    const row = typeshedProvider
      .getChildren()
      .find((candidate) => labelOf(candidate) === "Typeshed typeshed_source_unpinned");
    assert.ok(row, "the typeshed_source_unpinned warning row should exist");
    return row;
  } finally {
    typeshedProvider.dispose();
  }
}

// Tests [LSPCFGED-TYPESHED-SERVICE-INFO] navigation + [EXTACT-INFO-AFFORDANCE]:
// the typeshed_source_unpinned row's own message tells the user to "Pin current", and Pin
// current lives in the configuration editor — so the row must navigate there
// when the editor is genuinely reachable (info-panel.ts typeshedWarningItem).
function verifyUnpinnedWarningRowOpensConfigurationEditor(): void {
  const store = storeWithUnpinnedWarning();
  advertiseConfigurationEditor(store, { running: true });
  const unpinned = unpinnedRow(store);
  assert.strictEqual(
    unpinned.command?.command,
    "basilisk.openConfigurationEditor",
    "the typeshed_source_unpinned row advertises Pin current, so clicking it must open the configuration editor where Pin current lives",
  );
  assert.strictEqual(
    unpinned.contextValue,
    "typeshed-warning",
    "a navigating warning row is marked typeshed-warning so it never gets the feature-toggle inline button",
  );
  const tip = tooltipOf(unpinned).trim();
  assert.ok(
    tip.length > 0,
    "an actionable row must carry an imperative tooltip describing its effect",
  );
}

// Regression guard for issue #103 defect 1: basilisk.openConfigurationEditor
// is capability-gated (configuration-editor-registration.ts), so a warning row
// must NOT carry it while no server advertises the editor — a shown-but-dead
// command raises "command not found".
function verifyUnpinnedWarningRowStaysInertWithoutEditorCapability(): void {
  const unpinned = unpinnedRow(storeWithUnpinnedWarning());
  assert.strictEqual(
    unpinned.command,
    undefined,
    "without the configuration-editor capability the row must not carry a dead command",
  );
  assert.strictEqual(
    unpinned.contextValue,
    "info",
    "an inert warning row stays an ordinary read-only info row",
  );
}

// The command is registered on `running` AND the capability — not the
// capability alone. A client that has already returned its initializeResult
// while the server is still starting advertises the capability with no command
// registered yet, so the row must stay inert. Guards the gate asymmetry that
// would otherwise be load-bearing but unasserted.
function verifyUnpinnedWarningRowStaysInertWhileServerIsNotRunning(): void {
  const store = storeWithUnpinnedWarning();
  advertiseConfigurationEditor(store, { running: false });
  const unpinned = unpinnedRow(store);
  assert.strictEqual(
    unpinned.command,
    undefined,
    "the capability alone is not enough — the open command is only registered while the server runs",
  );
}

suite("Basilisk Info Panel Contents (slimmed, issue #103)", () => {
  let provider: InfoPanelProvider;

  // The write-through test drives the real basilisk.toggleFeature command,
  // which exists once the extension has initialized — await that here so this
  // file also passes standalone (single-file debugging), not only when an
  // earlier suite already initialized the extension.
  suiteSetup(async function () {
    this.timeout(SUITE_SETUP_TIMEOUT_MS);
    await waitForLspReady();
  });

  setup(() => {
    provider = new InfoPanelProvider(createStore());
  });

  teardown(() => {
    provider.dispose();
  });

  /**
   * Flat read-only server-information rows: every root row that is not one of
   * the shipped feature toggles. Selected by LABEL, never by `contextValue` —
   * selecting on the property under test would make the `contextValue`
   * assertions below vacuous (a row that lost its `info` marker would silently
   * drop out of the set instead of failing).
   */
  function serverInfoRows(): vscode.TreeItem[] {
    const toggles = new Set<string>(KEPT_FEATURE_LABELS);
    return provider.getChildren().filter((row) => !toggles.has(labelOf(row)));
  }

  // Tests [EXTACT-INFO-STRUCTURE] / [EXTACT-INFO-QUICK-ACTIONS] (no Quick Actions section).
  test("root is the Diagnostics toggle followed by flat read-only details", () => {
    const labels = provider.getChildren().map(labelOf);
    assert.deepStrictEqual(labels.slice(0, KEPT_FEATURE_LABELS.length), [...KEPT_FEATURE_LABELS]);
    assert.ok(labels.includes("Analysis Mode"), "server details should render at the root");
    assert.ok(!labels.includes("Server Info"), "read-only details do not need a collapsible parent");
    assert.ok(!labels.includes("Feature Status"), "the Feature Status header was removed (one toggle doesn't justify it)");
    assert.ok(!labels.includes("Quick Actions"), "the Quick Actions section was removed (actions live on the Modules toolbar / status bar / palette)");
  });

  // Tests [EXTACT-INFO-ACTION-WIRING]: no shown-but-dead actions in the panel.
  test("no row in the entire panel carries a command outside the allowed set", () => {
    // Regression for issue #103 defect 1: a row that looks clickable but has
    // no live handler raises "command not found". Exactly two commands may
    // appear in this panel, and each is guaranteed to be registered whenever
    // it is attached:
    //   - basilisk.toggleFeature — registerInfoPanel registers it itself, so
    //     it is always live.
    //   - basilisk.openConfigurationEditor — capability-gated. It is attached
    //     ONLY to typeshed warning rows and ONLY when the same predicate that
    //     registers it holds (running server + advertised capability), so it
    //     can never be shown dead. See [LSPCFGED-TYPESHED-SERVICE-INFO].
    // Any OTHER command, on any row, is the defect this test exists to catch.
    const allRows = provider
      .getChildren()
      .flatMap((row) => [row, ...provider.getChildren(row)]);
    assert.ok(allRows.length > 0, "panel should render rows");
    for (const row of allRows) {
      const commandId = row.command?.command;
      if (commandId === undefined) { continue; }
      if (commandId === "basilisk.openConfigurationEditor") {
        assert.strictEqual(
          row.contextValue,
          "typeshed-warning",
          `"${labelOf(row)}" carries the configuration-editor command but is not a typeshed warning row — only warning rows may navigate`,
        );
        continue;
      }
      assert.strictEqual(
        commandId,
        "basilisk.toggleFeature",
        `"${labelOf(row)}" carries "${commandId}" — only the always-registered toggle and the capability-gated configuration-editor command are allowed in this panel`,
      );
    }
  });

  // Tests [EXTACT-INFO-FEATURE-STATUS]: only effect-bearing toggles ship.
  test("every no-op toggle stays hidden", () => {
    const labels = provider.getChildren().map(labelOf);
    for (const removed of REMOVED_FEATURE_LABELS) {
      assert.ok(
        !labels.includes(removed),
        `"${removed}" must not appear — its setting is ignored, so the toggle does nothing`,
      );
    }
  });

  // Tests [EXTACT-INFO-SERVER-INFO]: no live server-state row.
  test("Server Info has no live Server state row (status bar owns it)", () => {
    const labels = serverInfoRows().map(labelOf);
    assert.ok(
      !labels.includes("Server"),
      "the Server state row duplicates the status bar and was dropped (issue #103)",
    );
  });

  test("Server Info renders the root-keyed Typeshed source and trust state", verifyTypeshedInfoRows);

  test("Server Info shows a downloading Typeshed spinner", verifyDownloadingTypeshedSpinner);

  test(
    "the typeshed_source_unpinned warning row opens the configuration editor where Pin current lives",
    verifyUnpinnedWarningRowOpensConfigurationEditor,
  );

  test(
    "the typeshed_source_unpinned warning row stays inert while no server advertises the configuration editor",
    verifyUnpinnedWarningRowStaysInertWithoutEditorCapability,
  );

  test(
    "the typeshed_source_unpinned warning row stays inert while the server is not yet running",
    verifyUnpinnedWarningRowStaysInertWhileServerIsNotRunning,
  );

  // Tests [EXTACT-INFO-SERVER-INFO]: one uv row, sub-settings in the tooltip.
  test("uv sub-settings are folded into the uv row tooltip, not separate rows", () => {
    const rows = serverInfoRows();
    const labels = rows.map(labelOf);
    assert.ok(!labels.includes("uv Auto-Sync"), "uv Auto-Sync must not be its own row");
    assert.ok(!labels.includes("Stub Suggestions"), "Stub Suggestions must not be its own row");

    const uvRow = rows.find((row) => labelOf(row) === "uv");
    assert.ok(uvRow, "the compact uv row should exist");
    const tip = tooltipOf(uvRow);
    assert.ok(tip.includes("Auto-Sync"), `uv tooltip must carry Auto-Sync, got: "${tip}"`);
    assert.ok(tip.includes("Executable"), `uv tooltip must carry Executable, got: "${tip}"`);
    // Stub suggestions are governed by rule severity (BSK-0152), not a uv
    // setting, so they are neither a row nor a tooltip line.
    assert.ok(!tip.includes("Stub Suggestions"), `uv tooltip must not carry the removed Stub Suggestions setting, got: "${tip}"`);
  });

  // Defect 2 of issue #103: basilisk.toggleFeature wrote to
  // ConfigurationTarget.Workspace unconditionally, which is invalid (and
  // rejects) when no workspace folder is open — and the info panel is always
  // visible, so that state is reachable. The target now derives from the live
  // folder count; the helper is pure in the count because the e2e host always
  // launches with a folder, making the no-folder branch unreachable end-to-end.
  // Tests [EXTACT-INFO-FEATURE-STATUS] write-target rule (Workspace vs Global).
  test("featureToggleTarget picks Workspace with a folder and Global without (defect 2)", () => {
    assert.strictEqual(
      featureToggleTarget(1),
      vscode.ConfigurationTarget.Workspace,
      "with a workspace folder, toggles write workspace settings",
    );
    assert.strictEqual(
      featureToggleTarget(0),
      vscode.ConfigurationTarget.Global,
      "with no folder open, ConfigurationTarget.Workspace is invalid — must fall back to Global",
    );
  });

  // Tests [EXTACT-INFO-FEATURE-STATUS]: a toggle has an observable, namesake effect.
  test("toggleFeature writes through and the panel reflects it", async () => {
    // End-to-end: flip the Diagnostics toggle off via the real command (this host has a
    // folder, so it writes the Workspace target) and assert the toggle row
    // re-renders as Disabled. (The deeper effect — diagnostics actually clear —
    // is proven end-to-end in type-checking-toggle.test.ts.)
    const cfg = vscode.workspace.getConfiguration();
    try {
      await vscode.commands.executeCommand("basilisk.toggleFeature", "basilisk.enabled", false);
      const toggle = provider.getChildren().find((row) => labelOf(row) === "Diagnostics");
      assert.ok(toggle, "Diagnostics toggle should exist");
      assert.strictEqual(toggle.description, "Disabled", "toggle row must reflect the written setting");
    } finally {
      await cfg.update("basilisk.enabled", undefined, vscode.ConfigurationTarget.Workspace);
    }
  });
});

// ── Affordance partition [EXTACT-INFO-AFFORDANCE] ───────────────────────────
//
// Regression tests for issue #65: actionable rows must be visually
// unmistakable from read-only rows. In the slimmed panel the actionable class
// is exactly the Diagnostics toggle; every server-detail row is read-only.
//
// Spec: docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-AFFORDANCE

interface InlineMenuEntry {
  readonly command: string;
  readonly when: string;
  readonly group?: string;
}

/** Load the extension's contributed view/item/context menu entries. */
function loadItemContextMenus(): InlineMenuEntry[] {
  const ext = vscode.extensions.getExtension(EXTENSION_ID);
  assert.ok(ext, "Extension should be installed");
  const pkg = ext.packageJSON as {
    contributes?: { menus?: { "view/item/context"?: InlineMenuEntry[] } };
  };
  return pkg.contributes?.menus?.["view/item/context"] ?? [];
}

suite("Basilisk Info Panel Affordance [EXTACT-INFO-AFFORDANCE]", () => {
  let provider: InfoPanelProvider;

  setup(() => {
    provider = new InfoPanelProvider(createStore());
  });

  teardown(() => {
    provider.dispose();
  });

  // Both partitions are selected by LABEL, never by `contextValue`. The whole
  // point of this suite is that the two classes are marked correctly, so
  // selecting on the marker under test would make every assertion below
  // self-fulfilling: a toggle that regressed to `contextValue: "info"` would
  // vanish from `toggleRows()` rather than fail. Label selection keeps the
  // partition independent of the property being asserted.

  /** Top-level feature toggle rows. */
  function toggleRows(): vscode.TreeItem[] {
    const toggles = new Set<string>(KEPT_FEATURE_LABELS);
    return provider.getChildren().filter((row) => toggles.has(labelOf(row)));
  }

  /** Flat read-only server-information rows. */
  function readOnlyRows(): vscode.TreeItem[] {
    const toggles = new Set<string>(KEPT_FEATURE_LABELS);
    return provider.getChildren().filter((row) => !toggles.has(labelOf(row)));
  }

  test("every feature toggle carries a command and an imperative tooltip", () => {
    const rows = toggleRows();
    assert.ok(rows.length > 0, "panel should render feature toggles");
    for (const row of rows) {
      const label = labelOf(row);
      assert.ok(
        row.command !== undefined && row.command.command !== "",
        `"${label}" must carry a command (actionable rows are clickable)`,
      );
      const tip = tooltipOf(row).trim();
      assert.ok(
        tip.length > 0,
        `"${label}" must carry an imperative tooltip describing its effect`,
      );
    }
  });

  test("every read-only server detail carries no command and contextValue 'info'", () => {
    const rows = readOnlyRows();
    assert.ok(rows.length > 0, "Server Info should have rows");
    for (const row of rows) {
      const label = labelOf(row);
      // The one documented exception ([LSPCFGED-TYPESHED-SERVICE-INFO]): a
      // typeshed warning row navigates to the Configuration Editor where its
      // named fix lives. It is still read-only — it mutates nothing — so it
      // gets its own contextValue and therefore still no inline button.
      if (row.contextValue === "typeshed-warning") {
        assert.strictEqual(
          row.command?.command,
          "basilisk.openConfigurationEditor",
          `"${label}" is marked typeshed-warning, so it must carry exactly the navigation-only editor command`,
        );
        continue;
      }
      assert.strictEqual(
        row.command,
        undefined,
        `"${label}" is read-only and must not carry a command`,
      );
      assert.strictEqual(
        row.contextValue,
        "info",
        `"${label}" must have contextValue "info" so it gets no inline button`,
      );
    }
  });

  test("no row is both actionable and read-only", () => {
    for (const row of toggleRows()) {
      assert.notStrictEqual(row.contextValue, "info", `"${labelOf(row)}" must not be read-only`);
    }
    for (const row of readOnlyRows()) {
      assert.notStrictEqual(row.contextValue, "feature", `"${labelOf(row)}" must not be actionable`);
    }
  });

  test("package.json contributes an inline action button for feature rows only", () => {
    const inlineForInfo = loadItemContextMenus().filter(
      (entry) => entry.group === "inline" && entry.when.includes("basilisk.info"),
    );
    assert.ok(
      inlineForInfo.length > 0,
      "feature toggle rows must contribute an inline button (literal button affordance)",
    );
    for (const entry of inlineForInfo) {
      assert.ok(
        entry.when.includes("feature"),
        `inline button '${entry.command}' must target feature rows, got when: ${entry.when}`,
      );
      assert.ok(
        !/viewItem\s*=~?=?\s*.*action/.test(entry.when),
        `inline button '${entry.command}' must not target the removed action rows, got when: ${entry.when}`,
      );
      assert.ok(
        !/viewItem\s*==\s*info/.test(entry.when),
        `inline button '${entry.command}' must not target read-only info rows, got when: ${entry.when}`,
      );
    }
  });
});
