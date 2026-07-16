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
}

/** A realistic snapshot: pep rules first, basilisk rules at the bottom. */
function fixtureSnapshot(): unknown {
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
    effectiveSeverity: { kind: "Error" },
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
  };
}

/**
 * In-page fake of the ConfigurationEditorController/store loop: answers the
 * runtime's `ready` and `occurrences` intents with the same state pushes the
 * real host produces, and keeps the REAL acquireVsCodeApi handle for the
 * driver to report results back to the extension host.
 */
function hostShimScript(): string {
  return `
    const __realApi = acquireVsCodeApi();
    window.__realApi = __realApi;
    const __snapshot = ${embedJson(fixtureSnapshot())};
    let __state = {
      phase: 'ready', rootUri: __snapshot.rootUri, snapshot: __snapshot,
      preview: undefined, occurrences: undefined, occurrencesLoading: false,
      repairUri: undefined, message: 'Configuration is up to date', refreshRequested: false,
    };
    const __push = (partial) => {
      __state = Object.assign({}, __state, partial);
      window.dispatchEvent(new MessageEvent('message', { data: { type: 'state', state: __state } }));
    };
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

/** Inject the shim before and the driver after the real runtime, same nonce. */
function harnessDocument(): string {
  const html = buildConfigurationEditorDocument();
  const openTag = /<script nonce="[^"]+">/.exec(html);
  assert.ok(openTag, "the configuration editor document must carry one nonce-gated script");
  return html
    .replace(openTag[0], `${openTag[0]}${hostShimScript()}\n;`)
    .replace("</script>\n</body>", `;\n${driverScript()}</script>\n</body>`);
}

async function runWebviewScenario(): Promise<DomTestResult> {
  const panel = vscode.window.createWebviewPanel(
    "basilisk.configurationEditorDomTest",
    "Configuration Editor DOM Test",
    vscode.ViewColumn.One,
    { enableScripts: true, localResourceRoots: [] },
  );
  try {
    return await new Promise<DomTestResult>((resolve, reject) => {
      const timer = setTimeout(() => {
        reject(new Error("the webview driver never reported a result"));
      }, RESULT_TIMEOUT_MS);
      panel.webview.onDidReceiveMessage((message: DomTestResult & { type?: string }) => {
        if (message.type === "domTestResult") {
          clearTimeout(timer);
          resolve(message);
        }
      });
      panel.webview.html = harnessDocument();
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
    const result = await runWebviewScenario();
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
});
