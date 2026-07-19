// Tests [CONFIGEDITOR-VSIX-EXPERIENCE] webview runtime behaviour in a REAL
// webview DOM. See docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-VSIX-EXPERIENCE.
//
// Regression test for the stale RULE DETAIL panel: after selecting a rule,
// every virtualized re-render called restoreFocus() without preventScroll,
// which yanked the viewport back to the previously selected rule on every
// scroll frame. Basilisk (BSK-*) rules sit below the pep rules in the catalog,
// so they could never be scrolled to or clicked — the detail panel displayed
// stale data from the previously selected rule forever.
//
// String-containment tests (configuration-editor-webview.test.ts) cannot catch
// this: the bug is an interaction between focus(), scroll anchoring, and the
// row rebuild — so this suite runs the real CSP-locked document inside a real
// VS Code webview (Chromium) and drives it like a user.

import * as assert from "assert";
import * as vscode from "vscode";
import { buildConfigurationEditorDocument } from "../../configuration-editor-document";
import { embedJson } from "../../profiler-webview";

/** Rows are 112px tall (ROW_HEIGHT in the webview script); 3 rows visible. */
const VIEWPORT_HEIGHT_PX = 336;
const PEP_RULE_COUNT = 40;
const BASILISK_RULE_COUNT = 5;
const RESULT_TIMEOUT_MS = 30_000;

interface DomTestResult {
  readonly ok: boolean;
  readonly reason?: string;
  readonly headingAfterPep?: string;
  readonly scrollTopAfterScroll?: number;
  readonly maxScrollTop?: number;
  readonly bskRowRendered?: boolean;
  readonly detailHeading?: string;
  readonly searchValue?: string;
  readonly filteredCount?: string;
  readonly searchAfterPush?: string;
  readonly detailAfterPush?: string;
  readonly pepRuleSelect?: string;
  readonly pepRuleHasDisabledOption?: boolean;
  readonly disabledRuleSelect?: string;
  readonly disabledRuleHasDisabledOption?: boolean;
  readonly pepTagSelect?: string;
  readonly basiliskTagSelect?: string;
  readonly navLabels?: string[];
  readonly hasOverview?: boolean;
  readonly hasAdoption?: boolean;
  readonly hasPaths?: boolean;
  readonly hasProject?: boolean;
  readonly hasPresets?: boolean;
  readonly overviewVisible?: boolean;
  readonly rulesHiddenOnOverview?: boolean;
  readonly remainingDebt?: string;
  readonly pathHeads?: string[];
  readonly openConfigButtons?: number;
  readonly sourceRows?: number;
  readonly typeshedSourceDisabled?: boolean;
  readonly typeshedSettingCount?: number;
  readonly disabledTypeshedSettingCount?: number;
  readonly typeshedActionCount?: number;
  readonly disabledTypeshedActionCount?: number;
}

