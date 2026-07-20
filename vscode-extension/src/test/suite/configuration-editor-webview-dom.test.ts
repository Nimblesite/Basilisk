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
import {
  DRIVER_PRELUDE,
  RESULT_TIMEOUT_MS,
  runScenario,
  ScenarioHost,
} from "./webview-dom-harness";

/** Rows are 112px tall (ROW_HEIGHT in the webview script); 3 rows visible. */
const VIEWPORT_HEIGHT_PX = 336;

/**
 * Drives the runtime like a user: select the first (pep) rule, scroll down in
 * wheel-sized increments toward the basilisk rules, click one, and report what
 * the RULE DETAIL panel shows.
 */
const detailDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    const heading = () => text(el('#detail-content h3')) || '';
    try {
      const viewport = document.getElementById('rule-viewport');
      viewport.style.height = '${VIEWPORT_HEIGHT_PX}px';
      viewport.style.minHeight = '${VIEWPORT_HEIGHT_PX}px';
      viewport.style.maxHeight = '${VIEWPORT_HEIGHT_PX}px';
      if (!await waitFor('[data-show-rule]', 200)) { report({ ok: false, reason: 'snapshot never rendered' }); return; }
      // 1. The user selects a pep rule; the occurrences round trip re-renders.
      const pepButton = el('[data-show-rule="pep_rule_000"]');
      pepButton.focus();
      pepButton.click();
      await sleep(250);
      const headingAfterPep = heading();
      // 2. The user scrolls toward the basilisk rules in wheel-sized steps.
      for (let stepIndex = 0; stepIndex < 30; stepIndex += 1) {
        viewport.scrollTop = viewport.scrollTop + 300;
        await sleep(40);
      }
      await sleep(150);
      const scrollTopAfterScroll = viewport.scrollTop;
      const maxScrollTop = viewport.scrollHeight - viewport.clientHeight;
      // 3. The user clicks the last basilisk rule.
      const bskButton = el('[data-show-rule="BSK-0005"]');
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
    } catch (error) { report({ ok: false, reason: String(error) }); }
  })();
`;

/**
 * The Configure Severity deep-link scenario ([CONFIGEDITOR-VSIX-EXPERIENCE]):
 * the state arrives with a focusRule; the runtime must prefill the search
 * filter with the code and open that rule's detail panel — no user input.
 */
const focusDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('[data-rule-code]', 200)) { report({ ok: false, reason: 'rules never rendered' }); return; }
      await sleep(300);
      report({
        ok: true,
        searchValue: document.getElementById('rule-search').value,
        filteredCount: text(document.getElementById('filter-result')),
        detailHeading: text(el('#detail-content h3')) || '',
      });
    } catch (error) { report({ ok: false, reason: String(error) }); }
  })();
`;

/**
 * One-shot semantics: focus is applied on the first snapshot render ONLY —
 * once the user edits the search, later host state pushes (still carrying the
 * same focusRule) must never stomp their filter or selection.
 */
const oneShotDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    try {
      if (!await waitFor('[data-rule-code]', 200)) { report({ ok: false, reason: 'rules never rendered' }); return; }
      await sleep(300);
      const searchValue = document.getElementById('rule-search').value;
      // The user retargets the filter to a pep rule...
      const search = document.getElementById('rule-search');
      search.value = 'pep_rule_001';
      search.dispatchEvent(new Event('input', { bubbles: true }));
      await sleep(100);
      // ...then the host pushes a later state still carrying the focusRule.
      window.__realApi.postMessage({ type: 'domTestSettle' });
      await sleep(300);
      report({
        ok: true,
        searchValue,
        searchAfterPush: document.getElementById('rule-search').value,
        filteredCount: text(document.getElementById('filter-result')),
        detailAfterPush: text(el('#detail-content h3')) || '',
      });
    } catch (error) { report({ ok: false, reason: String(error) }); }
  })();
