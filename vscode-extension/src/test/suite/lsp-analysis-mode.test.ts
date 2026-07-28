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

import { delay } from '../../timeouts';
import * as assert from 'assert';
import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import {
    closeAllEditors,
    DIAGNOSTIC_TIMEOUT_MS,
    EXTENSION_ID,
    filterBasiliskDiagnostics,
    NO_DIAGNOSTIC_WAIT_MS,
    openPythonFile,
    removeTestDir,
    SERVER_START_WAIT_MS,
    SUITE_SETUP_TIMEOUT_MS,
    waitForDiagnostics,
    waitForDiagnosticsCleared,
} from './test-helpers';

/** Extra buffer (ms) added to test timeouts beyond core wait. */
const TIMEOUT_BUFFER_MS = 5_000;

/** Large buffer (ms) for tests with multiple diagnostic waits or startup delays. */
const LARGE_TIMEOUT_BUFFER_MS = 15_000;

/** Fodder files written into the workspace to slow the wholeModule scan enough
 *  that closing an editor reliably lands mid-scan (GitHub #264). */
const FODDER_FILE_COUNT = 400;

/** Wait (ms) after flipping to wholeModule before closing the editor — long
 *  enough for the didChangeConfiguration to reach the server and the scan to
 *  begin computing, short enough that the scan is still running. */
const SCAN_KICKOFF_WAIT_MS = 1_000;

/** Budget (ms) for the fodder marker file to receive its scan diagnostics —
 *  i.e. for the slowed-down workspace scan to complete and publish. */
const SCAN_COMPLETE_TIMEOUT_MS = 45_000;

/** Window (ms) after the scan completes during which stale diagnostics for the
 *  closed file must NOT reappear. The buggy republish trails the marker
 *  publish by milliseconds, so this is generous. */
const STALE_REPUBLISH_GRACE_MS = 3_000;

/** Fodder module: fully annotated, diagnostic-free, but real enough that the
 *  scan pays parse+check cost for each file. */
function fodderModule(moduleIndex: number): string {
    const lines: string[] = ['"""Scan fodder for the #264 stale-republish test."""', ''];
    for (let functionIndex = 0; functionIndex < 12; functionIndex += 1) {
        lines.push(
            `def fodder_${moduleIndex}_${functionIndex}(value: int) -> int:`,
            `    total: int = value + ${functionIndex}`,
            '    return total',
            ''
        );
    }
    return lines.join('\n');
}

/** Write FODDER_FILE_COUNT clean modules plus one erroring marker module into
 *  `fodderDir`. The marker's scan diagnostics signal "publish loop reached the
 *  scan portion" — open-file refresh entries publish after it. */
function writeScanFodder(fodderDir: string): vscode.Uri {
    fs.mkdirSync(fodderDir, { recursive: true });
    for (let moduleIndex = 0; moduleIndex < FODDER_FILE_COUNT; moduleIndex += 1) {
        const name = `fodder_${String(moduleIndex).padStart(3, '0')}.py`;
        fs.writeFileSync(path.join(fodderDir, name), fodderModule(moduleIndex), 'utf8');
    }
    const markerPath = path.join(fodderDir, 'zz_marker_264.py');
    fs.writeFileSync(markerPath, 'def marker(name):\n    return f"Hello, {name}!"\n', 'utf8');
    return vscode.Uri.file(markerPath);
}

/** Poll for `windowMs` asserting the URI's diagnostics stay at zero — catches
 *  a stale scan republish arriving after didClose cleared them (#264). */
async function assertDiagnosticsStayCleared(uri: vscode.Uri, windowMs: number): Promise<void> {
    const deadline = Date.now() + windowMs;
    while (Date.now() < deadline) {
        const diags = vscode.languages.getDiagnostics(uri);
        assert.strictEqual(
            diags.length,
            0,
            `stale diagnostics republished for closed file ${uri.fsPath} after ` +
            `didClose cleared them (GitHub #264): ${diags.map((d) => d.message).join('; ')}`
        );
        await delay(100);
    }
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
            removeTestDir(tmpDir);
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
            await delay(SERVER_START_WAIT_MS + TIMEOUT_BUFFER_MS);

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
            await delay(NO_DIAGNOSTIC_WAIT_MS);

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

    // Regression test for GitHub #264 — the root cause of the flaky
    // "openFilesOnly: opening a file produces diagnostics, closing clears them"
    // failure under full-suite load. A wholeModule scan snapshots open files
    // (refresh_open_files) and publishes them last; a didClose processed
    // between the scan's publishes clears the file and removes it from the
    // index, then the scan republishes the stale diagnostics — which nothing
    // ever clears again. Exercises the publish staleness guard in
    // crates/basilisk-lsp/src/server/init.rs ([ANALYSIS-PUBLISH]).
    test('wholeModule: file closed mid-scan must not get stale diagnostics republished (#264)', async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS + SCAN_COMPLETE_TIMEOUT_MS);

        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        assert.ok(workspaceRoot !== undefined, 'no workspace folder configured');

        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalMode = cfg.get<string>('analysisMode');
        const fodderDir = path.join(workspaceRoot, 'scan_fodder_264');

        try {
            // Start in openFilesOnly so the later flip to wholeModule triggers
            // a fresh workspace scan while our file is open.
            await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);
            await delay(1_000);

            // Slow the upcoming scan down and plant the completion marker.
            const markerUri = writeScanFodder(fodderDir);

            // Open an erroring file OUTSIDE the workspace root and wait for
            // its diagnostics (published by didOpen).
            const { uri } = await openPythonFile(
                tmpDir,
                'stale_republish_264.py',
                'def greet(name):\n    return f"Hello, {name}!"\n'
            );
            const openDiags = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.ok(openDiags.length > 0, 'file must have diagnostics while open');

            // Flip to wholeModule: the scan snapshot now includes the open
            // file. Give the config change time to reach the server and the
            // scan time to start computing…
            await cfg.update('analysisMode', 'wholeModule', vscode.ConfigurationTarget.Workspace);
            await delay(SCAN_KICKOFF_WAIT_MS);

            // …then close the editor while the scan is still running. The
            // server clears the file's diagnostics on didClose.
            await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
            const cleared = await waitForDiagnosticsCleared(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.strictEqual(cleared.length, 0, 'didClose must clear diagnostics');

            // Wait for the scan's publish loop to reach the scan portion (the
            // marker fodder file gets its diagnostics)…
            await waitForDiagnostics(markerUri, SCAN_COMPLETE_TIMEOUT_MS);

            // …and assert the closed file's stale diagnostics never come back.
            await assertDiagnosticsStayCleared(uri, STALE_REPUBLISH_GRACE_MS);
        } finally {
            removeTestDir(fodderDir);
            await cfg.update('analysisMode', originalMode, vscode.ConfigurationTarget.Workspace);
        }
    });
});
