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

  test("uv Integration shows uv Quick Actions when enabled", async () => {
    await setUvEnabled(true);
    const labels = sectionItems("Quick Actions").map(labelOf);
    for (const action of UV_ACTIONS) {
      assert.ok(labels.includes(action), `"${action}" should appear when uv is enabled`);
    }
  });

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
});
