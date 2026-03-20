import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';

const EXTENSION_ID = 'basilisk-lang.basilisk';

suite('Basilisk uv Integration Tests', () => {

    suiteSetup(async () => {
        // Ensure the extension is activated by opening a Python file.
        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? __dirname;
        const pyFilePath = path.join(workspaceRoot, '__basilisk_uv_test__.py');
        const pyUri = vscode.Uri.file(pyFilePath);

        await vscode.workspace.fs.writeFile(pyUri, Buffer.from('x: int = 1\n'));
        const doc = await vscode.workspace.openTextDocument(pyUri);
        await vscode.window.showTextDocument(doc);

        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        if (ext && !ext.isActive) {
            await ext.activate();
        }
        const deadline = Date.now() + 5_000;
        while (Date.now() < deadline) {
            if (ext?.isActive) {break;}
            await new Promise<void>(r => setTimeout(r, 100));
        }
    });

    // ----------------------------------------------------------------
    // uv commands are registered
    // ----------------------------------------------------------------

    test('Extension registers basilisk.uv.sync command', async () => {
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.uv.sync'),
            'basilisk.uv.sync command should be registered'
        );
    });

    test('Extension registers basilisk.uv.add command', async () => {
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.uv.add'),
            'basilisk.uv.add command should be registered'
        );
    });

    test('Extension registers basilisk.uv.addDev command', async () => {
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.uv.addDev'),
            'basilisk.uv.addDev command should be registered'
        );
    });

    test('Extension registers basilisk.uv.remove command', async () => {
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.uv.remove'),
            'basilisk.uv.remove command should be registered'
        );
    });

    test('Extension registers basilisk.uv.lock command', async () => {
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.uv.lock'),
            'basilisk.uv.lock command should be registered'
        );
    });

    test('Extension registers basilisk.uv.createEnv command', async () => {
        const commands = await vscode.commands.getCommands(true);
        assert.ok(
            commands.includes('basilisk.uv.createEnv'),
            'basilisk.uv.createEnv command should be registered'
        );
    });

    // ----------------------------------------------------------------
    // uv settings exist in the configuration
    // ----------------------------------------------------------------

    test('Extension contributes basilisk.uv.enabled setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<boolean>('uv.enabled');
        assert.ok(inspected, 'basilisk.uv.enabled should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            true,
            'Default uv.enabled should be true'
        );
    });

    test('Extension contributes basilisk.uv.executablePath setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<string>('uv.executablePath');
        assert.ok(inspected, 'basilisk.uv.executablePath should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            '',
            'Default uv.executablePath should be empty string'
        );
    });

    test('Extension contributes basilisk.uv.autoSync setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<boolean>('uv.autoSync');
        assert.ok(inspected, 'basilisk.uv.autoSync should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            false,
            'Default uv.autoSync should be false'
        );
    });

    test('Extension contributes basilisk.uv.stubSuggestions setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<boolean>('uv.stubSuggestions');
        assert.ok(inspected, 'basilisk.uv.stubSuggestions should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            true,
            'Default uv.stubSuggestions should be true'
        );
    });

    test('Extension contributes basilisk.uv.dependencyDiagnostics setting', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const inspected = cfg.inspect<boolean>('uv.dependencyDiagnostics');
        assert.ok(inspected, 'basilisk.uv.dependencyDiagnostics should be a contributed setting');
        assert.strictEqual(
            inspected.defaultValue,
            true,
            'Default uv.dependencyDiagnostics should be true'
        );
    });

    // ----------------------------------------------------------------
    // Cleanup
    // ----------------------------------------------------------------
    suiteTeardown(async () => {
        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? __dirname;
        const pyUri = vscode.Uri.file(path.join(workspaceRoot, '__basilisk_uv_test__.py'));
        try {
            await vscode.workspace.fs.delete(pyUri);
        } catch {
            // File may not exist — ignore.
        }
    });
});
