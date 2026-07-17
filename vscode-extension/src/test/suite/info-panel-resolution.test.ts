// Tests for [EXTACT-INFO-SERVER-INFO]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-SERVER-INFO
/**
 * Server Info resolution E2E tests — regression guard for issue #153.
 *
 * The test workspace leaves `basilisk.python`, `basilisk.uv.executablePath`,
 * and `basilisk.executablePath` unset (their `""` defaults), i.e. auto-detect
 * is in effect for all three. The Server Info rows must then surface what the
 * server ACTUALLY resolved — interpreter/binary version + path — never the
 * bare `auto-detect` placeholder, and the Binary row must never be blank.
 * Resolution data is LSP-authoritative: it comes from the live client's
 * initialize response, so the provider here is built on the extension's REAL
 * store (unlike info-panel.test.ts, which uses a fresh store to test layout).
 */

import * as assert from "assert";
import * as path from "path";
import type * as vscode from "vscode";
import { InfoPanelProvider } from "../../info-panel";
import { getStore } from "../../extension";
import { SUITE_SETUP_TIMEOUT_MS, waitForLspReady } from "./test-helpers";

/** Extract a TreeItem's label as a plain string. */
function labelOf(item: vscode.TreeItem): string {
  const { label } = item;
  if (typeof label === "string") { return label; }
  return label?.label ?? "";
}

/** Extract a TreeItem's description as a plain string. */
function descriptionOf(item: vscode.TreeItem): string {
  const { description } = item;
  return typeof description === "string" ? description : "";
}

const ASCII_DIGITS = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"] as const;

/** Whether the text contains at least one ASCII digit (a version number does). */
function containsDigit(text: string): boolean {
  return ASCII_DIGITS.some((digit) => text.includes(digit));
}

/**
 * Assert a row description has the resolved shape the issue requires when
 * auto-detect is in effect: `auto-detect → <version> (<path>)`, or the
 * explicit failure form `auto-detect → none found`. The bare placeholder
 * literal `auto-detect` — the shipped bug — fails this.
 */
function assertAutoDetectResolved(rowLabel: string, desc: string): void {
  assert.notStrictEqual(
    desc,
    "auto-detect",
    `${rowLabel} row must not render the bare "auto-detect" placeholder — it must show what auto-detect resolved (issue #153)`,
  );
  assert.ok(
    desc.startsWith("auto-detect → "),
    `${rowLabel} row must mark that auto-detect is in effect and show its outcome, got "${desc}"`,
  );
  if (desc === "auto-detect → none found") {
    return; // Explicit failure state is a valid, honest outcome.
  }
  assert.ok(
    desc.includes(" (") && desc.endsWith(")") && containsDigit(desc),
    `${rowLabel} row must show the resolved "<version> (<path>)" or "none found", got "${desc}"`,
  );
}

suite("Server Info resolved environment (issue #153)", () => {
  let provider: InfoPanelProvider;

  suiteSetup(async function () {
    this.timeout(SUITE_SETUP_TIMEOUT_MS);
    await waitForLspReady();
  });

  setup(() => {
    const store = getStore();
    assert.ok(store, "extension store should exist once the LSP is ready");
    provider = new InfoPanelProvider(store);
  });

  teardown(() => {
    provider.dispose();
  });

  /** Children of the Server Info section, keyed by row label. */
  function serverInfoRow(label: string): vscode.TreeItem | undefined {
    const section = provider.getChildren().find((row) => labelOf(row) === "Server Info");
    assert.ok(section, "Server Info section should exist");
    return provider.getChildren(section).find((row) => labelOf(row) === label);
  }

  // Defect 1 of issue #153: the Python row rendered the raw setting (default
  // "" → literal "auto-detect") and never the resolved interpreter.
  test("Python row shows the resolved interpreter (version + path), not the literal auto-detect", () => {
    const row = serverInfoRow("Python");
    assert.ok(row, "Python row should exist");
    assertAutoDetectResolved("Python", descriptionOf(row));
  });

  // Defect 2 of issue #153: the uv row had the same placeholder problem —
  // no resolved uv binary, no uv version.
  test("uv row shows the resolved uv binary (version + path), not the literal auto-detect", () => {
    const row = serverInfoRow("uv");
    assert.ok(row, "uv row should exist");
    assertAutoDetectResolved("uv", descriptionOf(row));
  });

  // Defect 3 of issue #153: basilisk.executablePath defaults to "" (not
  // undefined), so the `?? "basilisk"` fallback never fired and the Binary
  // row rendered blank. With the server running it must name the actually
  // running binary; while the server is down the row is absent — never blank.
  test("Binary row is never blank — it names the running server binary (version + absolute path)", () => {
    const row = serverInfoRow("Binary");
    assert.ok(
      row,
      "Binary row should exist while the server is running (absent is only valid with no live server)",
    );
    const desc = descriptionOf(row);
    assert.ok(desc.trim() !== "", "Binary row must never render blank (issue #153)");
    assert.ok(
      desc.includes(" (") && desc.endsWith(")") && containsDigit(desc),
      `Binary row must show the running binary as "<version> (<path>)", got "${desc}"`,
    );
    const openParen = desc.lastIndexOf(" (");
    const binaryPath = desc.slice(openParen + 2, -1);
    assert.ok(
      path.isAbsolute(binaryPath),
      `Binary row must name the resolved ABSOLUTE path of the running binary, got "${binaryPath}"`,
    );
  });
});