/** A realistic snapshot: pep rules first, basilisk rules at the bottom. */
function fixtureSnapshot(typeshedAcquiring = false): unknown {
  const pepRules = Array.from({ length: PEP_RULE_COUNT }, (_ignored, index) => ({
    descriptor: {
      code: `pep_rule_${String(index).padStart(3, "0")}`,
      title: `PEP rule ${index}`,
      summary: `Summary for pep rule ${index}`,
      tags: ["pep", "generics"],
      docsUrl: `https://www.basilisk-python.dev/errors/pep-${index}`,
    },
    entry: undefined,
    effectiveSeverity: { kind: "Error" },
    diagnosticCount: index,
  }));
  const basiliskRules = Array.from({ length: BASILISK_RULE_COUNT }, (_ignored, index) => ({
    descriptor: {
      code: `BSK-${String(index + 1).padStart(4, "0")}`,
      title: `Basilisk rule ${index + 1}`,
      summary: `Summary for basilisk rule ${index + 1}`,
      tags: ["basilisk", "strictness"],
      docsUrl: `https://www.basilisk-python.dev/errors/BSK-${String(index + 1).padStart(4, "0")}`,
    },
    entry: undefined,
    // The last basilisk rule resolves to Disabled ([CHKARCH-CONFIG-MODEL]
    // step 3: an untouched analyze rule does not run) — the no-entry select
    // display-value test reads it.
    effectiveSeverity: index === BASILISK_RULE_COUNT - 1 ? { kind: "Disabled" } : { kind: "Error" },
    diagnosticCount: index + 1,
  }));
  return {
    rootUri: "file:///workspace/project",
    configUri: "file:///workspace/project/pyproject.toml",
    revision: "fnv1a64:test",
    rules: [...pepRules, ...basiliskRules],
    tags: [
      { name: "basilisk", kind: { kind: "Provenance" }, entry: undefined, ruleCount: BASILISK_RULE_COUNT, diagnosticCount: 15 },
      { name: "pep", kind: { kind: "Provenance" }, entry: undefined, ruleCount: PEP_RULE_COUNT, diagnosticCount: 780 },
    ],
    source: { uri: "file:///workspace/project/pyproject.toml", exists: true, readOnly: false },
    pathOverrides: [
      {
        path: "legacy",
        configUri: "file:///workspace/project/legacy/pyproject.toml",
        rules: [{ code: "BSK-0001", severity: { kind: "Warning" } }],
        tags: [],
      },
    ],
    debt: {
      remainingDiagnostics: 795,
      errorDiagnostics: 780,
      warningDiagnostics: 15,
      infoDiagnostics: 0,
      adoptedRules: 0,
      disabledRules: 1,
    },
    problems: [],
    typeshed: {
      sourceMode: { kind: "Latest" },
      sourceOptions: [
        { mode: { kind: "Latest" }, label: "Latest", enabled: !typeshedAcquiring },
        { mode: { kind: "ExactCommit" }, label: "Exact commit", enabled: !typeshedAcquiring },
        { mode: { kind: "CustomFolder" }, label: "Custom folder", enabled: !typeshedAcquiring },
      ],
      settings: [
        { key: { kind: "TypeshedPath" }, label: "Custom folder", description: "Custom Typeshed path", widget: { kind: "Directory" }, enabled: !typeshedAcquiring },
        { key: { kind: "TypeshedCommit" }, label: "Exact commit", description: "Exact Typeshed commit", widget: { kind: "Text" }, enabled: !typeshedAcquiring },
        { key: { kind: "TypeshedUrl" }, label: "Alternate archive URL", description: "Typeshed archive mirror", widget: { kind: "Text" }, enabled: !typeshedAcquiring },
        { key: { kind: "TypeshedCachePath" }, label: "Cache folder", description: "Typeshed cache path", widget: { kind: "Directory" }, enabled: !typeshedAcquiring },
        { key: { kind: "TypeshedCache" }, label: "Reuse downloads", description: "Reuse cached Typeshed", defaultValue: { kind: "Boolean", value: true }, widget: { kind: "Boolean" }, enabled: !typeshedAcquiring },
        { key: { kind: "TypeshedVerify" }, label: "Verify content", description: "Verify downloaded Typeshed", defaultValue: { kind: "Boolean", value: true }, widget: { kind: "Boolean" }, enabled: !typeshedAcquiring },
      ],
      actions: [
        { action: { kind: "PinCurrent" }, label: "Pin current", enabled: !typeshedAcquiring },
        { action: { kind: "AcquireFresh" }, label: "Acquire fresh", enabled: !typeshedAcquiring },
        { action: { kind: "ViewLicense" }, label: "View License", enabled: !typeshedAcquiring },
      ],
      status: {
        lifecycle: { kind: typeshedAcquiring ? "Acquiring" : "Ready" },
        blockedReason: undefined,
        activeSource: { kind: "Bundled" },
        commitIdentity: "83c2518a9e6abbda0c44592c3483de459198f887",
        treeIdentity: undefined,
        transport: { kind: "EmbeddedZip" },
        licenseStatus: { kind: "Approved" },
        licenseReference: "https://example.test/LICENSE",
        provenance: { kind: "BundleVetted" },
        signedRelease: false,
        warnings: [],
      },
    },
  };
}

/**
 * In-page fake of the ConfigurationEditorController/store loop: answers the
 * runtime's `ready` and `occurrences` intents with the same state pushes the
 * real host produces, and keeps the REAL acquireVsCodeApi handle for the
 * driver to report results back to the extension host.
 */
