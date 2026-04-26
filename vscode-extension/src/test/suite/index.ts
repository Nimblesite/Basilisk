/**
 * NOTE: This file is NOT used when running `npm test` (i.e. `@vscode/test-cli`).
 * The CLI creates its own Mocha instance from `.vscode-test.mjs`'s `mocha` block.
 * All Mocha config (timeout, bail, reporter) lives in `.vscode-test.mjs`.
 *
 * This module only exists as a `mocha.require` global-setup hook: it pre-warms
 * the LSP server ONCE before the test run so we don't pay cold-start on every
 * root suite.
 */
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as vscode from 'vscode';
import {
    EXTENSION_ID,
    POLL_INTERVAL_MS,
    WAIT_MS,
    isLspReady,
    markLspReady,
} from './test-helpers';

/**
 * Pre-warm the LSP server before any test suite runs.
 *
 * Runs exactly ONCE — `isLspReady()` short-circuits subsequent invocations
 * so we don't pay the cold-start cost for each of ~20 root suites.
 */
async function prewarmLsp(): Promise<void> {
    const ext = vscode.extensions.getExtension(EXTENSION_ID);
    if (ext && !ext.isActive) {
        await ext.activate();
    }

    if (isLspReady()) {
        return;
    }

    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-prewarm-'));
    const dummyPath = path.join(tmpDir, '__init__.py');
    fs.writeFileSync(dummyPath, '', 'utf8');
    const dummyUri = vscode.Uri.file(dummyPath);
    const dummyDoc = await vscode.workspace.openTextDocument(dummyUri);
    await vscode.window.showTextDocument(dummyDoc);

    const deadline = Date.now() + WAIT_MS;
    while (Date.now() < deadline) {
        try {
            const syms = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                'vscode.executeDocumentSymbolProvider', dummyUri
            );
            if (syms !== null && syms !== undefined) {
                markLspReady();
                break;
            }
        } catch { /* server not ready yet */ }
        await new Promise<void>((r) => setTimeout(r, POLL_INTERVAL_MS));
    }

    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    fs.rmSync(tmpDir, { recursive: true, force: true });
}

/** @vscode/test-cli calls this via `mocha.require` before any test file loads. */
export async function mochaGlobalSetup(): Promise<void> {
    await prewarmLsp();
}
