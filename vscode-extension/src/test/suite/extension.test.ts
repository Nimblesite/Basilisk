// Tests for [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
import { delay } from '../../timeouts';
import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import { getStore } from '../../extension';
import { POLL_INTERVAL_MS, WAIT_MS } from "./test-helpers";

const EXTENSION_ID = 'Nimblesite.basilisk';

interface PackageJSON {
    displayName: string;
    activationEvents?: string[];
}

// eslint-disable-next-line max-lines-per-function
suite('Basilisk Extension E2E Tests', () => {

    suiteSetup(async () => {
        // Ensure the extension is activated by opening a Python file.
        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? __dirname;
        const pyFilePath = path.join(workspaceRoot, '__basilisk_test__.py');
        const pyUri = vscode.Uri.file(pyFilePath);

        // Create a minimal Python file to trigger activation.
        await vscode.workspace.fs.writeFile(pyUri, Buffer.from('x: int = 1\n'));
        const doc = await vscode.workspace.openTextDocument(pyUri);
        await vscode.window.showTextDocument(doc);

        // Poll until the extension is active.
        const ext = vscode.extensions.getExtension('Nimblesite.basilisk');
        if (ext && !ext.isActive) {
            await ext.activate();
        }
        const deadline = Date.now() + WAIT_MS;
        while (Date.now() < deadline) {
            if (ext?.isActive) {break;}
            await delay(POLL_INTERVAL_MS);
        }
    });

    // ----------------------------------------------------------------
    // 1. Extension activates on Python file
    // ----------------------------------------------------------------
    test('Extension activates on Python file', async () => {
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);

        // The extension may already be active from suiteSetup, but
        // if not, activate it explicitly.
        if (!ext.isActive) {
            await ext.activate();
        }
        assert.strictEqual(ext.isActive, true, 'Extension should be active after opening a Python file');
    });

    // ----------------------------------------------------------------
    // 2. Extension registers expected commands [VSIX-COMMANDS]
    // ----------------------------------------------------------------
    test('Extension registers basilisk.restartServer command', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(
            store.isClientCommandRegistered('basilisk.restartServer'),
            'basilisk.restartServer should be tracked in internal VSIX state'
        );
    });

    test('Extension registers basilisk.showOutput command', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(
            store.isClientCommandRegistered('basilisk.showOutput'),
            'basilisk.showOutput should be tracked in internal VSIX state'
        );
    });

    test('LSP server advertises basilisk.organizeImports command', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(
            store.isServerCommandAdvertised('basilisk.organizeImports'),
            'basilisk.organizeImports should be advertised by the LSP server'
        );
    });

    // ----------------------------------------------------------------
    // 3. Extension contributes configuration settings
    // [VSIX-CONFIGURATION-SETTINGS], [VSIX-CONFIGURATION-SETTINGS-VS-CODE-ONLY]
    // (useLsp/trace.server). executablePath/bundled resolution → [VSIX-BINARY-
    // RESOLUTION] / [VSIX-BINARY-DISTRIBUTION].
    // ----------------------------------------------------------------
    test('Extension contributes basilisk.executablePath setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<string>('executablePath');
        assert.ok(inspected, 'basilisk.executablePath should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            '',
            'Default executablePath should be empty so Shipwright uses the bundled binary'
        );
    });

    // Tests [VSIX-BINARY-RESOLUTION]: the default resolution cascade picks the
    // bundled per-platform VSIX binary (Shipwright source = "bundled").
    test('Shipwright resolves basilisk from the bundled VSIX binary by default', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        const resolution = store.runtimeResolution.value;
        assert.ok(resolution, 'Shipwright runtime resolution should be recorded');
        assert.strictEqual(resolution.componentId, 'basilisk');
        assert.strictEqual(resolution.source, 'bundled');
        const normalized = resolution.path.split(path.sep).join('/');
        assert.ok(normalized.includes('/bin/'), `Expected bundled bin path, got ${resolution.path}`);
        assert.ok(
            normalized.endsWith('/basilisk') || normalized.endsWith('/basilisk.exe'),
            `Expected basilisk executable path, got ${resolution.path}`
        );
    });

    test('Extension contributes Shipwright binary directory setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<string>('binaries.path');
        assert.ok(inspected, 'basilisk.binaries.path should be a contributed setting');
        assert.strictEqual(inspected.defaultValue, '');
    });

    test('Extension contributes Shipwright per-component basilisk setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<string>('binaries.basilisk');
        assert.ok(inspected, 'basilisk.binaries.basilisk should be a contributed setting');
        assert.strictEqual(inspected.defaultValue, '');
    });

    test('Extension contributes basilisk.enabled setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<boolean>('enabled');
        assert.ok(inspected, 'basilisk.enabled should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            true,
            'Default enabled should be true'
        );
    });

    test('Extension contributes basilisk.useLsp setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<boolean>('useLsp');
        assert.ok(inspected, 'basilisk.useLsp should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            true,
            'Default useLsp should be true'
        );
    });

    test('Extension contributes basilisk.trace.server setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<string>('trace.server');
        assert.ok(inspected, 'basilisk.trace.server should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            'off',
            'Default trace.server should be "off"'
        );
    });

    test('Extension contributes basilisk.inlayHints.parameterNames setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<boolean>('inlayHints.parameterNames');
        assert.ok(inspected, 'basilisk.inlayHints.parameterNames should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            true,
            'Default inlayHints.parameterNames should be true'
        );
    });

    test('Extension contributes basilisk.inlayHints.variableTypes setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<boolean>('inlayHints.variableTypes');
        assert.ok(inspected, 'basilisk.inlayHints.variableTypes should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            true,
            'Default inlayHints.variableTypes should be true'
        );
    });

    // The formatter is the Ruff engine embedded in the Basilisk binary — there is
    // no external `ruff` binary, so there is no `ruff.executablePath`. The only
    // formatter setting is the engine selector. [LSPFMT-CONFIG]
    test('Extension contributes basilisk.formatter setting defaulting to "ruff"', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<string>('formatter');
        assert.ok(inspected, 'basilisk.formatter should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            'ruff',
            'Default formatter should be the embedded Ruff engine ("ruff")'
        );
    });

    test('Extension does NOT contribute any basilisk.ruff.* setting', () => {
        // The external ruff binary is jettisoned; a ruff path/toggle would be a
        // dead, misleading setting. [LSPFMT-DECISION]
        const cfg = vscode.workspace.getConfiguration('basilisk');
        assert.strictEqual(
            cfg.inspect('ruff.enabled')?.defaultValue,
            undefined,
            'basilisk.ruff.enabled must not exist'
        );
        assert.strictEqual(
            cfg.inspect('ruff.executablePath')?.defaultValue,
            undefined,
            'basilisk.ruff.executablePath must not exist'
        );
    });

    // ----------------------------------------------------------------
    // 4. Status bar item is created after activation [VSIX-STATUS-BAR]
    //    (also exercises basilisk.showOutput → [VSIX-OUTPUT-CHANNELS])
    // ----------------------------------------------------------------
    test('Status bar item is created after activation', async () => {
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);

        if (!ext.isActive) {
            await ext.activate();
        }

        // The extension creates a status bar item that is shown on activation.
        // We verify the extension exports are available and the extension is active,
        // which implies the status bar was created (since it's created in activate()).
        // Direct status bar item inspection is not exposed by the VS Code API,
        // but we can verify that the extension activated without error and that
        // the showOutput command (linked to the status bar) works.
        assert.strictEqual(ext.isActive, true, 'Extension must be active for status bar to exist');

        // Execute the showOutput command (which is bound to the status bar item).
        // If the status bar and output channel were not created, this would throw.
        await vscode.commands.executeCommand('basilisk.showOutput');
    });

    // ----------------------------------------------------------------
    // 5. Extension package metadata is correct
    // ----------------------------------------------------------------
    test('Extension has correct display name', () => {
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);
        assert.strictEqual((ext.packageJSON as PackageJSON).displayName, 'Basilisk');
    });

    test('Extension activates on Python language', () => {
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);
        const activationEvents: string[] = (ext.packageJSON as PackageJSON).activationEvents ?? [];
        assert.ok(
            activationEvents.includes('onLanguage:python'),
            'Extension should activate on Python language'
        );
    });

    // ----------------------------------------------------------------
    // Cleanup
    // ----------------------------------------------------------------
    suiteTeardown(async () => {
        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? __dirname;
        const pyUri = vscode.Uri.file(path.join(workspaceRoot, '__basilisk_test__.py'));
        try {
            await vscode.workspace.fs.delete(pyUri);
        } catch {
            // File may not exist — ignore.
        }
    });
});
