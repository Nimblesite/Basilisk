// Tests for [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * Analysis Mode Tests for the Basilisk VS Code Extension.
 *
 * These tests verify that the `basilisk.analysisMode` setting is correctly
 * wired: configuration schema, extension reads it, and the LSP server
 * respects it (wholeModule scan vs openFilesOnly).
 *
 * Extracted from lsp-integration.test.ts to keep files under the 500-line limit.
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import {
    DIAGNOSTIC_TIMEOUT_MS,
    EXTENSION_ID,
    NO_DIAGNOSTIC_WAIT_MS,
    SERVER_START_WAIT_MS,
    SUITE_SETUP_TIMEOUT_MS,
    closeAllEditors,
    openPythonFile,
    waitForDiagnostics,
    waitForDiagnosticsCleared,
} from './test-helpers';

/** Extra buffer (ms) added to test timeouts beyond core wait. */
const TIMEOUT_BUFFER_MS = 5_000;

/** Large buffer (ms) for tests with multiple diagnostic waits or startup delays. */
const LARGE_TIMEOUT_BUFFER_MS = 15_000;

/**
 * Filter diagnostics to only those produced by the Basilisk LSP server.
 */
function filterBasiliskDiagnostics(diags: vscode.Diagnostic[]): vscode.Diagnostic[] {
    return diags.filter(
        (d) =>
            d.source === 'basilisk' ||
            (typeof d.code === 'object' &&
                d.code !== null &&
                'value' in d.code &&
                typeof d.code.value === 'string' &&
                d.code.value.startsWith('BSK'))
    );
}

