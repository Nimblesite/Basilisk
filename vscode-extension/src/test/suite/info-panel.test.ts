// Tests for [EXTACT-INFO]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO
/**
 * Info Panel contents E2E tests — the slimmed panel of issue #103.
 *
 * The panel is exactly: the feature toggles at the root (no "Feature Status"
 * section header — a single shipped toggle doesn't justify one) plus a compact
 * read-only Server Info section. There is NO Quick Actions section: the high-value
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
import { createStore } from "../../store";
import { EXTENSION_ID, SUITE_SETUP_TIMEOUT_MS, waitForLspReady } from "./test-helpers";

/** Toggles that ship — each has a namesake, observable effect. */
const KEPT_FEATURE_LABELS = ["Type Checking"] as const;

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

  /** Children of the Server Info section. */
  function serverInfoRows(): vscode.TreeItem[] {
    const section = provider.getChildren().find((row) => labelOf(row) === "Server Info");
    assert.ok(section, "Server Info section should exist");
    return provider.getChildren(section);
  }

  // Tests [EXTACT-INFO-STRUCTURE] / [EXTACT-INFO-QUICK-ACTIONS] (no Quick Actions section).
  test("root is exactly the feature toggles followed by Server Info — no section headers, no Quick Actions", () => {
    const labels = provider.getChildren().map(labelOf);
    assert.deepStrictEqual(
      labels,
      [...KEPT_FEATURE_LABELS, "Server Info"],
      "slimmed panel root must be the shipped toggle(s) + Server Info, in order",
    );
    assert.ok(!labels.includes("Feature Status"), "the Feature Status header was removed (one toggle doesn't justify it)");
    assert.ok(!labels.includes("Quick Actions"), "the Quick Actions section was removed (actions live on the Modules toolbar / status bar / palette)");
  });

  // Tests [EXTACT-INFO-ACTION-WIRING]: no shown-but-dead actions in the panel.
  test("no row in the entire panel carries a command other than the registered toggle", () => {
    // Regression for issue #103 defect 1: a row that looks clickable but has
    // no live handler raises "command not found". The slimmed panel makes
    // that impossible — the ONLY command any row may carry is
    // basilisk.toggleFeature, which registerInfoPanel itself registers.
    const allRows = provider
      .getChildren()
      .flatMap((row) => [row, ...provider.getChildren(row)]);
    assert.ok(allRows.length > 0, "panel should render rows");
    for (const row of allRows) {
      const commandId = row.command?.command;
      if (commandId !== undefined) {
        assert.strictEqual(
          commandId,
          "basilisk.toggleFeature",
          `"${labelOf(row)}" carries "${commandId}" — only the always-registered toggle command is allowed in this panel`,
        );
      }
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
    // Stub suggestions are governed by rule severity (BSK-E0152), not a uv
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
    // End-to-end: flip Type Checking off via the real command (this host has a
    // folder, so it writes the Workspace target) and assert the toggle row
    // re-renders as Disabled. (The deeper effect — diagnostics actually clear —
    // is proven end-to-end in type-checking-toggle.test.ts.)
    const cfg = vscode.workspace.getConfiguration();
    try {
      await vscode.commands.executeCommand("basilisk.toggleFeature", "basilisk.enabled", false);
      const toggle = provider.getChildren().find((row) => labelOf(row) === "Type Checking");
      assert.ok(toggle, "Type Checking toggle should exist");
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
// is exactly the feature toggles; everything under Server Info is read-only.
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

  /** Top-level feature toggle rows. */
  function toggleRows(): vscode.TreeItem[] {
    return provider.getChildren().filter((row) => row.contextValue === "feature");
  }

  /** Read-only rows under Server Info. */
  function readOnlyRows(): vscode.TreeItem[] {
    const section = provider.getChildren().find((row) => labelOf(row) === "Server Info");
    assert.ok(section, "Server Info section should exist");
    return provider.getChildren(section);
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

  test("every read-only Server Info row carries no command and contextValue 'info'", () => {
    const rows = readOnlyRows();
    assert.ok(rows.length > 0, "Server Info should have rows");
    for (const row of rows) {
      const label = labelOf(row);
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
