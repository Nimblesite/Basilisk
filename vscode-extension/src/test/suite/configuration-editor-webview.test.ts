// Implements [CONFIGEDITOR-ACCESSIBILITY-SECURITY] / [CONFIGEDITOR-VSIX-EXPERIENCE].

import * as assert from "assert";
import { buildConfigurationEditorDocument } from "../../configuration-editor-document";
import { decodeConfigurationEditorIntent } from "../../configuration-editor-intents";

suite("Configuration editor — untrusted intent decoder", () => {
  // [CONFIGEDITOR-MODEL]: exactly four mutations exist — SetRule, RemoveRule,
  // SetTag, RemoveTag. Every persisted severity is accepted; nothing else is.
  test("accepts the four EditorMutation kinds with every persisted severity", () => {
    for (const severity of ["Error", "Warning", "Info", "Disabled"]) {
      const setRule = decodeConfigurationEditorIntent({
        type: "preview",
        mutations: [{ kind: "SetRule", code: "BSK-0001", severity: { kind: severity } }],
      });
      assert.strictEqual(setRule?.type, "preview", `SetRule ${severity} must be accepted`);
      const setTag = decodeConfigurationEditorIntent({
        type: "preview",
        mutations: [{ kind: "SetTag", tag: "basilisk", severity: { kind: severity } }],
      });
      assert.strictEqual(setTag?.type, "preview", `SetTag ${severity} must be accepted`);
    }
    assert.strictEqual(decodeConfigurationEditorIntent({
      type: "preview", mutations: [{ kind: "RemoveRule", code: "BSK-0001" }],
    })?.type, "preview");
    assert.strictEqual(decodeConfigurationEditorIntent({
      type: "preview", mutations: [{ kind: "RemoveTag", tag: "basilisk" }],
    })?.type, "preview");
  });

  // [CONFIGEDITOR-ACCEPTANCE]: selector mutations, Inherit/Native settings,
  // scopes, and fix-safety selectors were removed from the contract.
  test("rejects malformed payloads and every removed legacy shape", () => {
    assert.strictEqual(decodeConfigurationEditorIntent({ type: "preview", mutations: [] }), undefined);
    assert.strictEqual(decodeConfigurationEditorIntent({
      type: "preview",
      mutations: [{ kind: "SetRule", code: "BSK-0001", severity: { kind: "Inherit" } }],
    }), undefined);
    assert.strictEqual(decodeConfigurationEditorIntent({
      type: "preview",
      mutations: [{ kind: "SetRule", code: "BSK-0001", severity: { kind: "Native" } }],
    }), undefined);
    assert.strictEqual(decodeConfigurationEditorIntent({
      type: "preview",
      mutations: [{ selector: { kind: "All" }, setting: { kind: "Error" }, scope: { kind: "Project" } }],
    }), undefined);
    assert.strictEqual(decodeConfigurationEditorIntent({
      type: "preview", mutations: [{ kind: "SetRule", severity: { kind: "Error" } }],
    }), undefined);
    assert.strictEqual(decodeConfigurationEditorIntent({
      type: "preview", mutations: [{ kind: "SetTag", tag: "", severity: { kind: "Error" } }],
    }), undefined);
    assert.strictEqual(decodeConfigurationEditorIntent({ type: "fixSafe" }), undefined);
  });

  // [CONFIGEDITOR-OPERATIONS]: occurrence reads use only the all/codes/tags
  // selectors; fixability selectors no longer exist.
  test("accepts read-side occurrence selectors and rejects removed ones", () => {
    for (const selector of [
      { kind: "All" },
      { kind: "Codes", codes: ["BSK-0001"] },
      { kind: "Tags", tags: ["pep"], matchAll: false },
    ]) {
      assert.strictEqual(decodeConfigurationEditorIntent({
        type: "occurrences", selector, cursor: undefined, limit: 100,
      })?.type, "occurrences");
    }
    for (const selector of [
      { kind: "CurrentViolations" },
      { kind: "SafeFixable" },
      { kind: "WithoutSafeFix" },
    ]) {
      assert.strictEqual(decodeConfigurationEditorIntent({
        type: "occurrences", selector, cursor: undefined, limit: 100,
      }), undefined, `${selector.kind} was removed from the contract`);
    }
    assert.strictEqual(decodeConfigurationEditorIntent({ type: "occurrences", selector: { kind: "All" }, limit: 0 }), undefined);
  });
});

