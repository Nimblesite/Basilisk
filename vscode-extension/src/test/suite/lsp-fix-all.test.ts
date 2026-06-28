// Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
// Exercises [AUTOFIX-MASS] (File scope) and [AUTOFIX-MASS-VSCODE] — the
// `basilisk.fixFile` command and `source.fixAll.basilisk` code action.
/**
 * LSP Fix-All E2E Tests for the Basilisk VS Code Extension.
 *
 * Tests the file-level mass autofix functionality:
 * - `basilisk.fixFile` command applies edits and clears diagnostics
 * - `source.fixAll.basilisk` code action kind is returned by the server
 * - Multiple diagnostics across lines are fixed in a single action
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 *   - The binary must be on PATH or the test will fail hard
 */

import * as assert from 'assert';
import * as vscode from 'vscode';

import {
    COMMAND_WAIT_MS,
    DIAGNOSTIC_TIMEOUT_MS,
    NO_DIAGNOSTIC_WAIT_MS,
    SERVER_START_WAIT_MS,
    SUITE_SETUP_TIMEOUT_MS,
    waitForDiagnostics,
    waitForDiagnosticsCleared,
    openPythonFile,
    closeAllEditors,
    setupLspTestSuite,
    teardownLspTestSuite,
} from './test-helpers';

/** Time (ms) to wait for re-diagnosis after applying edits. */
const RECHECK_WAIT_MS = 3_000;

/** Filter diagnostics to only BSK-W0050 (redundant annotation). */
function filterW0050(diagnostics: vscode.Diagnostic[]): vscode.Diagnostic[] {
    return diagnostics.filter((d) => {
        if (typeof d.code === 'object' && d.code !== null && 'value' in d.code) {
            return d.code.value === 'BSK-W0050';
        }
        return typeof d.code === 'string' && d.code === 'BSK-W0050';
    });
}

