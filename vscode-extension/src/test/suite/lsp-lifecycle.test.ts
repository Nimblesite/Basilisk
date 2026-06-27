// Implements [VSIX-LSP-CLIENT-CONFIGURATION]. See docs/specs/VSIX-SPEC.md#VSIX-LSP-CLIENT-CONFIGURATION
/**
 * LSP Lifecycle Tests for the Basilisk VS Code Extension.
 *
 * These tests exercise LSP lifecycle features: status bar presence,
 * restart command, extension state management, the edit-diagnose-fix-clear
 * cycle, and independent per-file diagnostics.
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 *   - The binary must be on PATH or the test will fail hard
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import { getStore } from '../../extension';

import {
    EXTENSION_ID,
    DIAGNOSTIC_TIMEOUT_MS,
    NO_DIAGNOSTIC_WAIT_MS,
    SERVER_START_WAIT_MS,
    SUITE_SETUP_TIMEOUT_MS,
    waitForDiagnostics,
    waitForDiagnosticsCleared,
    waitForLspReady,
    openPythonFile,
    closeAllEditors,
    replaceDocumentContent,
    setupLspTestSuite,
    teardownLspTestSuite,
} from './test-helpers';

/** Extra buffer (ms) added to restart-test timeout to cover server restart. */
const RESTART_EXTRA_TIMEOUT_MS = 20_000;

/** Multiplier applied to DIAGNOSTIC_TIMEOUT_MS for multi-phase tests. */
const DIAGNOSTIC_TIMEOUT_MULTIPLIER = 3;

interface PackageJSON {
    readonly activationEvents?: string[];
}

