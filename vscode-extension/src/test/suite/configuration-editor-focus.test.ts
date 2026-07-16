// Implements [CONFIGEDITOR-VSIX-EXPERIENCE]: the Configure Severity hover
// deep link — `basilisk.openConfigurationEditor` with a `{ rule }` argument
// opens the editor focused on that rule, and the LSP hover markdown is
// trusted for exactly that one command.

import * as assert from "assert";
import * as vscode from "vscode";
import type {
  ApplyConfigurationRequest,
  ConfigurationPreview,
  ConfigurationSnapshot,
  PreviewConfigurationRequest,
  RuleOccurrencesRequest,
  RuleOccurrencesResponse,
} from "../../configuration-editor-model";
import {
  ConfigurationEditorController,
  CONFIGURATION_EDITOR_COMMAND,
  configurationEditorFocusRule,
  type ConfigurationEditorTransport,
} from "../../configuration-editor";
import { trustConfigureSeverityLinks } from "../../lsp-client";
import { createStore } from "../../store";

const ROOT_URI = "file:///workspace";
const RULE_CODE = "BSK-0001";

function snapshotWithRule(): ConfigurationSnapshot {
  return {
    rootUri: ROOT_URI,
    configUri: `${ROOT_URI}/pyproject.toml`,
    revision: "revision-1",
    rules: [{
      descriptor: {
        code: RULE_CODE,
        title: "Missing parameter type annotation",
        summary: "All function parameters require explicit types.",
        docsUrl: `https://example.test/errors/${RULE_CODE}`,
        tags: ["basilisk", "annotations"],
      },
      entry: undefined,
      effectiveSeverity: { kind: "Error" },
      diagnosticCount: 1,
    }],
    tags: [],
  };
}

/** Snapshot-only transport; preview/apply/occurrences are unreachable here. */
class SnapshotTransport implements ConfigurationEditorTransport {
  public async snapshot(_rootUri: string): Promise<ConfigurationSnapshot> {
    return snapshotWithRule();
  }
  public async preview(_request: PreviewConfigurationRequest): Promise<ConfigurationPreview> {
    throw new Error("preview is not under test");
  }
  public async apply(_request: ApplyConfigurationRequest): Promise<ConfigurationSnapshot> {
    throw new Error("apply is not under test");
  }
  public async occurrences(_request: RuleOccurrencesRequest): Promise<RuleOccurrencesResponse> {
    throw new Error("occurrences is not under test");
  }
}

async function pollUntil(predicate: () => boolean, timeoutMs = 5_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!predicate() && Date.now() < deadline) {
    await new Promise<void>((resolve) => setTimeout(resolve, 25));
  }
  assert.ok(predicate(), "condition did not become true before timeout");
}

suite("Configuration editor — Configure Severity deep link", () => {
  // The command argument is untrusted webview/markdown input: only a bounded,
  // non-empty `{ rule: string }` yields a focus target.
  test("decodes only a bounded { rule } command argument", () => {
    assert.strictEqual(configurationEditorFocusRule({ rule: RULE_CODE }), RULE_CODE);
    assert.strictEqual(configurationEditorFocusRule(undefined), undefined);
    assert.strictEqual(configurationEditorFocusRule(null), undefined);
    assert.strictEqual(configurationEditorFocusRule(RULE_CODE), undefined);
    assert.strictEqual(configurationEditorFocusRule({ rule: 7 }), undefined);
    assert.strictEqual(configurationEditorFocusRule({ rule: "" }), undefined);
    assert.strictEqual(configurationEditorFocusRule({ rule: "x".repeat(65) }), undefined);
    assert.strictEqual(configurationEditorFocusRule([{ rule: RULE_CODE }]), undefined);
  });

  // Store semantics: string sets, undefined (internal refresh) preserves for
  // the same root, null (plain open) clears, and a root change clears.
  test("focusRule is set, survives same-root refreshes, and clears on plain open", () => {
    const store = createStore();
    store.beginConfigurationLoad(ROOT_URI, RULE_CODE);
    assert.strictEqual(store.configurationEditor.value.focusRule, RULE_CODE);

    store.beginConfigurationLoad(ROOT_URI);
    assert.strictEqual(store.configurationEditor.value.focusRule, RULE_CODE);

    store.acceptConfigurationSnapshot(snapshotWithRule());
    assert.strictEqual(store.configurationEditor.value.focusRule, RULE_CODE);

    store.beginConfigurationLoad(ROOT_URI, null);
    assert.strictEqual(store.configurationEditor.value.focusRule, undefined);

    store.beginConfigurationLoad(ROOT_URI, RULE_CODE);
    store.beginConfigurationLoad("file:///elsewhere");
    assert.strictEqual(store.configurationEditor.value.focusRule, undefined);
  });

  // The controller's open() carries the focus target through its load chain
  // into the state the webview renders; a later plain open clears it.
  test("open(rootUri, rule) keeps the focus target through load; plain open clears it", async () => {
    const store = createStore();
    const controller = new ConfigurationEditorController(store, new SnapshotTransport());
    try {
      controller.open(ROOT_URI, RULE_CODE);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      assert.strictEqual(store.configurationEditor.value.focusRule, RULE_CODE);

      controller.open(ROOT_URI);
      await pollUntil(() => store.configurationEditor.value.phase === "ready");
      assert.strictEqual(store.configurationEditor.value.focusRule, undefined);
    } finally {
      controller.dispose();
    }
  });

  // Hover markdown from the LSP must become clickable for exactly the one
  // configuration-editor command — nothing else gets trusted.
  test("hover middleware trusts exactly the openConfigurationEditor command", () => {
    const markdown = new vscode.MarkdownString(
      `[Configure Severity](command:${CONFIGURATION_EDITOR_COMMAND}?%5B%7B%22rule%22%3A%22${RULE_CODE}%22%7D%5D)`,
    );
    const hover = trustConfigureSeverityLinks(new vscode.Hover([markdown]));
    assert.ok(hover);
    const [content] = hover.contents;
    assert.ok(content instanceof vscode.MarkdownString);
    assert.deepStrictEqual(content.isTrusted, { enabledCommands: [CONFIGURATION_EDITOR_COMMAND] });

    assert.strictEqual(trustConfigureSeverityLinks(null), null);
    assert.strictEqual(trustConfigureSeverityLinks(undefined), undefined);
  });
});