suite("Configuration editor — hardened, accessible document", () => {
  test("is CSP locked, theme-native, zoom resilient, inert while blocked, and keyboard traversable", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(html.includes("default-src 'none'"));
    assert.ok(/style-src 'nonce-[^']+'/.test(html));
    assert.ok(/script-src 'nonce-[^']+'/.test(html));
    assert.ok(!html.includes("unsafe-inline"));
    assert.ok(!html.includes("https://"), "the shell must load no remote resources");
    assert.ok(html.includes("var(--vscode-editor-background)"));
    assert.ok(html.includes("vscode-high-contrast"));
    assert.ok(html.includes("prefers-reduced-motion"));
    assert.ok(html.includes('id="announcer" class="sr-only" aria-live="polite"'));
    assert.ok(html.includes(".inert = blocking"));
    assert.ok(html.includes('aria-modal="true"'));
    assert.ok(html.includes("max-height: calc(100vh - 32px)"));
    assert.ok(html.includes("function moveVirtualRuleFocus(event)"));
    assert.ok(html.includes('id="rule-spacer" role="list"'));
    assert.ok(html.includes("row.setAttribute('role', 'listitem')"));
    assert.ok(html.includes("row.setAttribute('aria-posinset'"));
    assert.ok(html.includes("row.setAttribute('aria-setsize'"));
    assert.ok(html.includes("['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End']"));
    assert.ok(html.includes("viewport.scrollTop = target * ROW_HEIGHT"));
  });

  // [CONFIGEDITOR-VSIX-EXPERIENCE]: the tag-first Rules view is the whole
  // editor. Tag groups get the tag-entry control; rows get per-rule entry
  // controls; pep rows have no Disabled option ([CHKARCH-CONFIG-MODEL]).
  test("renders the tag-first Rules view with pep-gated entry controls", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(html.includes("select.dataset.tagEntry = tag.name"), "tag groups expose the tag-entry control");
    assert.ok(html.includes("select.dataset.ruleEntry = rule.descriptor.code"), "rows expose per-rule entry controls");
    assert.ok(html.includes("const PEP_TAG = 'pep'"));
    assert.ok(
      html.includes("SEVERITIES.filter((value) => value !== 'Disabled')"),
      "pep rows must offer error/warning/info/remove-entry and never Disabled",
    );
    assert.ok(html.includes("isPepRule(rule)"));
    assert.ok(html.includes("{ kind: 'RemoveRule', code }"), "remove-entry emits RemoveRule");
    assert.ok(html.includes("{ kind: 'RemoveTag', tag }"), "remove-entry emits RemoveTag");
    assert.ok(html.includes("{ kind: 'SetRule', code, severity: { kind: value } }"));
    assert.ok(html.includes("{ kind: 'SetTag', tag, severity: { kind: value } }"));
    assert.ok(html.includes("Load more occurrences"));
    assert.ok(html.includes("Exact resolved changes"));
    assert.ok(html.includes("impactCell(impact.errorsBefore, impact.errorsAfter, 'errors')"));
    assert.ok(html.includes('id="rule-search"'), "search stays");
    assert.ok(html.includes("Open raw"));
  });

  // [CONFIGEDITOR-ACCEPTANCE]: adoption, path overrides, presets, and
  // Inherit/Native controls are removed outright — no legacy shims.
  test("carries no adoption, path-override, preset, or Inherit/Native UI", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(!html.includes("adoption"), "the Adoption view is deleted");
    assert.ok(!html.includes("pathOverrides"), "the Path Overrides view is deleted");
    assert.ok(!html.includes("preset"), "preset UI is deleted");
    assert.ok(!html.includes("'Inherit'"), "no Inherit control survives");
    assert.ok(!html.includes("'Native'"), "no Native control survives");
    assert.ok(!html.includes("fixSafe"), "safe-fix actions are not editor UI");
    assert.ok(!html.includes("data-section-target"), "no multi-section navigation survives");
  });
});
