// Implements [LSPUV]. See docs/specs/LSP-UV-SPEC.md#LSPUV
import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import { getStore } from '../../extension';
import { createServerCommandHandler } from '../../lsp-client';
import {
    SUITE_SETUP_TIMEOUT_MS,
    setupLspTestSuite,
    teardownLspTestSuite,
} from './test-helpers';

/** Run `body`, capturing any `showInformationMessage` toasts, then restore. */
async function captureInfoToasts(body: () => Promise<unknown>): Promise<string[]> {
    const messages: string[] = [];
    const win = vscode.window as {
        showInformationMessage: typeof vscode.window.showInformationMessage;
    };
    const original = win.showInformationMessage;
    win.showInformationMessage = (async (msg: string) => {
        messages.push(msg);
        return undefined;
    }) as typeof vscode.window.showInformationMessage;
    try {
        await body();
    } finally {
        win.showInformationMessage = original;
    }
    return messages;
}

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

    // ----------------------------------------------------------------
    // Quick-fix success toast names the package (regression)
    // ----------------------------------------------------------------

    // A code action (e.g. the BSK-E0152 "install stubs" fix) invokes
    // basilisk.uv.addDev with a BARE STRING argument, not a `{ package }`
    // object. The success toast must read the package name from either shape;
    // previously it only handled the object form and showed "undefined".
    test('uv.addDev success toast names the package from a bare-string arg', async () => {
        const fakeClient = {
            sendRequest: async () => ({ success: true }),
        } as unknown as Parameters<typeof createServerCommandHandler>[0];
        const handler = createServerCommandHandler(fakeClient, 'basilisk.uv.addDev');

        // A code action sends the package as a bare string, not { package }.
        const messages = await captureInfoToasts(async () => handler('types-six'));

        assert.ok(
            messages.some((m) => m.includes('types-six')),
            `toast should name the package; got: ${JSON.stringify(messages)}`
        );
        assert.ok(
            !messages.some((m) => m.includes('undefined')),
            `toast must not say "undefined"; got: ${JSON.stringify(messages)}`
        );
    });

});


