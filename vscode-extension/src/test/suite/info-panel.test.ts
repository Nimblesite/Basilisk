// Tests for [EXTACT-INFO]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-FEATURE-STATUS
/**
 * Info Panel contents E2E tests.
 *
 * The Feature Status section is only allowed to list toggles whose setting has
 * a real, observable effect (a toggle that writes a setting nothing reads is a
 * lie to the user). These tests pin the shipped set and prove the one toggle
 * with a panel-visible effect — uv Integration — actually changes the panel.
 *
 * If someone re-adds a no-op toggle (e.g. "Ruff Integration", whose setting the
 * LSP server silently drops), the first test fails. See
 * EXTENSION-ACTIVITY-PANEL-PLAN.md#EXTACT-PLAN-FEATURE-TOGGLES for the work
 * required before a removed toggle may return.
 */

import * as assert from "assert";
import * as vscode from "vscode";
import { InfoPanelProvider } from "../../info-panel";
import { createStore } from "../../store";
import { EXTENSION_ID } from "./test-helpers";

/** Toggles that ship — each has a namesake, observable effect. */
const KEPT_FEATURE_LABELS = ["Type Checking", "uv Integration"] as const;

/** Toggles removed because their setting was a no-op (server dropped it). */
const REMOVED_FEATURE_LABELS = [
  "Inlay Hints (Params)",
  "Inlay Hints (Types)",
  "Ruff Integration",
  "Test Explorer",
  "Debugger",
  "AI Typing",
] as const;

/** uv Quick Actions that must vanish when uv Integration is toggled off. */
const UV_ACTIONS = ["uv Sync", "uv Add Package", "uv Lock", "uv Create Env"] as const;

/** Extract a TreeItem's label as a plain string. */
function labelOf(item: vscode.TreeItem): string {
  const { label } = item;
  if (typeof label === "string") { return label; }
  return label?.label ?? "";
}

async function setUvEnabled(value: boolean | undefined): Promise<void> {
  await vscode.workspace.getConfiguration().update(
    "basilisk.uv.enabled",
    value,
    vscode.ConfigurationTarget.Workspace,
  );
}

suite("Basilisk Info Panel Contents", () => {
  let provider: InfoPanelProvider;

  setup(() => {
    provider = new InfoPanelProvider(createStore());
  });

  teardown(async () => {
    provider.dispose();
    await setUvEnabled(undefined);
  });

  /** Children of a named top-level section in the info tree. */
  function sectionItems(sectionLabel: string): vscode.TreeItem[] {
    const sections = provider.getChildren();
    const section = sections.find((entry) => labelOf(entry) === sectionLabel);
    assert.ok(section, `"${sectionLabel}" section should exist`);
    return provider.getChildren(section);
  }

  test("Feature Status lists exactly the toggles with a real effect", () => {
    const labels = sectionItems("Feature Status").map(labelOf);
    assert.deepStrictEqual(
      labels,
      [...KEPT_FEATURE_LABELS],
      `Feature Status must list exactly: ${KEPT_FEATURE_LABELS.join(", ")}`,
    );
  });

  test("Feature Status hides every no-op toggle", () => {
    const labels = sectionItems("Feature Status").map(labelOf);
    for (const removed of REMOVED_FEATURE_LABELS) {
      assert.ok(
        !labels.includes(removed),
        `"${removed}" must not appear — its setting is ignored, so the toggle does nothing`,
      );
    }
  });

  // The positive "uv actions appear when uv is enabled" case lives in
  // activity-panel.test.ts, not here: per issue #103 defect 1, a uv Quick Action
  // may only render when the server advertises its handler, which requires a
  // running LSP. This suite's provider is built on a fresh, server-down store,
  // so the uv rows are correctly hidden regardless of the uv.enabled toggle —
  // see "Quick Actions hides server-advertised rows when the server is not
  // running" below for the server-down half of the contract.

  test("uv Integration hides uv Quick Actions when disabled", async () => {
    await setUvEnabled(false);
    const labels = sectionItems("Quick Actions").map(labelOf);
    for (const action of UV_ACTIONS) {
      assert.ok(
        !labels.includes(action),
        `"${action}" must be hidden when uv Integration is off`,
      );
    }
    // Non-uv actions remain regardless of the toggle.
    assert.ok(labels.includes("Restart Server"), "non-uv actions stay visible");
  });

  // Regression for issue #103 defect 1 [EXTACT-INFO-ACTION-WIRING]: the
  // server-advertised quick actions (Fix All, Organize Imports, uv.*) were
  // rendered unconditionally. Their handlers are only registered once the LSP
  // reaches Running (store.syncServerCommands), so with the server stopped or
  // mid-startup clicking a row raised "command not found". A row may only
  // appear if its command resolves to a live handler — so when the server is
  // not running these rows MUST be hidden, not shown-but-dead.
  const SERVER_DEPENDENT_ACTIONS = [
    "Fix All in Workspace",
    "Organize Imports (Workspace)",
    ...UV_ACTIONS,
  ] as const;

  test("Quick Actions hides server-advertised rows when the server is not running", () => {
    // The suite-level provider is built on a fresh createStore(): lspState is
    // "idle" and serverCommands is empty — i.e. the server is stopped. None of
    // the server-advertised actions resolve to a live handler in this state.
    const labels = sectionItems("Quick Actions").map(labelOf);
    for (const dead of SERVER_DEPENDENT_ACTIONS) {
      assert.ok(
        !labels.includes(dead),
        `"${dead}" is a server-advertised command; with the server stopped its ` +
        `handler is not registered, so the row would raise "command not found" ` +
        `when clicked and must be hidden`,
      );
    }
    // The client-core actions stay available regardless of server state — you
    // must be able to restart a stopped server and open its log.
    assert.ok(labels.includes("Restart Server"), "client-core actions stay visible");
    assert.ok(labels.includes("Show Output"), "client-core actions stay visible");
  });
});

