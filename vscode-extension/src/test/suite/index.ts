import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import Mocha from 'mocha';
import { glob } from 'glob';
import * as vscode from 'vscode';
import {
    EXTENSION_ID,
    SERVER_START_WAIT_MS,
    SUITE_SETUP_TIMEOUT_MS,
} from './test-helpers';

/**
 * Pre-warm the LSP server before any test suite runs.
 *
 * Individual suiteSetup hooks have short (30 s) timeouts that are fine once the
 * server is already responsive, but too short for a cold CI start (cargo build +
 * LSP init can exceed 60 s).  By waiting here — with the generous 90 s timeout —
 * we guarantee the server is ready before any suite begins.
 */
async function prewarmLsp(): Promise<void> {
    const ext = vscode.extensions.getExtension(EXTENSION_ID);
    if (ext && !ext.isActive) {
        await ext.activate();
    }

    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-prewarm-'));
    const dummyPath = path.join(tmpDir, '__init__.py');
    fs.writeFileSync(dummyPath, '', 'utf8');
    const dummyUri = vscode.Uri.file(dummyPath);
    const dummyDoc = await vscode.workspace.openTextDocument(dummyUri);
    await vscode.window.showTextDocument(dummyDoc);

    const pollIntervalMs = 200;
    const deadline = Date.now() + SERVER_START_WAIT_MS;
    let serverReady = false;
    while (Date.now() < deadline) {
        try {
            const syms = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                'vscode.executeDocumentSymbolProvider', dummyUri
            );
            if (syms !== null && syms !== undefined) {
                serverReady = true;
                break;
            }
        } catch { /* server not ready yet */ }
        await new Promise<void>((r) => setTimeout(r, pollIntervalMs));
    }

    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    fs.rmSync(tmpDir, { recursive: true, force: true });

    if (!serverReady) {
        throw new Error(
            `prewarmLsp: LSP server failed to respond within ${SERVER_START_WAIT_MS}ms. ` +
            'All tests will fail. Build the binary: cargo build -p basilisk-cli'
        );
    }
}

export async function run(): Promise<void> {
    const timeout = parseInt(process.env.MOCHA_TIMEOUT ?? '60000', 10);
    const mocha = new Mocha({
        ui: 'tdd',
        color: true,
        timeout,
        rootHooks: {
            beforeAll(this: Mocha.Context, done: Mocha.Done) {
                this.timeout(SUITE_SETUP_TIMEOUT_MS);
                prewarmLsp().then(() => done(), done);
            },
        },
    });
    const testsRoot = path.resolve(__dirname);
    const files = await glob('**/**.test.js', { cwd: testsRoot });
    files.forEach(f => mocha.addFile(path.resolve(testsRoot, f)));
    return new Promise<void>((resolve, reject) => {
        mocha.run(failures => {
            if (failures > 0) {
                reject(new Error(`${failures} tests failed.`));
            } else {
                resolve();
            }
        });
    });
}