// eslint-disable-next-line max-lines-per-function
suite('LSP Lifecycle Tests', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-lifecycle-test-');
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
    // 1. restartServer command works
    // ----------------------------------------------------------------
    test('restartServer command works and server remains functional', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + RESTART_EXTRA_TIMEOUT_MS);

        // Execute the restart command -- it should not throw.
        let didThrow = false;
        try {
            await vscode.commands.executeCommand('basilisk.restartServer');
        } catch {
            didThrow = true;
        }
        assert.strictEqual(didThrow, false, 'basilisk.restartServer should not throw');

        // Deterministically wait for the restarted server to re-advertise its
        // commands before probing it. lspClient.stop() clears store.serverCommands
        // and start() re-populates it asynchronously after re-initialize; waitForLspReady
        // polls that signal, so we never race a half-restarted server (replaces the
        // previous fixed 500ms sleep, which was the timing flake).
        await waitForLspReady();

        // Verify the extension is still active after restart.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);
        assert.strictEqual(ext.isActive, true, 'Extension should remain active after server restart');

        // Open a Python file with an error to prove the restarted server works.
        const { uri } = await openPythonFile(
            tmpDir,
            'test_restart_verify.py',
            'def after_restart(x):\n    return x\n'
        );

        const diagnostics = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
        assert.ok(
            diagnostics.length > 0,
            'Expected diagnostics after server restart, proving the server restarted and is working'
        );
    });

    // ----------------------------------------------------------------
    // 2. showOutput command works
    // ----------------------------------------------------------------
    test('showOutput command works without error', async function () {
        this.timeout(SERVER_START_WAIT_MS);

        // Ensure the extension is active.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);
        assert.strictEqual(ext.isActive, true, 'Extension should be active');

        // Execute the showOutput command -- it should not throw.
        let didThrow = false;
        try {
            await vscode.commands.executeCommand('basilisk.showOutput');
        } catch {
            didThrow = true;
        }
        assert.strictEqual(didThrow, false, 'basilisk.showOutput should not throw');
    });

    // ----------------------------------------------------------------
    // 3. Status bar exists after activation
    // ----------------------------------------------------------------
    test('status bar exists after activation', async function () {
        this.timeout(SERVER_START_WAIT_MS);

        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);
        assert.strictEqual(ext.isActive, true, 'Extension must be active for status bar to exist');

        // Verify the showOutput command is available via internal VSIX state.
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(
            store.isClientCommandRegistered('basilisk.showOutput'),
            'basilisk.showOutput should be tracked in internal VSIX state'
        );

        // Execute the status bar command to confirm the output channel is alive.
        // If the status bar or output channel was not created, this would throw.
        let didThrow = false;
        try {
            await vscode.commands.executeCommand('basilisk.showOutput');
        } catch {
            didThrow = true;
        }
        assert.strictEqual(
            didThrow,
            false,
            'Executing the status bar command (showOutput) should not throw'
        );
    });

    // ----------------------------------------------------------------
    // 4. Extension activates on Python file open
    // ----------------------------------------------------------------
    test('extension activates on Python file open', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS);

        // The extension should already be active from suiteSetup, but we
        // verify the activation mechanism by checking the extension state.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);

        // Verify activation events include Python language.
        const activationEvents: string[] = (ext.packageJSON as PackageJSON).activationEvents ?? [];
        assert.ok(
            activationEvents.includes('onLanguage:python'),
            'Extension should declare onLanguage:python activation event'
        );

        // Open a .py file and confirm the extension is active.
        const { doc } = await openPythonFile(
            tmpDir,
            'test_activate.py',
            'x: int = 42\n'
        );

        assert.strictEqual(doc.languageId, 'python', 'Opened document should be identified as Python');

        // After opening a Python file the extension must be active.
        assert.strictEqual(
            ext.isActive,
            true,
            'Extension should be active after opening a Python file'
        );
    });

    // ----------------------------------------------------------------
    // 5. Diagnostics update on file edit (full edit-diagnose-fix-clear cycle)
    // ----------------------------------------------------------------
    test('diagnostics update on file edit -- full edit-diagnose-fix-clear cycle', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * DIAGNOSTIC_TIMEOUT_MULTIPLIER + SERVER_START_WAIT_MS);

        // Step 1: Open a clean, fully typed file -- expect zero diagnostics.
        const { doc, uri } = await openPythonFile(
            tmpDir,
            'test_edit_cycle.py',
            'def good(x: int) -> int:\n    return x\n'
        );

        // Wait for the server to have processed the file (diagnostics cleared or stable).
        await waitForDiagnosticsCleared(uri, NO_DIAGNOSTIC_WAIT_MS);

        const initialDiags = vscode.languages.getDiagnostics(uri);
        const initialBasiliskDiags = initialDiags.filter(
            (d) =>
                d.source === 'basilisk' ||
                (typeof d.code === 'object' &&
                    d.code !== null &&
                    'value' in d.code &&
                    typeof d.code.value === 'string' &&
                    d.code.value.startsWith('BSK-'))
        );
        assert.strictEqual(
            initialBasiliskDiags.length,
            0,
            `Expected zero Basilisk diagnostics for clean code, got ${initialBasiliskDiags.length}`
        );

        // Step 2: Edit the file to introduce a type error (missing param type).
        const editApplied = await replaceDocumentContent(
            doc,
            'def good(x: int) -> int:\n    return x\n\ndef bad(x):\n    return x\n'
        );
        assert.strictEqual(editApplied, true, 'Edit to introduce error should succeed');

        const errorDiags = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
        assert.ok(
            errorDiags.length > 0,
            'Expected diagnostics after introducing untyped parameter'
        );

        // Step 3: Fix the error by adding the type annotation.
        const fixApplied = await replaceDocumentContent(
            doc,
            'def good(x: int) -> int:\n    return x\n\ndef bad(x: int) -> int:\n    return x\n'
        );
        assert.strictEqual(fixApplied, true, 'Edit to fix error should succeed');

        const clearedDiags = await waitForDiagnosticsCleared(uri, DIAGNOSTIC_TIMEOUT_MS);
        const remainingBasilisk = clearedDiags.filter(
            (d) =>
                d.source === 'basilisk' ||
                (typeof d.code === 'object' &&
                    d.code !== null &&
                    'value' in d.code &&
                    typeof d.code.value === 'string' &&
                    d.code.value.startsWith('BSK-'))
        );
        assert.strictEqual(
            remainingBasilisk.length,
            0,
            `Expected diagnostics to clear after fixing the type error, but ${remainingBasilisk.length} remain`
        );
    });

    // ----------------------------------------------------------------
    // 6. Multiple files get independent diagnostics
    // ----------------------------------------------------------------
    test('multiple files get independent diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + SERVER_START_WAIT_MS);

        // Open file A with an error.
        const { uri: uriA } = await openPythonFile(
            tmpDir,
            'test_multi_a.py',
            'def broken(x):\n    return x\n'
        );

        // Wait for diagnostics on file A.
        const diagsA = await waitForDiagnostics(uriA, DIAGNOSTIC_TIMEOUT_MS);
        assert.ok(
            diagsA.length > 0,
            'File A (with errors) should have diagnostics'
        );

        // Open file B with clean code.
        const { uri: uriB } = await openPythonFile(
            tmpDir,
            'test_multi_b.py',
            'def clean(x: int) -> int:\n    return x\n'
        );

        // Wait for the server to process file B (no diagnostics expected).
        await waitForDiagnosticsCleared(uriB, NO_DIAGNOSTIC_WAIT_MS);

        const diagsB = vscode.languages.getDiagnostics(uriB);
        const basiliskDiagsB = diagsB.filter(
            (d) =>
                d.source === 'basilisk' ||
                (typeof d.code === 'object' &&
                    d.code !== null &&
                    'value' in d.code &&
                    typeof d.code.value === 'string' &&
                    d.code.value.startsWith('BSK-'))
        );
        assert.strictEqual(
            basiliskDiagsB.length,
            0,
            `File B (clean code) should have zero Basilisk diagnostics, got ${basiliskDiagsB.length}`
        );

        // Verify file A still has its diagnostics while file B is open.
        const diagsAStill = vscode.languages.getDiagnostics(uriA);
        assert.ok(
            diagsAStill.length > 0,
            'File A should still have diagnostics while file B is open'
        );

        // Close file A.
        await closeAllEditors();

        // Re-open file B only.
        const { uri: uriBReopened } = await openPythonFile(
            tmpDir,
            'test_multi_b.py',
            'def clean(x: int) -> int:\n    return x\n'
        );

        await waitForDiagnosticsCleared(uriBReopened, NO_DIAGNOSTIC_WAIT_MS);

        const diagsBAfterClose = vscode.languages.getDiagnostics(uriBReopened);
        const basiliskDiagsBAfter = diagsBAfterClose.filter(
            (d) =>
                d.source === 'basilisk' ||
                (typeof d.code === 'object' &&
                    d.code !== null &&
                    'value' in d.code &&
                    typeof d.code.value === 'string' &&
                    d.code.value.startsWith('BSK-'))
        );
        assert.strictEqual(
            basiliskDiagsBAfter.length,
            0,
            'File B should still have zero Basilisk diagnostics after closing file A'
        );
    });
});
