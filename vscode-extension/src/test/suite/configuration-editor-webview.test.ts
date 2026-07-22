// Implements [CONFIGEDITOR-ACCESSIBILITY-SECURITY] / [CONFIGEDITOR-VSIX-EXPERIENCE].

import * as assert from "assert";
import { buildConfigurationEditorDocument } from "../../configuration-editor-document";
import { decodeConfigurationEditorIntent } from "../../configuration-editor-intents";

suite("Configuration editor — untrusted intent decoder", () => {
  // [CONFIGEDITOR-MODEL]: the four rule/tag mutations and two allowlisted
  // Typeshed setting mutations are the complete write vocabulary.
  test("accepts the six EditorMutation kinds with typed values", () => {
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
    // [LSPCFGED-TYPESHED]: the three surviving keys are all text-typed, so the
    // model carries a bare String — `SetTypeshedSetting { key, value: String }`
    // in models/configuration_editor.td.
    for (const key of ["TypeshedPath", "TypeshedCommit", "TypeshedStorePath"]) {
      assert.strictEqual(decodeConfigurationEditorIntent({
        type: "preview",
        mutations: [{ kind: "SetTypeshedSetting", key: { kind: key }, value: "configured" }],
      })?.type, "preview", `SetTypeshedSetting ${key} must be accepted`);
      // The retired tagged value shape must be REJECTED, not quietly coerced:
      // accepting it would let the webview post a mutation the LSP cannot
      // apply, losing the user's edit with no error.
      assert.strictEqual(decodeConfigurationEditorIntent({
        type: "preview",
        mutations: [{ kind: "SetTypeshedSetting", key: { kind: key }, value: { kind: "Text", value: "configured" } }],
      }), undefined, `the retired tagged value shape must be rejected for ${key}`);
    }
    assert.strictEqual(decodeConfigurationEditorIntent({
      type: "preview", mutations: [{ kind: "RemoveTypeshedSetting", key: { kind: "TypeshedStorePath" } }],
    })?.type, "preview");
    for (const download of ["DownloadLatest", "DownloadPinned", "ViewLicense"]) {
      assert.strictEqual(
        decodeConfigurationEditorIntent({ type: "typeshedAction", action: download })?.type,
        "typeshedAction",
        `${download} is the complete action vocabulary`,
      );
    }
    for (const key of ["TypeshedPath", "TypeshedStorePath"]) {
      assert.strictEqual(decodeConfigurationEditorIntent({ type: "pickTypeshedFolder", key })?.type, "pickTypeshedFolder");
    }
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
    // [LSPCFGED-TYPESHED]: the cache/verify toggles, the cache-path key, and
    // the alternate-URL key are deleted from the contract entirely.
    for (const key of ["TypeshedCache", "TypeshedVerify", "TypeshedCachePath", "TypeshedUrl", "ArbitraryKey"]) {
      // The value is a VALID bare string, so the retired key is the only thing
      // that can cause the rejection — a malformed value would let this pass
      // even if the key check regressed.
      assert.strictEqual(decodeConfigurationEditorIntent({
        type: "preview",
        mutations: [{ kind: "SetTypeshedSetting", key: { kind: key }, value: "x" }],
      }), undefined, `${key} was removed from the contract`);
      assert.strictEqual(decodeConfigurationEditorIntent({
        type: "preview",
        mutations: [{ kind: "RemoveTypeshedSetting", key: { kind: key } }],
      }), undefined, `${key} must not be removable either`);
    }
    // A surviving key never accepts a boolean value.
    assert.strictEqual(decodeConfigurationEditorIntent({
      type: "preview",
      mutations: [{ kind: "SetTypeshedSetting", key: { kind: "TypeshedCommit" }, value: { kind: "Boolean", value: true } }],
    }), undefined);
    // The pin-current/acquire-fresh actions and the cache-path picker are gone.
    for (const legacyAction of ["PinCurrent", "AcquireFresh"]) {
      assert.strictEqual(
        decodeConfigurationEditorIntent({ type: "typeshedAction", action: legacyAction }),
        undefined,
        `${legacyAction} was removed from the contract`,
      );
    }
    assert.strictEqual(
      decodeConfigurationEditorIntent({ type: "pickTypeshedFolder", key: "TypeshedCachePath" }),
      undefined,
    );
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
  test("is CSP locked, theme-native, zoom resilient, never self-blocking, and keyboard traversable", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(html.includes("default-src 'none'"));
    assert.ok(/style-src 'nonce-[^']+'/.test(html));
    assert.ok(/script-src 'nonce-[^']+'/.test(html));
    assert.ok(!html.includes("unsafe-inline"));
    // `default-src 'none'` is the enforcement; assert the document also never
    // ASKS for a remote resource (an example URL in placeholder copy is not a
    // fetch, so match resource attributes rather than the bare scheme).
    assert.ok(
      !/(?:src|href)\s*=\s*["']https?:/i.test(html),
      "the shell must load no remote resources",
    );
    assert.ok(!/url\(\s*["']?https?:/i.test(html), "no stylesheet may fetch a remote asset");
    assert.ok(html.includes("img-src data:"), "images are inline data only");
    assert.ok(html.includes("var(--vscode-editor-background)"));
    assert.ok(html.includes("vscode-high-contrast"));
    assert.ok(html.includes("prefers-reduced-motion"));
    assert.ok(html.includes('id="announcer" class="sr-only" aria-live="polite"'));
    // The full-panel lock screen is DELETED ([LSPCFGED-TYPESHED-DOWNLOAD]):
    // no overlay node, no code that makes the shell inert, no modal state
    // card — editor lifecycle renders as the non-blocking inline notice.
    assert.ok(!html.includes("state-overlay"), "no full-panel overlay element may exist");
    assert.ok(!html.includes("inert"), "no code path may make the panel inert");
    assert.ok(!html.includes("aria-modal"), "the native impact dialog is the only modal surface");
    assert.ok(html.includes('id="state-notice" role="status"'), "lifecycle renders as an inline notice row");
    assert.ok(html.includes("max-height: calc(100vh - 32px)"));
    assert.ok(html.includes("function moveVirtualRuleFocus(event)"));
    assert.ok(html.includes('id="rule-spacer" role="list"'));
    assert.ok(html.includes("row.setAttribute('role', 'listitem')"));
    assert.ok(html.includes("row.setAttribute('aria-posinset'"));
    assert.ok(html.includes("row.setAttribute('aria-setsize'"));
    assert.ok(html.includes("['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End']"));
    assert.ok(html.includes("viewport.scrollTop = target * ROW_HEIGHT"));
  });

  // [LSPCFGED-TYPESHED] / [LSPCFGED-TYPESHED-DOWNLOAD]: two sources and no
  // third, download buttons instead of lifecycle locks, and none of the
  // deleted cache/verify/URL controls.
  test("ships the two-source Typeshed panel with download buttons and no deleted controls", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(html.includes("'Pinned commit'"), "the pinned-commit radio exists");
    assert.ok(html.includes("'Custom folder'"), "the custom-folder radio exists");
    assert.ok(!html.includes("'Latest'"), "no Latest source radio may ever render");
    assert.ok(html.includes("'DownloadLatest', 'Download latest'"), "Download latest is a real button");
    assert.ok(html.includes("'DownloadPinned', 'Download pinned'"), "Download pinned is the NO SOURCE fix");
    assert.ok(html.includes("typeshed-no-source"), "the missing source renders as an inline row");
    assert.ok(!html.includes("PinCurrent"), "the PinCurrent action is deleted");
    assert.ok(!html.includes("AcquireFresh"), "the AcquireFresh action is deleted");
    assert.ok(!html.includes("TypeshedCache"), "the cache toggle and cache path are deleted");
    assert.ok(!html.includes("TypeshedVerify"), "the verify toggle is deleted");
    assert.ok(!html.includes("TypeshedUrl"), "the alternate-URL setting is deleted");
    assert.ok(html.includes("TypeshedStorePath"), "the store folder picker remains under Advanced");
    assert.ok(html.includes("COMMIT_PATTERN"), "the 40-hex SHA gate remains client-side");
  });

  // [CONFIGEDITOR-VSIX-EXPERIENCE]: the tag-first Rules view is the whole
  // editor. Tag groups get the tag-entry control; rows get per-rule entry
  // controls; pep controls have no Disabled option ([CHKARCH-CONFIG-MODEL]).
  test("renders the tag-first Rules view with pep-gated entry controls", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(html.includes("select.dataset.tagEntry = tag.name"), "tag groups expose the tag-entry control");
    assert.ok(html.includes("select.dataset.ruleEntry = rule.descriptor.code"), "rows expose per-rule entry controls");
    assert.ok(html.includes("const PEP_TAG = 'pep'"));
    assert.ok(
      html.includes("SEVERITIES.filter((value) => value !== 'Disabled')"),
      "pep controls must offer error/warning/info and never Disabled",
    );
    assert.ok(html.includes("isPepRule(rule)"));
    assert.ok(html.includes("{ kind: 'SetRule', code, severity: { kind: value } }"));
    assert.ok(html.includes("{ kind: 'SetTag', tag, severity: { kind: value } }"));
    assert.ok(html.includes("Load more occurrences"));
    assert.ok(html.includes("Exact resolved changes"));
    assert.ok(html.includes("impactCell(impact.errorsBefore, impact.errorsAfter, 'errors')"));
    assert.ok(html.includes('id="rule-search"'), "search stays");
    assert.ok(html.includes("Open raw"));
  });

  // [CONFIGEDITOR-VSIX-EXPERIENCE] / [CHKARCH-CONFIG-MODEL]: an entry dropdown
  // lists concrete severities only. "No entry" duplicated Disabled — an analyze
  // rule or tag with no entry does not run (resolution step 3) — so the
  // redundant choice is gone from every select. Disabled is gone from every
  // pep-affecting control (pep rows, the pep source tag, PEP-category tags)
  // because no disable exists for pep rules.
  test("entry dropdowns never offer No entry and pep-affecting controls omit Disabled", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(
      !html.includes("[NO_ENTRY].concat"),
      "no dropdown may offer a No-entry option",
    );
    assert.ok(
      html.includes("severityOptions(isPepRule(rule))"),
      "rule rows must gate Disabled on pep provenance",
    );
    assert.ok(
      html.includes("severityOptions(isPepTag(tag))"),
      "tag controls must gate Disabled on pep-affecting tags",
    );
    assert.ok(
      !html.includes("'RemoveRule'"),
      "a dropdown change always writes a rule entry — never removes one",
    );
    assert.ok(
      !html.includes("'RemoveTag'"),
      "a dropdown change always writes a tag entry — never removes one",
    );
  });

  // [CONFIGEDITOR-VSIX-EXPERIENCE]: the five navigation views are server-data
  // projections; removed preset and Inherit/Native mutation concepts stay out.
  test("retains the five views without preset or Inherit/Native UI", () => {
    const html = buildConfigurationEditorDocument();
    assert.ok(html.includes("adoption"), "the Adoption view is present");
    assert.ok(html.includes("pathOverrides"), "the Path Overrides view is present");
    assert.ok(html.includes("data-section-target"), "multi-section navigation is present");
    assert.ok(!html.includes("preset"), "preset UI is deleted");
    assert.ok(!html.includes("'Inherit'"), "no Inherit control survives");
    assert.ok(!html.includes("'Native'"), "no Native control survives");
    assert.ok(html.includes("fixSafe"), "the standalone safe-fix action is available");
  });
});
