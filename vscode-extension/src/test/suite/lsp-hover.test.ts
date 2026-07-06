// Implements [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
/**
 * HOVER HAMMER — exhaustive go-over of textDocument/hover through VS Code's
 * real command API against the live Basilisk LSP.
 *
 * Why this suite exists: hover was reported as INTERMITTENT in the shipped
 * extension (works for some symbol kinds, silently returns nothing for
 * others — e.g. module-level constants, see #199 / #200). A single happy-path
 * hover check (the old lsp-navigation/lsp-integration tests) cannot catch
 * that. So this suite hammers EVERY symbol kind a user can hover, asserting
 * both that a hover *appears* and that it carries the right content.
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 *   - The binary must be on PATH / discoverable or the suite fails hard.
 */

import * as assert from 'assert';
import type * as vscode from 'vscode';
import {
    SUITE_SETUP_TIMEOUT_MS,
    DIAGNOSTIC_TIMEOUT_MS,
    closeAllEditors,
    getHoverText,
    locate,
    openPythonFile,
    setupLspTestSuite,
    teardownLspTestSuite,
} from './test-helpers';
import { HELPER_FILENAME, HELPER_SOURCE, SUBJECT_SOURCE } from './nav-fixtures';

/** Per-test timeout: the poll budget plus headroom for a cold index. */
const HOVER_TEST_TIMEOUT_MS = DIAGNOSTIC_TIMEOUT_MS + 5_000;

/** Assert a hover both appeared and carries every expected fragment. */
function assertHover(label: string, text: string, fragments: readonly string[]): void {
    assert.notStrictEqual(
        text, '',
        `HOVER MISSING for ${label} — provider returned no content (intermittent-hover regression, see #200)`
    );
    for (const fragment of fragments) {
        assert.ok(
            text.includes(fragment),
            `Hover for ${label} should contain "${fragment}", got:\n${text}`
        );
    }
}

suite('LSP Hover Hammer', () => {
    let tmpDir: string;
    let uri: vscode.Uri;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const setup = await setupLspTestSuite('basilisk-hover-test-');
        tmpDir = setup.tmpDir;
        // Helper first so the subject file's import resolves cross-file.
        await openPythonFile(tmpDir, HELPER_FILENAME, HELPER_SOURCE);
        ({ uri } = await openPythonFile(tmpDir, 'hover_subject.py', SUBJECT_SOURCE));
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        teardownLspTestSuite(tmpDir);
    });

    test('hover: module-level Final constant (PI)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'PI'));
        assertHover('module constant PI', text, ['PI', 'Final']);
    });

    test('hover: module constant use inside a function (PI in scaled_area)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'PI', 1));
        assertHover('module constant PI use', text, ['PI', 'Final']);
    });

    test('hover: module-level plain variable (counter)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'counter'));
        assertHover('module variable counter', text, ['counter']);
    });

    test('hover: function definition name carries docstring', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'calculate', 0));
        assertHover('function def calculate', text, ['calculate', 'Compute the square of operand']);
    });

    test('hover: function reference (call site) carries signature', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'calculate', 1));
        assertHover('function call calculate', text, ['calculate', 'int']);
    });

    test('hover: function parameter (operand)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'operand', 0));
        assertHover('parameter operand', text, ['operand', 'int']);
    });

    test('hover: local variable inside a function (squared)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'squared', 0));
        assertHover('local variable squared', text, ['squared']);
    });

    test('hover: class definition name carries docstring', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'Widget', 0));
        assertHover('class def Widget', text, ['Widget', 'A configurable widget']);
    });

    test('hover: class reference (annotation site)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'Widget', 1));
        assertHover('class annotation Widget', text, ['Widget']);
    });

    test('hover: class reference (constructor call)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'Widget', 2));
        assertHover('class constructor Widget', text, ['Widget']);
    });

    test('hover: class attribute (width)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'width', 0));
        assertHover('attribute width', text, ['width', 'int']);
    });

    test('hover: method definition (resize) carries docstring', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'resize', 0));
        assertHover('method def resize', text, ['resize', 'Resize the widget by a factor']);
    });

    test('hover: method parameter (factor)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'factor', 0));
        assertHover('method parameter factor', text, ['factor', 'int']);
    });

    test('hover: imported symbol usage (helper_fn call)', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'helper_fn', 1));
        assertHover('imported helper_fn usage', text, ['helper_fn']);
    });

    test('hover: import statement line carries resolution info', async function () {
        this.timeout(HOVER_TEST_TIMEOUT_MS);
        const text = await getHoverText(uri, locate(SUBJECT_SOURCE, 'nav_helper', 0));
        assertHover('import of nav_helper', text, ['nav_helper']);
    });
});
