// Implements [LSPUV]. See docs/specs/LSP-UV-SPEC.md#LSPUV
import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import { getStore } from '../../extension';
import {
    SUITE_SETUP_TIMEOUT_MS,
    setupLspTestSuite,
    teardownLspTestSuite,
} from './test-helpers';

suite('Basilisk uv Integration Tests', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-uv-test-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(async () => {
        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? __dirname;
        const pyUri = vscode.Uri.file(path.join(workspaceRoot, '__basilisk_uv_test__.py'));
        try {
            await vscode.workspace.fs.delete(pyUri);
        } catch {
            // File may not exist — ignore.
        }
        teardownLspTestSuite(tmpDir);
    });

    // ----------------------------------------------------------------
    // uv commands are advertised by the LSP server
    // ----------------------------------------------------------------

    test('LSP server advertises basilisk.uv.sync command', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(store.isServerCommandAdvertised('basilisk.uv.sync'), 'basilisk.uv.sync should be advertised by the LSP server');
    });

    test('LSP server advertises basilisk.uv.add command', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(store.isServerCommandAdvertised('basilisk.uv.add'), 'basilisk.uv.add should be advertised by the LSP server');
    });

    test('LSP server advertises basilisk.uv.addDev command', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(store.isServerCommandAdvertised('basilisk.uv.addDev'), 'basilisk.uv.addDev should be advertised by the LSP server');
    });

    test('LSP server advertises basilisk.uv.remove command', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(store.isServerCommandAdvertised('basilisk.uv.remove'), 'basilisk.uv.remove should be advertised by the LSP server');
    });

    test('LSP server advertises basilisk.uv.lock command', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(store.isServerCommandAdvertised('basilisk.uv.lock'), 'basilisk.uv.lock should be advertised by the LSP server');
    });

    test('LSP server advertises basilisk.uv.createEnv command', () => {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(store.isServerCommandAdvertised('basilisk.uv.createEnv'), 'basilisk.uv.createEnv should be advertised by the LSP server');
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

});