`;

/**
 * Chrome scenario: a no-entry select must DISPLAY the resolved severity —
 * an untouched pep rule/tag reads Error, an untouched analyze rule its
 * effective value (Disabled for one that does not run), an untouched
 * non-pep tag Disabled — and only non-pep controls offer Disabled at all.
 */
const selectValueDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    const optionValues = (select) => Array.from(select.options).map((option) => option.value);
    try {
      if (!await waitFor('[data-rule-code]', 200)) { report({ ok: false, reason: 'rules never rendered' }); return; }
      const pepRule = el('select[data-rule-entry="pep_rule_000"]');
      const pepTag = el('select[data-tag-entry="pep"]');
      const basiliskTag = el('select[data-tag-entry="basilisk"]');
      // The Disabled analyze rule sits below the virtual window — filter to it.
      const search = document.getElementById('rule-search');
      search.value = 'BSK-0005';
      search.dispatchEvent(new Event('input', { bubbles: true }));
      await sleep(150);
      const disabledRule = el('select[data-rule-entry="BSK-0005"]');
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
    } catch (error) { report({ ok: false, reason: String(error) }); }
  })();
`;

/**
 * [CONFIGEDITOR-VSIX-EXPERIENCE]: the editor ships FIVE navigation views —
 * Overview, Rules, Adoption, Path Overrides, Project — and no Presets tab.
 * Every dashboard view renders exact server-computed snapshot state; this
 * driver switches views and reads back the real values it painted.
 */
const navPresenceDriver = String.raw`
  (async () => {
    ${DRIVER_PRELUDE}
    const sectionOf = (name) => el('[data-section="' + name + '"]');
    try {
      if (!await waitFor('[data-rule-code]', 200)) { report({ ok: false, reason: 'rules never rendered' }); return; }
      await sleep(100);
      const navLabels = all('#section-nav [data-section-target]')
        .map((button) => (text(button.querySelector('span:last-child')) || ''));
      // Overview: switch to it and read the exact server debt total it renders.
      await click(el('[data-section-target="overview"]'));
      const overviewVisible = !sectionOf('overview').hidden;
      const rulesHiddenOnOverview = sectionOf('rules').hidden;
      const remainingDebt = text(document.getElementById('overview-diagnostics'));
      // Path Overrides: read the discovered nested-config list + open action.
      await click(el('[data-section-target="paths"]'));
      const pathHeads = all('#path-override-list .path-override-card h3').map(text);
      const openConfigButtons = all('#path-override-list [data-open-config]').length;
      // Project: real source detail rows.
      await click(el('[data-section-target="project"]'));
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
        sourceRows: all('#source-details dt').length,
      });
    } catch (error) { report({ ok: false, reason: String(error) }); }
  })();
`;

