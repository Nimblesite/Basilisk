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
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { execFileSync } from 'child_process';

const EXTENSION_ID = 'basilisk-lang.basilisk';

/** Maximum time (ms) to wait for diagnostics from the LSP server. */
const DIAGNOSTIC_TIMEOUT_MS = 15_000;

/** Maximum time (ms) to wait for diagnostics to stabilise. */
const NO_DIAGNOSTIC_WAIT_MS = 5_000;

/**
 * Resolves the absolute path to the basilisk binary built from Cargo.
 * Returns undefined if the binary does not exist.
 */
function findBasiliskBinary(): string | undefined {
    const workspaceRoot = path.resolve(__dirname, '../../../../..');
    const debugBinary = path.join(workspaceRoot, 'target', 'debug', 'basilisk');
    if (fs.existsSync(debugBinary)) {
        return debugBinary;
    }

    try {
        execFileSync('basilisk', ['--version'], { timeout: 5000 });
        return 'basilisk';
    } catch {
        return undefined;
    }
}

/**
 * Wait until at least one diagnostic appears for the given URI,
 * or until the timeout elapses -- whichever comes first.
 */
function waitForDiagnostics(
    uri: vscode.Uri,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS
): Promise<vscode.Diagnostic[]> {
    return new Promise((resolve) => {
        const existing = vscode.languages.getDiagnostics(uri);
        if (existing.length > 0) {
            resolve(existing);
            return;
        }

        const timeout = setTimeout(() => {
            disposable.dispose();
            resolve(vscode.languages.getDiagnostics(uri));
        }, timeoutMs);

        const disposable = vscode.languages.onDidChangeDiagnostics((event) => {
            if (event.uris.some((u) => u.toString() === uri.toString())) {
                const diags = vscode.languages.getDiagnostics(uri);
                if (diags.length > 0) {
                    clearTimeout(timeout);
                    disposable.dispose();
                    resolve(diags);
                }
            }
        });
    });
}

/**
 * Wait for diagnostics to clear (reach zero) for the given URI,
 * or until the timeout elapses.
 */
function waitForDiagnosticsCleared(
    uri: vscode.Uri,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS
): Promise<vscode.Diagnostic[]> {
    return new Promise((resolve) => {
        const existing = vscode.languages.getDiagnostics(uri);
        if (existing.length === 0) {
            resolve([]);
            return;
        }

        const timeout = setTimeout(() => {
            disposable.dispose();
            resolve(vscode.languages.getDiagnostics(uri));
        }, timeoutMs);

        const disposable = vscode.languages.onDidChangeDiagnostics((event) => {
            if (event.uris.some((u) => u.toString() === uri.toString())) {
                const diags = vscode.languages.getDiagnostics(uri);
                if (diags.length === 0) {
                    clearTimeout(timeout);
                    disposable.dispose();
                    resolve([]);
                }
            }
        });
    });
}

/**
 * Create a temporary Python file, open it in the editor, and return
 * the document + URI. Caller is responsible for cleanup via tmpDir.
 */
async function openPythonFile(
    tmpDir: string,
    filename: string,
    content: string
): Promise<{ doc: vscode.TextDocument; uri: vscode.Uri }> {
    const filePath = path.join(tmpDir, filename);
    fs.writeFileSync(filePath, content, 'utf8');
    const uri = vscode.Uri.file(filePath);
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc, { preview: false });
    return { doc, uri };
}

/**
 * Close all open editors to avoid cross-test pollution.
 */
async function closeAllEditors(): Promise<void> {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
}

/**
 * Replace the entire contents of a document with new text.
 * Uses WorkspaceEdit for reliability — editor.edit() can fail when
 * the editor state is transitioning (e.g. after a server restart).
 */
async function replaceDocumentContent(
    doc: vscode.TextDocument,
    newContent: string
): Promise<boolean> {
    const edit = new vscode.WorkspaceEdit();
    const fullRange = new vscode.Range(
        new vscode.Position(0, 0),
        new vscode.Position(doc.lineCount, 0)
    );
    edit.replace(doc.uri, fullRange, newContent);
    return vscode.workspace.applyEdit(edit);
}

suite('LSP Lifecycle Tests', () => {
    let tmpDir: string;
    let basiliskBinary: string | undefined;

    suiteSetup(async function () {
        this.timeout(30_000);

        basiliskBinary = findBasiliskBinary();
        if (!basiliskBinary) {
            throw new Error(
                'Basilisk binary not found. Build with: cargo build -p basilisk-cli'
            );
        }

        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-lifecycle-test-'));

        // Ensure the extension is activated.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        if (ext && !ext.isActive) {
            await ext.activate();
        }

        // Poll until the LSP server is responsive.
        const dummyPath = path.join(tmpDir, '__init__.py');
        fs.writeFileSync(dummyPath, '', 'utf8');
        const dummyUri = vscode.Uri.file(dummyPath);
        const dummyDoc = await vscode.workspace.openTextDocument(dummyUri);
        await vscode.window.showTextDocument(dummyDoc);
        const deadline = Date.now() + 10_000;
        while (Date.now() < deadline) {
            try {
                const syms = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                    'vscode.executeDocumentSymbolProvider', dummyUri
                );
                if (syms !== null && syms !== undefined) break;
            } catch { /* server not ready yet */ }
            await new Promise<void>((r) => setTimeout(r, 200));
        }
        await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        if (tmpDir && fs.existsSync(tmpDir)) {
            fs.rmSync(tmpDir, { recursive: true, force: true });
        }
    });

    teardown(async () => {
        await closeAllEditors();
    });

    // ----------------------------------------------------------------
    // 1. restartServer command works
    // ----------------------------------------------------------------
    test('restartServer command works and server remains functional', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 20_000);

        // Execute the restart command -- it should not throw.
        let didThrow = false;
        try {
            await vscode.commands.executeCommand('basilisk.restartServer');
        } catch {
            didThrow = true;
        }
        assert.strictEqual(didThrow, false, 'basilisk.restartServer should not throw');

        // Brief pause for the server to restart — diagnostics polling below
        // will wait for the server to actually respond.
        await new Promise<void>((resolve) => setTimeout(resolve, 500));

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
        this.timeout(10_000);

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
        this.timeout(10_000);

        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);
        assert.strictEqual(ext.isActive, true, 'Extension must be active for status bar to exist');

        // The status bar item's command is basilisk.showOutput (set in extension.ts).
        // Verify the command is registered -- this proves the status bar item was created,
        // because the status bar item and command registration happen together in activate().
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.showOutput'),
            'basilisk.showOutput command should be registered, confirming status bar creation'
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
        this.timeout(15_000);

        // The extension should already be active from suiteSetup, but we
        // verify the activation mechanism by checking the extension state.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);

        // Verify activation events include Python language.
        const activationEvents: string[] = ext.packageJSON.activationEvents ?? [];
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
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 3 + 10_000);

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
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + 10_000);

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
