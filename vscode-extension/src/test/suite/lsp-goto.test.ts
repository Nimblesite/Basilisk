// Implements [LSPARCH-FEATURES-DEFINITION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-DEFINITION
/**
 * GOTO HAMMER — exhaustive go-over of go-to-definition / -declaration /
 * -type-definition through VS Code's real command API against the live
 * Basilisk LSP.
 *
 * Why this suite exists: go-to-definition was reported as INTERMITTENT in the
 * shipped extension (jumps for some symbol kinds, silently no-ops for others —
 * see #200). The old single happy-path goto check (one function call) cannot
 * catch that. So this suite hammers EVERY navigable symbol kind — including
 * cross-file targets — asserting both that a location *comes back* and that it
 * lands on the EXACT file + line of the real definition (lines derived from
 * the fixture via `locate`, so they cannot drift).
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
    getNavLocations,
    locate,
    openPythonFile,
    setupLspTestSuite,
    teardownLspTestSuite,
} from './test-helpers';
import { HELPER_FILENAME, HELPER_SOURCE, SUBJECT_SOURCE } from './nav-fixtures';

/** Per-test timeout: the poll budget plus headroom for a cold index. */
const GOTO_TEST_TIMEOUT_MS = DIAGNOSTIC_TIMEOUT_MS + 5_000;

/** The exact file + line a definition is expected to land on. */
interface ExpectedTarget {
    readonly uri: vscode.Uri;
    readonly line: number;
}

/** Assert exactly one resolved location landing on the expected file + line. */
function assertLands(
    label: string,
    locations: readonly vscode.Location[],
    expected: ExpectedTarget,
): void {
    assert.ok(
        locations.length > 0,
        `NO DEFINITION for ${label} — provider returned nothing (intermittent-goto regression, see #200)`
    );
    const target = locations[0];
    assert.strictEqual(
        target.uri.toString(),
        expected.uri.toString(),
        `Definition for ${label} should resolve in ${expected.uri.fsPath}, got ${target.uri.fsPath}`
    );
    assert.strictEqual(
        target.range.start.line,
        expected.line,
        `Definition for ${label} should land on line ${expected.line}, got ${target.range.start.line}`
    );
}

suite('LSP Goto Hammer', () => {
    let tmpDir: string;
    let uri: vscode.Uri;
    let helperUri: vscode.Uri;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const setup = await setupLspTestSuite('basilisk-goto-test-');
        tmpDir = setup.tmpDir;
        // Helper first so the subject file's import resolves cross-file.
        ({ uri: helperUri } = await openPythonFile(tmpDir, HELPER_FILENAME, HELPER_SOURCE));
        ({ uri } = await openPythonFile(tmpDir, 'goto_subject.py', SUBJECT_SOURCE));
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        teardownLspTestSuite(tmpDir);
    });

    // ── Same-file definitions ────────────────────────────────────────

    test('goto-def: function call resolves to its def', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'calculate', 1)
        );
        assertLands('function call calculate', locations, { uri, line: locate(SUBJECT_SOURCE, 'calculate', 0).line });
    });

    test('goto-def: class annotation resolves to its class def', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'Widget', 1)
        );
        assertLands('class annotation Widget', locations, { uri, line: locate(SUBJECT_SOURCE, 'Widget', 0).line });
    });

    test('goto-def: class constructor resolves to its class def', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'Widget', 2)
        );
        assertLands('class constructor Widget', locations, { uri, line: locate(SUBJECT_SOURCE, 'Widget', 0).line });
    });

    test('goto-def: local variable use resolves to its assignment', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'squared', 1)
        );
        assertLands('local variable squared', locations, { uri, line: locate(SUBJECT_SOURCE, 'squared', 0).line });
    });

    test('goto-def: parameter use resolves to the parameter', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        // occurrence 2 = first use in `squared = operand * operand` (occ 1 is the docstring word).
        const locations = await getNavLocations(
            'vscode.executeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'operand', 2)
        );
        assertLands('parameter operand', locations, { uri, line: locate(SUBJECT_SOURCE, 'operand', 0).line });
    });

    test('goto-def: attribute use resolves to the attribute def', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'width', 1)
        );
        assertLands('attribute width', locations, { uri, line: locate(SUBJECT_SOURCE, 'width', 0).line });
    });

    // ── Cross-file definitions ───────────────────────────────────────

    test('goto-def: imported function usage resolves cross-file', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'helper_fn', 1)
        );
        assertLands('imported helper_fn', locations, { uri: helperUri, line: locate(HELPER_SOURCE, 'helper_fn', 0).line });
    });

    test('goto-def: imported class usage resolves cross-file', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'HelperClass', 1)
        );
        assertLands('imported HelperClass', locations, { uri: helperUri, line: locate(HELPER_SOURCE, 'HelperClass', 0).line });
    });

    // ── Declaration provider ─────────────────────────────────────────

    test('goto-declaration: function call resolves to its def', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeDeclarationProvider', uri, locate(SUBJECT_SOURCE, 'calculate', 1)
        );
        assertLands('declaration of calculate', locations, { uri, line: locate(SUBJECT_SOURCE, 'calculate', 0).line });
    });

    // ── Type-definition provider ─────────────────────────────────────

    test('goto-type-def: variable resolves to its class type', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeTypeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'gadget', 0)
        );
        assertLands('type of gadget', locations, { uri, line: locate(SUBJECT_SOURCE, 'Widget', 0).line });
    });

    test('goto-type-def: variable resolves to cross-file class type', async function () {
        this.timeout(GOTO_TEST_TIMEOUT_MS);
        const locations = await getNavLocations(
            'vscode.executeTypeDefinitionProvider', uri, locate(SUBJECT_SOURCE, 'instance', 0)
        );
        assertLands('type of instance', locations, { uri: helperUri, line: locate(HELPER_SOURCE, 'HelperClass', 0).line });
    });
});