function hostShimScript(focusRule: string | null = null, typeshedAcquiring = false): string {
  return `
    const __realApi = acquireVsCodeApi();
    window.__realApi = __realApi;
    // Boot beacon + page-error reporting: lets the extension-side test tell
    // "webview never loaded" apart from "driver hung" and surfaces script
    // errors that would otherwise silently eat the result.
    __realApi.postMessage({ type: 'domTestBoot' });
    window.addEventListener('error', (event) => {
      __realApi.postMessage({ type: 'domTestResult', ok: false, reason: 'page error: ' + event.message });
    });
    const __snapshot = ${embedJson(fixtureSnapshot(typeshedAcquiring))};
    let __state = {
      phase: 'ready', rootUri: __snapshot.rootUri, snapshot: __snapshot,
      preview: undefined, occurrences: undefined, occurrencesLoading: false,
      repairUri: undefined, message: 'Configuration is up to date', refreshRequested: false,
      focusRule: ${embedJson(focusRule)},
    };
    const __push = (partial) => {
      __state = Object.assign({}, __state, partial);
      window.dispatchEvent(new MessageEvent('message', { data: { type: 'state', state: __state } }));
    };
    // Drivers re-push state to simulate later host updates (one-shot focus).
    window.__push = __push;
    window.acquireVsCodeApi = () => ({
      postMessage(message) {
        if (message.type === 'ready') {
          setTimeout(() => __push({}), 0);
        } else if (message.type === 'occurrences') {
          const code = message.selector && message.selector.codes ? message.selector.codes[0] : '';
          setTimeout(() => __push({ occurrences: undefined, occurrencesLoading: true }), 0);
          setTimeout(() => __push({
            occurrences: {
              items: [{
                code,
                uri: 'file:///workspace/project/app.py',
                range: { start: { line: 1, character: 0 }, end: { line: 1, character: 4 } },
                severity: { kind: 'Error' },
              }],
              nextCursor: undefined,
            },
            occurrencesLoading: false,
          }), 30);
        }
      },
      getState() { return undefined; },
      setState() {},
    });
  `;
}

/**
 * Drives the runtime like a user: select the first (pep) rule, scroll down in
 * wheel-sized increments toward the basilisk rules, click one, and report what
 * the RULE DETAIL panel shows.
 */
function driverScript(): string {
  return `
    (async () => {
      const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
      const heading = () => {
        const node = document.querySelector('#detail-content h3');
        return node && node.textContent ? node.textContent : '';
      };
      const report = (result) => window.__realApi.postMessage(Object.assign({ type: 'domTestResult' }, result));
      try {
        const viewport = document.getElementById('rule-viewport');
        viewport.style.height = '${VIEWPORT_HEIGHT_PX}px';
        viewport.style.minHeight = '${VIEWPORT_HEIGHT_PX}px';
        viewport.style.maxHeight = '${VIEWPORT_HEIGHT_PX}px';
        let waited = 0;
        while (!document.querySelector('[data-show-rule]') && waited < 200) { await sleep(25); waited += 1; }
        if (!document.querySelector('[data-show-rule]')) {
          report({ ok: false, reason: 'snapshot never rendered' });
          return;
        }
        // 1. The user selects a pep rule; the occurrences round trip re-renders.
        const pepButton = document.querySelector('[data-show-rule="pep_rule_000"]');
        pepButton.focus();
        pepButton.click();
        await sleep(250);
        const headingAfterPep = heading();
        // 2. The user scrolls toward the basilisk rules in wheel-sized steps.
        for (let step = 0; step < 30; step += 1) {
          viewport.scrollTop = viewport.scrollTop + 300;
          await sleep(40);
        }
        await sleep(150);
        const scrollTopAfterScroll = viewport.scrollTop;
        const maxScrollTop = viewport.scrollHeight - viewport.clientHeight;
        // 3. The user clicks the last basilisk rule.
        const bskButton = document.querySelector('[data-show-rule="BSK-0005"]');
        if (bskButton) {
          bskButton.focus();
          bskButton.click();
          await sleep(250);
        }
        report({
          ok: true,
          headingAfterPep,
          scrollTopAfterScroll,
          maxScrollTop,
          bskRowRendered: bskButton !== null,
          detailHeading: heading(),
        });
      } catch (error) {
        report({ ok: false, reason: String(error) });
      }
    })();
  `;
}

