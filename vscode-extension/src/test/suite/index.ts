import * as path from 'path';
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
 *
 * Readiness is determined by store.serverCommands being non-empty, which means
 * Basilisk's LSP has completed the initialize handshake and sent its capabilities.
 */
async function prewarmLsp(): Promise<void> {
    const ext = vscode.extensions.getExtension(EXTENSION_ID);
    if (ext && !ext.isActive) {
        await ext.activate();
    }

    const { getStore } = await import('./../../extension');

    const pollIntervalMs = 200;
    const deadline = Date.now() + SERVER_START_WAIT_MS;
    let serverReady = false;
    while (Date.now() < deadline) {
        const store = getStore();
        if (store !== undefined && store.serverCommands.value.size > 0) {
            serverReady = true;
            break;
        }
        await new Promise<void>((r) => setTimeout(r, pollIntervalMs));
    }

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
