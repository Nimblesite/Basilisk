import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';

const EXTENSION_ID = 'basilisk-lang.basilisk';

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

        // Give the extension time to activate.
        await new Promise<void>(resolve => setTimeout(resolve, 2000));
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
    // 2. Extension registers expected commands
    // ----------------------------------------------------------------
    test('Extension registers basilisk.restartServer command', async () => {
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.restartServer'),
            'basilisk.restartServer command should be registered'
        );
    });

    test('Extension registers basilisk.showOutput command', async () => {
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.showOutput'),
            'basilisk.showOutput command should be registered'
        );
    });

    test('Extension registers basilisk.organizeImports command', async () => {
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.organizeImports'),
            'basilisk.organizeImports command should be registered'
        );
    });

    // ----------------------------------------------------------------
    // 3. Extension contributes configuration settings
    // ----------------------------------------------------------------
    test('Extension contributes basilisk.executablePath setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<string>('executablePath');
        assert.ok(inspected, 'basilisk.executablePath should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            'basilisk',
            'Default executablePath should be "basilisk"'
        );
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

    test('Extension contributes basilisk.ruff.enabled setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<boolean>('ruff.enabled');
        assert.ok(inspected, 'basilisk.ruff.enabled should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            true,
            'Default ruff.enabled should be true'
        );
    });

    test('Extension contributes basilisk.ruff.executablePath setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<string>('ruff.executablePath');
        assert.ok(inspected, 'basilisk.ruff.executablePath should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            'ruff',
            'Default ruff.executablePath should be "ruff"'
        );
    });

    // ----------------------------------------------------------------
    // 4. Status bar item is created after activation
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
        assert.strictEqual(ext.packageJSON.displayName, 'Basilisk');
    });

    test('Extension activates on Python language', () => {
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, `Extension ${EXTENSION_ID} should be installed`);
        const activationEvents: string[] = ext.packageJSON.activationEvents ?? [];
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
