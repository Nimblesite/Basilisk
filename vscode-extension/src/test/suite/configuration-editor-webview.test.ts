// Implements [CONFIGEDITOR-ACCESSIBILITY-SECURITY].

import * as assert from "assert";
import { buildConfigurationEditorDocument } from "../../configuration-editor-document";
import { decodeConfigurationEditorIntent } from "../../configuration-editor-intents";

suite("Configuration editor — untrusted intent decoder", () => {
  test("accepts every severity and reusable selector while rejecting malformed payloads", () => {
    for (const severity of ["Native", "Error", "Warning", "Info", "Disabled", "Inherit"]) {
      const decoded = decodeConfigurationEditorIntent({
        type: "preview",
        mutations: [{
          selector: { kind: "Codes", codes: ["B001"] },
          setting: { kind: severity },
          scope: { kind: "Path", pattern: "legacy/**" },
        }],
      });
      assert.strictEqual(decoded?.type, "preview", `${severity} must be accepted`);
    }
    for (const selector of [
      { kind: "All" },
      { kind: "Tags", tags: ["strictness"], matchAll: false },
      { kind: "CurrentViolations" },
      { kind: "SafeFixable" },
      { kind: "WithoutSafeFix" },
    ]) {
      assert.strictEqual(decodeConfigurationEditorIntent({
        type: "occurrences", selector, cursor: undefined, limit: 100,
      })?.type, "occurrences");
    }
    assert.strictEqual(decodeConfigurationEditorIntent({ type: "preview", mutations: [] }), undefined);
    assert.strictEqual(decodeConfigurationEditorIntent({ type: "occurrences", selector: { kind: "All" }, limit: 0 }), undefined);
  });
});

suite("Configuration editor — hardened, accessible document", () => {
  test("is CSP locked, tag-first, zoom resilient, inert while blocked, and keyboard traversable", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(html.includes("default-src 'none'"));
    assert.ok(/style-src 'nonce-[^']+'/.test(html));
    assert.ok(/script-src 'nonce-[^']+'/.test(html));
    assert.ok(!html.includes("unsafe-inline"));
    assert.ok(!html.includes("https://"), "the shell must load no remote resources");
    assert.ok(html.includes("var(--vscode-editor-background)"));
    assert.ok(html.includes("vscode-high-contrast"));
    assert.ok(html.includes("prefers-reduced-motion"));
    assert.ok(html.includes('data-section="rules" aria-labelledby="rules-title">'));
    assert.ok(html.includes('data-section-target="rules" aria-current="page"'));
    assert.ok(!html.includes("#rule-detail { display: none; }"));
    assert.ok(!html.includes("#tag-rail { display: none; }"));
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
    assert.ok(html.includes("lastFocusedRule = { code: filteredRules[target].descriptor.code, control }"));
  });

  test("shows exact preview changes, path inventory, paging, and honest adoption actions", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(html.includes("preset.summary"));
    assert.ok(html.includes('id="fix-safe-button"'));
    assert.ok(html.includes("snapshot.rules.reduce((total, rule) => total + rule.safeFixCount, 0)"));
    assert.ok(html.includes("Unsafe fixes are never included"));
    assert.ok(html.includes("Exact resolved changes"));
    assert.ok(html.includes("previous + ' → ' + result"));
    assert.ok(html.includes("postPreview({ kind: 'WithoutSafeFix' }, 'Disabled', projectScope())"));
    assert.ok(html.includes("has:diagnostics fix:without-safe"));
    assert.ok(html.includes("selector: { kind: 'WithoutSafeFix' }"));
    assert.ok(html.includes("Load more occurrences"));
    assert.ok(html.includes("snapshot.pathOverrides"));
    assert.ok(html.includes("Preview removing override"));
    assert.ok(html.includes("Open raw configuration"));
  });
});
