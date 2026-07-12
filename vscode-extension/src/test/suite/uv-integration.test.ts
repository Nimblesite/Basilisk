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
    });
    try {
        await body();
    } finally {
        win.showInformationMessage = original;
    }
    return messages;
}

/**
 * Drive a server command through `createServerCommandHandler` with a fake LSP
 * client that returns `result`, capturing the info toasts it produces. Lets a
 * test assert on the toast behaviour for a given LSP outcome without a live
 * server.
 */
async function uvToastsForResult(
    result: unknown,
    command: string,
    arg: unknown
): Promise<string[]> {
    const fakeClient = {
        sendRequest: async () => result,
    } as unknown as Parameters<typeof createServerCommandHandler>[0];
    const handler = createServerCommandHandler(fakeClient, command);
    return captureInfoToasts(async () => handler(arg));
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

    test('Rule-family policy is not exposed as VS Code settings', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        // A key that is NOT contributed as a setting has no declared default.
        // (Recent VS Code returns a shaped inspect() object with every value
        // `undefined` for an unknown key under a known section rather than a
        // literal `undefined`, so assert on the absence of a `defaultValue` —
        // a contributed boolean setting would report its declared default.)
        assert.strictEqual(
            cfg.inspect<boolean>('uv.stubSuggestions')?.defaultValue,
            undefined,
            'stub suggestions must be configured through BSK-E0152 severity, not a VS Code setting'
        );
        assert.strictEqual(
            cfg.inspect<boolean>('uv.dependencyDiagnostics')?.defaultValue,
            undefined,
            'dependency diagnostics must be configured through explicit rule severities, not a VS Code setting'
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
        // A code action sends the package as a bare string, not { package }.
        const messages = await uvToastsForResult({ success: true }, 'basilisk.uv.addDev', 'types-six');

        assert.ok(
            messages.some((m) => m.includes('types-six')),
            `toast should name the package; got: ${JSON.stringify(messages)}`
        );
        assert.ok(
            !messages.some((m) => m.includes('undefined')),
            `toast must not say "undefined"; got: ${JSON.stringify(messages)}`
        );
    });

    // Regression for issue #84: the optimistic "Added X" success toast fired
    // unconditionally, never inspecting the LSP result. When `uv add` failed
    // (e.g. on a non-package internal module like `_pydevd_bundle`), the UI
    // showed BOTH the green "Added X" toast and the LSP's own error toast for
    // the same operation. The success toast must be gated on result.success.
    test('uv.add success toast is suppressed when the command fails', async () => {
        const failure = {
            success: false,
            stdout: '',
            stderr: 'error: Failed to parse: `_pydevd_bundle`',
        };
        const messages = await uvToastsForResult(failure, 'basilisk.uv.add', '_pydevd_bundle');

        assert.ok(
            !messages.some((m) => m.includes('Added')),
            `must not show a success toast when uv add failed; got: ${JSON.stringify(messages)}`
        );
    });

});