/**
 * The Configure Severity deep-link scenario ([CONFIGEDITOR-VSIX-EXPERIENCE]):
 * the state arrives with a focusRule; the runtime must prefill the search
 * filter with the code and open that rule's detail panel — no user input.
 */
function focusDriverScript(): string {
  return `
    (async () => {
      const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
      const heading = () => {
        const node = document.querySelector('#detail-content h3');
        return node && node.textContent ? node.textContent : '';
      };
      const report = (result) => window.__realApi.postMessage(Object.assign({ type: 'domTestResult' }, result));
      try {
        let waited = 0;
        while (!document.querySelector('[data-rule-code]') && waited < 200) { await sleep(25); waited += 1; }
        // Allow the occurrences round trip triggered by showRule to settle.
        await sleep(300);
        report({
          ok: true,
          searchValue: document.getElementById('rule-search').value,
          filteredCount: document.getElementById('filter-result').textContent,
          detailHeading: heading(),
        });
      } catch (error) {
        report({ ok: false, reason: String(error) });
      }
    })();
  `;
}

/**
 * One-shot semantics: focus is applied on the first snapshot render ONLY —
 * once the user edits the search, later host state pushes (still carrying the
 * same focusRule) must never stomp their filter or selection.
 */
function oneShotDriverScript(): string {
  return `
    (async () => {
      const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
      const heading = () => {
        const node = document.querySelector('#detail-content h3');
        return node && node.textContent ? node.textContent : '';
      };
      const report = (result) => window.__realApi.postMessage(Object.assign({ type: 'domTestResult' }, result));
      try {
        let waited = 0;
        while (!document.querySelector('[data-rule-code]') && waited < 200) { await sleep(25); waited += 1; }
        await sleep(300);
        const searchValue = document.getElementById('rule-search').value;
        // The user retargets the filter to a pep rule...
        const search = document.getElementById('rule-search');
        search.value = 'pep_rule_001';
        search.dispatchEvent(new Event('input', { bubbles: true }));
        await sleep(100);
        // ...then the host pushes a later state still carrying the focusRule.
        window.__push({});
        await sleep(250);
        report({
          ok: true,
          searchValue,
          searchAfterPush: document.getElementById('rule-search').value,
          filteredCount: document.getElementById('filter-result').textContent,
          detailAfterPush: heading(),
        });
      } catch (error) {
        report({ ok: false, reason: String(error) });
      }
    })();
  `;
}

/**
 * Chrome scenario: a no-entry select must DISPLAY the resolved severity —
 * an untouched pep rule/tag reads Error, an untouched analyze rule its
 * effective value (Disabled for one that does not run), an untouched
 * non-pep tag Disabled — and only non-pep controls offer Disabled at all.
 */
function selectValueDriverScript(): string {
  return `
    (async () => {
      const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
      const report = (result) => window.__realApi.postMessage(Object.assign({ type: 'domTestResult' }, result));
      const optionValues = (select) => Array.from(select.options).map((option) => option.value);
      try {
        let waited = 0;
        while (!document.querySelector('[data-rule-code]') && waited < 200) { await sleep(25); waited += 1; }
        const pepRule = document.querySelector('select[data-rule-entry="pep_rule_000"]');
        const pepTag = document.querySelector('select[data-tag-entry="pep"]');
        const basiliskTag = document.querySelector('select[data-tag-entry="basilisk"]');
        // The Disabled analyze rule sits below the virtual window — filter to it.
        const search = document.getElementById('rule-search');
        search.value = 'BSK-0005';
        search.dispatchEvent(new Event('input', { bubbles: true }));
        await sleep(150);
        const disabledRule = document.querySelector('select[data-rule-entry="BSK-0005"]');
        if (!pepRule || !pepTag || !basiliskTag || !disabledRule) {
          report({ ok: false, reason: 'expected entry selects did not render' });
          return;
        }
        report({
          ok: true,
          pepRuleSelect: pepRule.value,
          pepRuleHasDisabledOption: optionValues(pepRule).includes('Disabled'),
          disabledRuleSelect: disabledRule.value,
          disabledRuleHasDisabledOption: optionValues(disabledRule).includes('Disabled'),
          pepTagSelect: pepTag.value,
          basiliskTagSelect: basiliskTag.value,
        });
      } catch (error) {
        report({ ok: false, reason: String(error) });
      }
    })();
  `;
}