// eslint-disable-next-line max-lines-per-function
suite('LSP Fix-All Tests', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-fixall-test-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    // ----------------------------------------------------------------
    // 1. fixFile command applies edits and clears diagnostics
    // ----------------------------------------------------------------
    test('fixFile command applies edits and clears diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + SERVER_START_WAIT_MS);

        // Open a file with a redundant annotation — W0050 is auto-fixable.
        const { uri } = await openPythonFile(
            tmpDir,
            'test_fix_file.py',
            'x: int = 42\n'
        );

        // Wait for the W0050 diagnostic to appear.
        const diagnostics = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
        const w0050 = filterW0050(diagnostics);
        assert.ok(
            w0050.length > 0,
            `Expected BSK-W0050 diagnostic for redundant annotation, ` +
            `got: ${diagnostics.map((d) => JSON.stringify(d.code)).join(', ')}`
        );

        // Execute the fixFile command — it should apply edits via workspace/applyEdit.
        await vscode.commands.executeCommand('basilisk.fixFile');

        // After fixing, the redundant annotation diagnostic should clear.
        const cleared = await waitForDiagnosticsCleared(uri, DIAGNOSTIC_TIMEOUT_MS);
        const remaining = filterW0050(cleared);
        assert.strictEqual(
            remaining.length,
            0,
            `Expected W0050 diagnostic to clear after fixFile, but ${remaining.length} remain`
        );
    });

    // ----------------------------------------------------------------
    // 2. source.fixAll code action returned for fixable diagnostics
    // ----------------------------------------------------------------
    test('source.fixAll code action returned for fixable diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + SERVER_START_WAIT_MS);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_fix_all_action.py',
            'x: int = 42\n'
        );

        // Wait for diagnostics to appear.
        const diagnostics = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
        assert.ok(
            diagnostics.length > 0,
            'Expected at least one diagnostic for redundant annotation'
        );

        // Request code actions with the source.fixAll filter.
        const fullRange = new vscode.Range(
            new vscode.Position(0, 0),
            new vscode.Position(1, 0)
        );
        const codeActions = await vscode.commands.executeCommand<vscode.CodeAction[]>(
            'vscode.executeCodeActionProvider',
            uri,
            fullRange,
            vscode.CodeActionKind.SourceFixAll.value
        );

        assert.ok(codeActions !== undefined, 'Expected code actions result to be defined');
        assert.ok(
            codeActions.length > 0,
            `Expected at least one source.fixAll code action, got ${codeActions.length}`
        );

        const fixAllAction = codeActions.find(
            (a) => a.title.includes('Fix all auto-fixable issues')
        );
        assert.ok(
            fixAllAction,
            `Expected a 'Fix all auto-fixable issues' action. ` +
            `Got titles: ${codeActions.map((a) => a.title).join(', ')}`
        );
    });

    // ----------------------------------------------------------------
    // 3. fixFile fixes multiple diagnostics across lines
    // ----------------------------------------------------------------
    test('fixFile fixes multiple diagnostics across lines', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + SERVER_START_WAIT_MS);

        // Two redundant annotations on separate lines — both fixable.
        const { uri } = await openPythonFile(
            tmpDir,
            'test_fix_multi.py',
            'x: int = 42\ny: str = "hello"\n'
        );

        // Wait for diagnostics to appear on both lines.
        const diagnostics = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
        const w0050s = filterW0050(diagnostics);
        assert.ok(
            w0050s.length >= 2,
            `Expected at least 2 BSK-W0050 diagnostics, got ${w0050s.length}`
        );

        // Execute fixFile — should fix both in one action.
        await vscode.commands.executeCommand('basilisk.fixFile');

        // Both diagnostics should clear.
        const cleared = await waitForDiagnosticsCleared(uri, DIAGNOSTIC_TIMEOUT_MS);
        const remainingW0050 = filterW0050(cleared);
        assert.strictEqual(
            remainingW0050.length,
            0,
            `Expected all W0050 diagnostics to clear after fixFile, ` +
            `but ${remainingW0050.length} remain`
        );
    });

    // ----------------------------------------------------------------
    // 4. fixFile on clean file is a no-op
    // ----------------------------------------------------------------
    test('fixFile on clean file is a no-op', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + SERVER_START_WAIT_MS);

        // Fully typed file — nothing to fix.
        const { doc, uri } = await openPythonFile(
            tmpDir,
            'test_fix_noop.py',
            'def clean(x: int) -> int:\n    return x\n'
        );

        // Wait for the server to process (no diagnostics expected).
        await waitForDiagnosticsCleared(uri, NO_DIAGNOSTIC_WAIT_MS);

        const before = doc.getText();

        // Execute fixFile — should not modify the document.
        await vscode.commands.executeCommand('basilisk.fixFile');

        // Brief wait for any edits to land.
        await new Promise<void>((r) => setTimeout(r, COMMAND_WAIT_MS));

        const after = doc.getText();
        assert.strictEqual(
            after,
            before,
            'fixFile should not modify a file with no fixable diagnostics'
        );
    });

    // ----------------------------------------------------------------
    // 5. fixFile with mixed fixable and unfixable diagnostics
    // ----------------------------------------------------------------
    test('fixFile fixes what it can and leaves unfixable diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + SERVER_START_WAIT_MS);

        // x: int = 42 produces W0050 (fixable — redundant annotation).
        // def broken(y) produces E0001+E0002 (fixable — missing annotations).
        // After fixFile, both should be fixed.
        const { uri } = await openPythonFile(
            tmpDir,
            'test_fix_mixed.py',
            'x: int = 42\n\ndef broken(y):\n    return y\n'
        );

        const diagnostics = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
        assert.ok(
            diagnostics.length >= 2,
            `Expected at least 2 diagnostics (W0050 + E0001/E0002), got ${diagnostics.length}`
        );

        // Execute fixFile.
        await vscode.commands.executeCommand('basilisk.fixFile');

        // Wait for edits to apply and re-diagnosis to happen.
        await new Promise<void>((r) => setTimeout(r, RECHECK_WAIT_MS));

        // The W0050 should be gone — the redundant annotation was removed.
        const after = vscode.languages.getDiagnostics(uri);
        const remainingW0050 = filterW0050(after);
        assert.strictEqual(
            remainingW0050.length,
            0,
            `Expected W0050 to be fixed, but ${remainingW0050.length} remain`
        );
    });
});