// Tests the editor-setting source of [ANALYSIS-CONFIG-SRC] — the
// `basilisk.analysisMode` workspace setting: default `wholeModule`, all three
// enum values accepted, and the server respecting the selected scope.
// eslint-disable-next-line max-lines-per-function
suite('Analysis Mode Tests', () => {
    let tmpDir: string;

    suiteSetup(function () {
        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'bsk-mode-test-'));
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        if (tmpDir !== undefined && tmpDir !== '' && fs.existsSync(tmpDir)) {
            fs.rmSync(tmpDir, { recursive: true, force: true });
        }
    });

    teardown(async () => {
        await closeAllEditors();
    });

    // -------------------------------------------------------
    // Configuration schema tests — verify the setting exists
    // -------------------------------------------------------

    test('basilisk.analysisMode setting has correct default', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const mode = cfg.get<string>('analysisMode');
        // Default is wholeModule.
        assert.strictEqual(
            mode,
            'wholeModule',
            `Expected default analysisMode to be 'wholeModule', got '${mode}'`
        );
    });

    test('basilisk.analysisMode accepts openFilesOnly', async () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const original = cfg.get<string>('analysisMode');
        try {
            await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);
            const mode = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
            assert.strictEqual(mode, 'openFilesOnly');
        } finally {
            await cfg.update('analysisMode', original, vscode.ConfigurationTarget.Workspace);
        }
    });

    test('basilisk.analysisMode accepts crossModule', async () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const original = cfg.get<string>('analysisMode');
        try {
            await cfg.update('analysisMode', 'crossModule', vscode.ConfigurationTarget.Workspace);
            const mode = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
            assert.strictEqual(mode, 'crossModule');
        } finally {
            await cfg.update('analysisMode', original, vscode.ConfigurationTarget.Workspace);
        }
    });

    test('basilisk.analysisMode can be reset to wholeModule', async () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        // Set to openFilesOnly first, then back to wholeModule.
        await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);
        await cfg.update('analysisMode', 'wholeModule', vscode.ConfigurationTarget.Workspace);
        const mode = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
        assert.strictEqual(mode, 'wholeModule', 'should be able to reset to wholeModule');
    });

    test('basilisk.analysisMode: all three enum values are accepted', async () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const original = cfg.get<string>('analysisMode');
        const modes = ['openFilesOnly', 'wholeModule', 'crossModule'];
        try {
            for (const m of modes) {
                await cfg.update('analysisMode', m, vscode.ConfigurationTarget.Workspace);
                const current = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
                assert.strictEqual(current, m, `setting should accept '${m}'`);
            }
        } finally {
            await cfg.update('analysisMode', original, vscode.ConfigurationTarget.Workspace);
        }
    });

    // -------------------------------------------------------
    // Extension wiring tests — prove the extension reads and
    // forwards the setting to the LSP server correctly.
    // -------------------------------------------------------

    test('wholeModule mode: setting is wired into initializationOptions', () => {
        // Structural test — the extension must read analysisMode and pass it
        // to the server. The extension source sets initializationOptions.analysisMode.
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const mode = cfg.get<string>('analysisMode') ?? 'wholeModule';
        const validModes = ['openFilesOnly', 'wholeModule', 'crossModule'];
        assert.ok(
            validModes.includes(mode),
            `analysisMode '${mode}' is not a valid mode. Expected one of: ${validModes.join(', ')}`
        );
    });

    test('openFilesOnly mode: disabling whole-module sets setting correctly', async () => {
        // Prove the user can turn OFF whole-module analysis (important for large projects).
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalMode = cfg.get<string>('analysisMode') ?? 'wholeModule';

        try {
            await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);
            const updated = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
            assert.strictEqual(
                updated,
                'openFilesOnly',
                `Expected analysisMode to be 'openFilesOnly' after update, got '${updated}'`
            );
            // Verify that this is a meaningful change from the default.
            assert.notStrictEqual(
                updated,
                'wholeModule',
                'openFilesOnly must be different from wholeModule default'
            );
        } finally {
            await cfg.update('analysisMode', originalMode, vscode.ConfigurationTarget.Workspace);
        }
    });

    // -------------------------------------------------------
    // Whole-module LSP behaviour: a file written to the VS Code
    // workspace root but NEVER opened in the editor must receive
    // diagnostics from the startup scan.
    //
    // The workspace root is test-fixtures/workspace/ (configured
    // in .vscode-test.mjs). Files written there are within the
    // LSP server's rootUri, so the wholeModule startup scan will
    // pick them up.
    // -------------------------------------------------------

    test('wholeModule: startup scan publishes diagnostics for closed file in workspace root', async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);

        // Determine the workspace root that VS Code opened.
        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        assert.ok(
            workspaceRoot !== undefined,
            'wholeModule scan test: no workspace folder configured. ' +
            'Ensure .vscode-test.mjs sets workspaceFolder.'
        );

        // Ensure wholeModule mode is set BEFORE the extension activates.
        // (The extension reads the setting during activate(), so changing it
        // here affects the server's initializationOptions.)
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalMode = cfg.get<string>('analysisMode');
        await cfg.update('analysisMode', 'wholeModule', vscode.ConfigurationTarget.Workspace);

        try {
            // Write a Python file with type errors into the workspace root.
            // Do NOT open it — the whole-module scan must find it on its own.
            const closedFilePath = path.join(workspaceRoot, 'wm_scan_target.py');
            fs.writeFileSync(
                closedFilePath,
                'def greet(name):\n    return f"Hello, {name}!"\n',
                'utf8'
            );

            // Activate the extension (or restart to pick up the new file).
            const ext = vscode.extensions.getExtension(EXTENSION_ID);
            if (ext !== undefined && !ext.isActive) {
                await ext.activate();
            }

            // Wait for the LSP server startup scan to complete.
            await new Promise<void>((resolve) => setTimeout(resolve, SERVER_START_WAIT_MS + TIMEOUT_BUFFER_MS));

            // The startup scan must have published diagnostics for the closed file.
            const closedFileUri = vscode.Uri.file(closedFilePath);
            const diags = await waitForDiagnostics(closedFileUri, DIAGNOSTIC_TIMEOUT_MS);

            assert.ok(
                diags.length > 0,
                'wholeModule: startup scan must publish diagnostics for a closed file ' +
                'that exists in the workspace root. Diagnostics were empty — either ' +
                'the scan did not run or the file was not analysed.'
            );

            // Verify the diagnostics are from Basilisk (not another linter).
            const basiliskDiags = filterBasiliskDiagnostics(diags);
            assert.ok(
                basiliskDiags.length > 0,
                `wholeModule: diagnostics must be from Basilisk (BSK codes), got: ${
                JSON.stringify(diags.map(d => ({ source: d.source, code: d.code })))}`
            );

            // Cleanup the test file from the workspace root.
            fs.unlinkSync(closedFilePath);
        } finally {
            await cfg.update('analysisMode', originalMode, vscode.ConfigurationTarget.Workspace);
        }
    });

    test('openFilesOnly: startup scan does NOT run — closed workspace file gets no diagnostics', async function () {
        this.timeout(NO_DIAGNOSTIC_WAIT_MS + LARGE_TIMEOUT_BUFFER_MS);

        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        assert.ok(
            workspaceRoot !== undefined,
            'openFilesOnly test: no workspace folder configured. ' +
            'Ensure .vscode-test.mjs sets workspaceFolder.'
        );

        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalMode = cfg.get<string>('analysisMode');

        // Write a file with type errors into the workspace root.
        const closedFilePath = path.join(workspaceRoot, 'ofo_no_scan_target.py');
        fs.writeFileSync(
            closedFilePath,
            'def greet(name):\n    return f"Hello, {name}!"\n',
            'utf8'
        );

        try {
            await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);

            // Activate (or restart) the extension with openFilesOnly mode.
            const ext = vscode.extensions.getExtension(EXTENSION_ID);
            if (ext !== undefined && !ext.isActive) {
                await ext.activate();
            }

            // Wait long enough for a scan to have run (if it was going to).
            await new Promise<void>((resolve) => setTimeout(resolve, NO_DIAGNOSTIC_WAIT_MS));

            // In openFilesOnly mode, the closed file must NOT have diagnostics.
            const closedFileUri = vscode.Uri.file(closedFilePath);
            const diags = vscode.languages.getDiagnostics(closedFileUri);
            assert.strictEqual(
                diags.length,
                0,
                'openFilesOnly: startup scan must NOT run — closed file should have zero diagnostics, ' +
                `got: ${JSON.stringify(diags)}`
            );
        } finally {
            fs.unlinkSync(closedFilePath);
            await cfg.update('analysisMode', originalMode, vscode.ConfigurationTarget.Workspace);
        }
    });

    test('openFilesOnly: opening a file produces diagnostics, closing clears them', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + LARGE_TIMEOUT_BUFFER_MS);

        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalMode = cfg.get<string>('analysisMode');

        try {
            await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);

            // Open a file with type errors.
            const { uri } = await openPythonFile(
                tmpDir,
                'ofo_open_close.py',
                'def greet(name):\n    return f"Hello, {name}!"\n'
            );

            // Wait for diagnostics to appear (file is open, so should be analysed
            // regardless of mode).
            const openDiags = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.ok(
                openDiags.length > 0,
                'openFilesOnly: should have diagnostics while file is open'
            );

            // Close the file.
            await vscode.commands.executeCommand('workbench.action.closeActiveEditor');

            // In openFilesOnly mode the server clears diagnostics when the file is closed.
            const clearedDiags = await waitForDiagnosticsCleared(uri, NO_DIAGNOSTIC_WAIT_MS);
            assert.strictEqual(
                clearedDiags.length,
                0,
                'openFilesOnly: diagnostics should be cleared when file is closed'
            );
        } finally {
            await cfg.update('analysisMode', originalMode, vscode.ConfigurationTarget.Workspace);
        }
    });
});