suite("Configuration editor — rule detail panel in a real webview DOM", () => {
  // The reported bug: basilisk rules show no details — clicking one leaves the
  // detail panel on stale data from the previously selected (pep) rule,
  // because restoreFocus() yanks every scroll back to that rule's row.
  test("scrolling to and clicking a basilisk rule updates the rule detail panel", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const { result } = await runScenario(detailDriver, new ScenarioHost());
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.ok(
      String(result.headingAfterPep).includes("pep_rule_000"),
      `selecting a pep rule must populate the detail panel (got "${String(result.headingAfterPep)}")`,
    );
    assert.ok(
      result.bskRowRendered,
      "scrolling toward the basilisk rules must reach them — the viewport was yanked back to the "
      + `previously selected rule (scrollTop ${String(result.scrollTopAfterScroll)} of ${String(result.maxScrollTop)})`,
    );
    assert.ok(
      String(result.detailHeading).includes("BSK-0005"),
      `clicking a basilisk rule must show ITS detail, not stale data (panel shows "${String(result.detailHeading)}")`,
    );
  });

  // [CONFIGEDITOR-VSIX-EXPERIENCE]: the Configure Severity hover deep link —
  // a state carrying focusRule must open the editor "to the right place":
  // search prefilled with the code, the list filtered to it, and the rule's
  // detail panel open, all without any user interaction.
  test("a focusRule state opens the editor focused on that rule", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const { result } = await runScenario(focusDriver, new ScenarioHost({ focusRule: "BSK-0003" }));
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(
      result.searchValue,
      "BSK-0003",
      `the search filter must be prefilled with the focused rule code (got "${String(result.searchValue)}")`,
    );
    assert.ok(
      String(result.filteredCount).startsWith("1 "),
      `the rule list must be filtered to the focused rule (got "${String(result.filteredCount)}")`,
    );
    assert.ok(
      String(result.detailHeading).includes("BSK-0003"),
      `the focused rule's detail panel must open (panel shows "${String(result.detailHeading)}")`,
    );
  });

  // One-shot: the focus target is applied on the FIRST snapshot render only.
  // Later state pushes (occurrences round trips, refreshes) still carry the
  // focusRule — they must never stomp the user's own search or selection.
  test("a later state push never re-applies the consumed focusRule over the user's search", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const { result } = await runScenario(oneShotDriver, new ScenarioHost({ focusRule: "BSK-0003" }));
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(result.searchValue, "BSK-0003", "the deep link must focus first");
    assert.strictEqual(
      result.searchAfterPush,
      "pep_rule_001",
      `a later state push must keep the user's own filter (got "${String(result.searchAfterPush)}")`,
    );
    assert.ok(
      String(result.filteredCount).startsWith("1 "),
      `the list must stay filtered to the USER's query, not the focus target (got "${String(result.filteredCount)}")`,
    );
  });

  // A focus target the snapshot does not contain (stale hover, wrong server)
  // must be ignored gracefully: no crash, no vacuous filter, no detail panel.
  test("an unknown focusRule is ignored without filtering or crashing", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const { result } = await runScenario(focusDriver, new ScenarioHost({ focusRule: "BSK-9999" }));
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(
      result.searchValue,
      "",
      `an unknown focus target must not prefill the search (got "${String(result.searchValue)}")`,
    );
    assert.ok(
      !String(result.detailHeading).includes("BSK-9999"),
      `an unknown focus target must not open a detail panel (panel shows "${String(result.detailHeading)}")`,
    );
  });

  // [CHKARCH-CONFIG-MODEL] resolution shown honestly: a no-entry select must
  // DISPLAY what no entry resolves to — never a blank or a lying default.
  test("no-entry selects display the resolved severity (pep→Error, analyze→effective, non-pep tag→Disabled)", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const { result } = await runScenario(selectValueDriver, new ScenarioHost());
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.strictEqual(
      result.pepRuleSelect,
      "Error",
      `an untouched pep rule runs at error and its select must say so (got "${String(result.pepRuleSelect)}")`,
    );
    assert.strictEqual(
      result.pepRuleHasDisabledOption,
      false,
      "pep rule selects must not offer Disabled ([CHKARCH-CONFIG-MODEL])",
    );
    assert.strictEqual(
      result.disabledRuleSelect,
      "Disabled",
      `an untouched analyze rule that does not run must display Disabled (got "${String(result.disabledRuleSelect)}")`,
    );
    assert.strictEqual(
      result.disabledRuleHasDisabledOption,
      true,
      "analyze rule selects must offer Disabled",
    );
    assert.strictEqual(
      result.pepTagSelect,
      "Error",
      `an untouched pep tag grades at error and its select must say so (got "${String(result.pepTagSelect)}")`,
    );
    assert.strictEqual(
      result.basiliskTagSelect,
      "Disabled",
      `an untouched non-pep tag does not run and its select must say so (got "${String(result.basiliskTagSelect)}")`,
    );
  });
});

suite("Configuration editor — restored navigation views in a real webview DOM", () => {
  // The reported regression: the config editor lost every view except Rules.
  // The nav rail must offer all five views, each dashboard view must render
  // exact server-computed snapshot state, and there must be NO Presets tab.
  test("renders the five navigation views with real server data and no presets tab", async function () {
    this.timeout(RESULT_TIMEOUT_MS + 15_000);
    const { result } = await runScenario(navPresenceDriver, new ScenarioHost());
    assert.strictEqual(result.ok, true, `webview driver failed: ${result.reason ?? "unknown"}`);
    assert.deepStrictEqual(
      result.navLabels,
      ["Overview", "Rules", "Adoption", "Path Overrides", "Project"],
      "the nav rail must offer exactly the five restored views in order",
    );
    assert.ok(
      result.hasOverview === true && result.hasAdoption === true
      && result.hasPaths === true && result.hasProject === true,
      "all four restored view sections must exist in the DOM",
    );
    assert.strictEqual(result.hasPresets, false, "there is no Presets tab ([CHKARCH-CONFIGURATION-ONLY])");
    assert.strictEqual(result.overviewVisible, true, "selecting Overview must reveal its section");
    assert.strictEqual(result.rulesHiddenOnOverview, true, "selecting Overview must hide the Rules section");
    assert.strictEqual(result.remainingDebt, "795", "Overview renders the exact server debt total, not a synthetic score");
    assert.deepStrictEqual(result.pathHeads, ["legacy"], "Path Overrides lists the discovered nested config");
    assert.strictEqual(result.openConfigButtons, 1, "each path override exposes a real open-file action");
    assert.ok(Number(result.sourceRows ?? 0) >= 3, "the Project view renders the real source details");
  });
});