/** Inject the shim before and the driver after the real runtime, same nonce. */
/**
 * [CONFIGEDITOR-VSIX-EXPERIENCE]: the editor ships FIVE navigation views —
 * Overview, Rules, Adoption, Path Overrides, Project — and no Presets tab.
 * Every dashboard view renders exact server-computed snapshot state; this
 * driver switches views and reads back the real values it painted.
 */
function navPresenceDriverScript(): string {
  return `
    (async () => {
      const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
      const report = (result) => window.__realApi.postMessage(Object.assign({ type: 'domTestResult' }, result));
      const sectionOf = (name) => document.querySelector('[data-section="' + name + '"]');
      try {
        let waited = 0;
        while (!document.querySelector('[data-rule-code]') && waited < 200) { await sleep(25); waited += 1; }
        await sleep(100);
        const navLabels = Array.from(document.querySelectorAll('#section-nav [data-section-target]'))
          .map((button) => (button.querySelector('span:last-child')?.textContent || '').trim());
        // Overview: switch to it and read the exact server debt total it renders.
        document.querySelector('[data-section-target="overview"]').click();
        await sleep(60);
        const overviewVisible = !sectionOf('overview').hidden;
        const rulesHiddenOnOverview = sectionOf('rules').hidden;
        const remainingDebt = document.getElementById('overview-diagnostics').textContent;
        // Path Overrides: read the discovered nested-config list + open action.
        document.querySelector('[data-section-target="paths"]').click();
        await sleep(60);
        const pathHeads = Array.from(document.querySelectorAll('#path-override-list .path-override-card h3'))
          .map((node) => node.textContent);
        const openConfigButtons = document.querySelectorAll('#path-override-list [data-open-config]').length;
        // Project: real source detail rows.
        document.querySelector('[data-section-target="project"]').click();
        await sleep(60);
        const sourceRows = document.querySelectorAll('#source-details dt').length;
        report({
          ok: true,
          navLabels,
          hasOverview: !!sectionOf('overview'),
          hasAdoption: !!sectionOf('adoption'),
          hasPaths: !!sectionOf('paths'),
          hasProject: !!sectionOf('project'),
          hasPresets: !!sectionOf('presets'),
          overviewVisible,
          rulesHiddenOnOverview,
          remainingDebt,
          pathHeads,
          openConfigButtons,
          sourceRows,
        });
      } catch (error) {
        report({ ok: false, reason: String(error) });
      }
    })();
  `;
}

/** [LSPCFGED-TYPESHED]: every Typeshed control is inert during acquisition. */
function typeshedAcquiringDriverScript(): string {
  return `
    (async () => {
      const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
      const report = (result) => window.__realApi.postMessage(Object.assign({ type: 'domTestResult' }, result));
      try {
        let waited = 0;
        while (!document.getElementById('typeshed-source-mode') && waited < 200) { await sleep(25); waited += 1; }
        const source = document.getElementById('typeshed-source-mode');
        const settings = Array.from(document.querySelectorAll(
          '[data-typeshed-text], [data-typeshed-boolean], [data-pick-typeshed-folder]'
        ));
        const actions = Array.from(document.querySelectorAll('[data-typeshed-action]'));
        report({
          ok: true,
          typeshedSourceDisabled: source ? source.disabled : false,
          typeshedSettingCount: settings.length,
          disabledTypeshedSettingCount: settings.filter((control) => control.disabled).length,
          typeshedActionCount: actions.length,
          disabledTypeshedActionCount: actions.filter((control) => control.disabled).length,
        });
      } catch (error) {
        report({ ok: false, reason: String(error) });
      }
    })();
  `;
}

function harnessDocument(
  driver: string,
  focusRule: string | null = null,
  typeshedAcquiring = false,
): string {
  const html = buildConfigurationEditorDocument();
  const openTag = /<script nonce="[^"]+">/.exec(html);
  assert.ok(openTag, "the configuration editor document must carry one nonce-gated script");
  return html
    .replace(openTag[0], `${openTag[0]}${hostShimScript(focusRule, typeshedAcquiring)}\n;`)
    .replace("</script>\n</body>", `;\n${driver}</script>\n</body>`);
}