// ── Affordance partition [EXTACT-INFO-AFFORDANCE] ───────────────────────────
//
// Regression tests for issue #65: actionable rows (feature toggles, quick
// actions) were visually indistinguishable from read-only Server Info rows —
// no imperative tooltip, no inline button affordance — so users could not tell
// what was a button and what was just information.
//
// Spec: docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-AFFORDANCE

/** Section labels whose rows are actionable (carry a command). */
const ACTIONABLE_SECTIONS = ["Feature Status", "Quick Actions"] as const;
/** Section label whose rows are read-only display only. */
const READONLY_SECTION = "Server Info";

/** contextValues the spec assigns to actionable rows. */
const ACTIONABLE_CONTEXT = new Set(["feature", "action"]);

/** Extract a TreeItem's tooltip as a plain string (handles MarkdownString). */
function tooltipOf(item: vscode.TreeItem): string {
  const { tooltip } = item;
  if (typeof tooltip === "string") { return tooltip; }
  if (tooltip instanceof vscode.MarkdownString) { return tooltip.value; }
  return "";
}

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

  /** Children of a named top-level section. */
  function rowsIn(sectionLabel: string): vscode.TreeItem[] {
    const section = provider.getChildren().find((entry) => labelOf(entry) === sectionLabel);
    assert.ok(section, `"${sectionLabel}" section should exist`);
    return provider.getChildren(section);
  }

  test("every actionable row carries a command and an imperative tooltip", () => {
    for (const sectionLabel of ACTIONABLE_SECTIONS) {
      const rows = rowsIn(sectionLabel);
      assert.ok(rows.length > 0, `"${sectionLabel}" should have rows`);
      for (const row of rows) {
        const label = labelOf(row);
        assert.ok(
          row.command !== undefined && row.command.command !== "",
          `"${label}" in ${sectionLabel} must carry a command (actionable rows are clickable)`,
        );
        assert.ok(
          ACTIONABLE_CONTEXT.has(String(row.contextValue)),
          `"${label}" must have an actionable contextValue, got "${String(row.contextValue)}"`,
        );
        const tip = tooltipOf(row).trim();
        assert.ok(
          tip.length > 0,
          `"${label}" in ${sectionLabel} must carry an imperative tooltip describing its effect`,
        );
      }
    }
  });

  test("every read-only Server Info row carries no command and contextValue 'info'", () => {
    const rows = rowsIn(READONLY_SECTION);
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
    const actionable = ACTIONABLE_SECTIONS.flatMap(rowsIn);
    const readOnly = rowsIn(READONLY_SECTION);
    for (const row of actionable) {
      assert.notStrictEqual(row.contextValue, "info", `"${labelOf(row)}" must not be read-only`);
    }
    for (const row of readOnly) {
      assert.ok(
        !ACTIONABLE_CONTEXT.has(String(row.contextValue)),
        `"${labelOf(row)}" must not be actionable`,
      );
    }
  });

  test("package.json contributes an inline action button for actionable info rows only", () => {
    const inlineForInfo = loadItemContextMenus().filter(
      (entry) => entry.group === "inline" && entry.when.includes("basilisk.info"),
    );
    assert.ok(
      inlineForInfo.length > 0,
      "info panel actionable rows must contribute an inline button (literal button affordance)",
    );
    for (const entry of inlineForInfo) {
      assert.ok(
        entry.when.includes("action") && entry.when.includes("feature"),
        `inline button '${entry.command}' must target action/feature rows, got when: ${entry.when}`,
      );
      assert.ok(
        !/viewItem\s*==\s*info/.test(entry.when),
        `inline button '${entry.command}' must not target read-only info rows, got when: ${entry.when}`,
      );
    }
  });
});
