// Tests for [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * End-to-end regression for the "Type Checking" toggle (`basilisk.enabled`).
 *
 * GitHub #65 / #119. This drives a REAL VS Code window and a REAL Basilisk LSP
 * (not a mock, not a direct `executeCommand("basilisk.toggleFeature")` poke):
 * it flips the actual `basilisk.enabled` setting the toggle writes, then asserts
 * the observable downstream effect a user sees — Basilisk diagnostics clear from
 * the editor when type checking is disabled and return when it is re-enabled.
 *
 * The toggle kept getting reported as broken because previous "fixes" were
 * validated by static code reads / mock-level tests that only checked the row
 * label flipped to "Disabled" (issue #65 comment). Those never proved the
 * diagnostics actually cleared. This test does. Implements the
 * [EXTACT-INFO-FEATURE-STATUS] "Type Checking" effect and the client half of
 * [ANALYSIS-ENABLED].
 */

import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import * as vscode from 'vscode';
import {
    DIAGNOSTIC_TIMEOUT_MS,
    closeAllEditors,
    openPythonFile,
    waitForDiagnostics,
    waitForDiagnosticsCleared,
} from './test-helpers';

/** A snippet that produces Basilisk diagnostics in the test workspace. */
const ERRORING_SOURCE = 'def greet(name):\n    return f"Hello, {name}!"\n';

/** Buffer (ms) added on top of the multiple diagnostic waits this suite makes. */
const TIMEOUT_BUFFER_MS = 20_000;

suite('Type Checking Toggle (basilisk.enabled)', () => {
    let tmpDir: string;

    suiteSetup(() => {
        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'bsk-enabled-test-'));
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

    test('disabling clears Basilisk diagnostics; re-enabling restores them', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 3 + TIMEOUT_BUFFER_MS);

        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalEnabled = cfg.get<boolean>('enabled');

        try {
            // Start from a known-enabled state.
            await cfg.update('enabled', true, vscode.ConfigurationTarget.Workspace);

            // Open an erroring file — diagnostics must appear while enabled.
            const { uri } = await openPythonFile(tmpDir, 'type_checking_toggle.py', ERRORING_SOURCE);
            const openDiags = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.ok(
                openDiags.length > 0,
                'precondition: Basilisk diagnostics must be present while type checking is enabled'
            );

            // Flip the Type Checking toggle OFF (the setting the panel writes).
            await cfg.update('enabled', false, vscode.ConfigurationTarget.Workspace);

            // The whole point of the toggle (#119): diagnostics must clear.
            const cleared = await waitForDiagnosticsCleared(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.strictEqual(
                cleared.length,
                0,
                'disabling Type Checking must clear Basilisk diagnostics from the editor (#119)'
            );

            // Flip it back ON — diagnostics must return (the toggle is reversible).
            await cfg.update('enabled', true, vscode.ConfigurationTarget.Workspace);
            const restored = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.ok(
                restored.length > 0,
                're-enabling Type Checking must restore Basilisk diagnostics'
            );
        } finally {
            await cfg.update('enabled', originalEnabled, vscode.ConfigurationTarget.Workspace);
            await closeAllEditors();
        }
    });
});