async function runWebviewScenario(document: string): Promise<DomTestResult> {
  // A hidden webview gets its timers throttled and requestAnimationFrame
  // paused, which starves both the driver and the virtualized rule window —
  // start from a clean editor area so the panel is frontmost and stays so.
  await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  const panel = vscode.window.createWebviewPanel(
    "basilisk.configurationEditorDomTest",
    "Configuration Editor DOM Test",
    vscode.ViewColumn.One,
    { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] },
  );
  try {
    return await new Promise<DomTestResult>((resolve, reject) => {
      let booted = false;
      const timer = setTimeout(() => {
        reject(new Error(
          "the webview driver never reported a result "
          + `(boot beacon ${booted ? "received" : "missing"}; panel visible=${panel.visible}, active=${panel.active})`,
        ));
      }, RESULT_TIMEOUT_MS);
      panel.webview.onDidReceiveMessage((message: DomTestResult & { type?: string }) => {
        if (message.type === "domTestBoot") {
          booted = true;
        } else if (message.type === "domTestResult") {
          clearTimeout(timer);
          resolve(message);
        }
      });
      panel.webview.html = document;
    });
  } finally {
    panel.dispose();
  }
}

suite("Configuration editor — rule detail panel in a real webview DOM", () => {
  // The reported bug: basilisk rules show no details — clicking one leaves the
  // detail panel on stale data from the previously selected (pep) rule,
  // because restoreFocus() yanks every scroll back to that rule's row.
  test("scrolling to and clicking a basilisk rule updates the rule detail panel", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const result = await runWebviewScenario(harnessDocument(driverScript()));
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.ok(
      result.headingAfterPep?.includes("pep_rule_000"),
      `selecting a pep rule must populate the detail panel (got "${result.headingAfterPep}")`,
    );
    assert.ok(
      result.bskRowRendered,
      "scrolling toward the basilisk rules must reach them — the viewport was yanked back to the "
      + `previously selected rule (scrollTop ${result.scrollTopAfterScroll} of ${result.maxScrollTop})`,
    );
    assert.ok(
      result.detailHeading?.includes("BSK-0005"),
      `clicking a basilisk rule must show ITS detail, not stale data (panel shows "${result.detailHeading}")`,
    );
  });

  // [CONFIGEDITOR-VSIX-EXPERIENCE]: the Configure Severity hover deep link —
  // a state carrying focusRule must open the editor "to the right place":
  // search prefilled with the code, the list filtered to it, and the rule's
  // detail panel open, all without any user interaction.
  test("a focusRule state opens the editor focused on that rule", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const result = await runWebviewScenario(harnessDocument(focusDriverScript(), "BSK-0003"));
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(
      result.searchValue,
      "BSK-0003",
      `the search filter must be prefilled with the focused rule code (got "${result.searchValue}")`,
    );
    assert.ok(
      result.filteredCount?.startsWith("1 "),
      `the rule list must be filtered to the focused rule (got "${result.filteredCount}")`,
    );
    assert.ok(
      result.detailHeading?.includes("BSK-0003"),
      `the focused rule's detail panel must open (panel shows "${result.detailHeading}")`,
    );
  });

  // One-shot: the focus target is applied on the FIRST snapshot render only.
  // Later state pushes (occurrences round trips, refreshes) still carry the
  // focusRule — they must never stomp the user's own search or selection.
  test("a later state push never re-applies the consumed focusRule over the user's search", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const result = await runWebviewScenario(harnessDocument(oneShotDriverScript(), "BSK-0003"));
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(result.searchValue, "BSK-0003", "the deep link must focus first");
    assert.strictEqual(
      result.searchAfterPush,
      "pep_rule_001",
      `a later state push must keep the user's own filter (got "${result.searchAfterPush}")`,
    );
    assert.ok(
      result.filteredCount?.startsWith("1 "),
      `the list must stay filtered to the USER's query, not the focus target (got "${result.filteredCount}")`,
    );
  });

  // A focus target the snapshot does not contain (stale hover, wrong server)
  // must be ignored gracefully: no crash, no vacuous filter, no detail panel.
  test("an unknown focusRule is ignored without filtering or crashing", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const result = await runWebviewScenario(harnessDocument(focusDriverScript(), "BSK-9999"));
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(
      result.searchValue,
      "",
      `an unknown focus target must not prefill the search (got "${result.searchValue}")`,
    );
    assert.ok(
      !(result.detailHeading ?? "").includes("BSK-9999"),
      `an unknown focus target must not open a detail panel (panel shows "${result.detailHeading}")`,
    );
  });

  // [CHKARCH-CONFIG-MODEL] resolution shown honestly: a no-entry select must
  // DISPLAY what no entry resolves to — never a blank or a lying default.
  test("no-entry selects display the resolved severity (pep→Error, analyze→effective, non-pep tag→Disabled)", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const result = await runWebviewScenario(harnessDocument(selectValueDriverScript()));
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(
      result.pepRuleSelect,
      "Error",
      `an untouched pep rule runs at error and its select must say so (got "${result.pepRuleSelect}")`,
    );
    assert.strictEqual(
      result.pepRuleHasDisabledOption,
      false,
      "pep rule selects must not offer Disabled ([CHKARCH-CONFIG-MODEL])",
    );
    assert.strictEqual(
      result.disabledRuleSelect,
      "Disabled",
      `an untouched analyze rule that does not run must display Disabled (got "${result.disabledRuleSelect}")`,
    );
    assert.strictEqual(
      result.disabledRuleHasDisabledOption,
      true,
      "analyze rule selects must offer Disabled",
    );
    assert.strictEqual(
      result.pepTagSelect,
      "Error",
      `an untouched pep tag grades at error and its select must say so (got "${result.pepTagSelect}")`,
    );
    assert.strictEqual(
      result.basiliskTagSelect,
      "Disabled",
      `an untouched non-pep tag does not run and its select must say so (got "${result.basiliskTagSelect}")`,
    );
  });
});

suite("Configuration editor — restored navigation views in a real webview DOM", () => {
  // The reported regression: the config editor lost every view except Rules.
  // The nav rail must offer all five views, each dashboard view must render
  // exact server-computed snapshot state, and there must be NO Presets tab.
  test("renders the five navigation views with real server data and no presets tab", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const result = await runWebviewScenario(harnessDocument(navPresenceDriverScript()));
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.deepStrictEqual(
      result.navLabels,
      ["Overview", "Rules", "Adoption", "Path Overrides", "Project"],
      "the nav rail must offer exactly the five restored views in order",
    );
    assert.ok(
      result.hasOverview && result.hasAdoption && result.hasPaths && result.hasProject,
      "all four restored view sections must exist in the DOM",
    );
    assert.strictEqual(result.hasPresets, false, "there is no Presets tab ([CHKARCH-CONFIGURATION-ONLY])");
    assert.strictEqual(result.overviewVisible, true, "selecting Overview must reveal its section");
    assert.strictEqual(result.rulesHiddenOnOverview, true, "selecting Overview must hide the Rules section");
    assert.strictEqual(result.remainingDebt, "795", "Overview renders the exact server debt total, not a synthetic score");
    assert.deepStrictEqual(result.pathHeads, ["legacy"], "Path Overrides lists the discovered nested config");
    assert.strictEqual(result.openConfigButtons, 1, "each path override exposes a real open-file action");
    assert.ok((result.sourceRows ?? 0) >= 3, "the Project view renders the real source details");
  });
});

suite("Configuration editor — Typeshed acquisition in a real webview DOM", () => {
  test("disables the source selector, all six settings, and every action while acquiring", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const result = await runWebviewScenario(
      harnessDocument(typeshedAcquiringDriverScript(), null, true),
    );
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(result.typeshedSourceDisabled, true, "the source selector must be disabled");
    assert.strictEqual(result.typeshedSettingCount, 6, "the fixture must render all six settings");
    assert.strictEqual(
      result.disabledTypeshedSettingCount,
      result.typeshedSettingCount,
      "every Typeshed setting must be disabled",
    );
    assert.strictEqual(result.typeshedActionCount, 3, "the fixture must render all three actions");
    assert.strictEqual(
      result.disabledTypeshedActionCount,
      result.typeshedActionCount,
      "every Typeshed action must be disabled",
    );
  });
});
