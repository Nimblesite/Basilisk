// Tests for [VSIX-PYTHON-DEBUGGER-DAP-ARCHITECTURE]. See docs/specs/VSIX-SPEC.md
// Covers the readiness gate in src/debug-adapter.ts.
/**
 * Starting a debug session before the language server is running.
 *
 * The adapter factory asks the LSP to spawn debugpy. It used to take whatever
 * `store.client` held and send into it, with only a truthiness check — but a
 * client that exists is not a client that is *running*. A request sent while
 * the client is still `Starting` is not answered, and nothing ever rejects it:
 * the debug session hangs with no error, no diagnostic and no way out but
 * cancelling.
 *
 * The window is the whole of server startup, so on win32 — where spawning the
 * server binary takes ~10s — pressing F5 on a freshly opened project lands in
 * it routinely. That is what the Windows CI job reported: the first debug test
 * fires at T, the server reaches Running at T+0.1s, and the request sent at T
 * is never answered while every later one is served in under 300ms.
 *
 * The factory must therefore WAIT for readiness before sending, and say so
 * plainly if readiness never comes.
 */

import * as assert from 'assert';
import type * as vscode from 'vscode';
import type { LanguageClient } from 'vscode-languageclient/node';
import { createDebugAdapterFactory } from '../../debug-adapter';
import type { Result } from '../../result';
import { delay } from '../../timeouts';
import { fakeLanguageClient } from './test-helpers';

/** A debug session double carrying only the configuration the factory reads. */
function sessionWith(config: vscode.DebugConfiguration): vscode.DebugSession {
    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- DebugSession is host-owned; only `configuration` is read here
    return { configuration: config } as unknown as vscode.DebugSession;
}

const LAUNCH_CONFIG: vscode.DebugConfiguration = {
    name: 'readiness probe',
    type: 'basilisk-debug',
    request: 'launch',
    program: 'probe.py',
};

interface ClientDouble {
    readonly client: LanguageClient;
    requests(): number;
}

/** A client that records requests and answers `startDebugSession` with junk. */
function recordingClient(): ClientDouble {
    let requests = 0;
    const client = fakeLanguageClient({
        isRunning: (): boolean => true,
        // The factory only gets this far once readiness has been granted.
        // Rejecting keeps the test off the real debugpy/proxy path — what is
        // under test is WHEN the send happens, not what comes back.
        sendRequest: async (): Promise<never> => {
            requests += 1;
            throw new Error('probe: request reached the server');
        },
    });
    return { client, requests: (): number => requests };
}

suite('Debug adapter waits for LSP readiness [VSIX-PYTHON-DEBUGGER-DAP-ARCHITECTURE]', () => {

    test('no request is sent while the client is still starting', async () => {
        const double = recordingClient();
        let ready = false;
        const factory = createDebugAdapterFactory(async (): Promise<Result<LanguageClient>> => {
            // Mirrors awaitLspReady: pends until the client reaches Running.
            while (!ready) { await delay(10); }
            return { ok: true, value: double.client };
        });

        const descriptor = Promise.resolve(
            factory.createDebugAdapterDescriptor(sessionWith(LAUNCH_CONFIG), undefined),
        );
        // A pending descriptor must not surface as an unhandled rejection
        // before the assertions below attach their own handler.
        descriptor.catch(() => { /* asserted on below */ });
        await delay(150);
        assert.strictEqual(
            double.requests(),
            0,
            'the factory must not send startDebugSession before the server is running — ' +
            'that request is never answered and the session hangs silently',
        );

        ready = true;
        // Once ready, the send happens (and our double rejects it, proving it ran).
        await assert.rejects(
            descriptor,
            /probe: request reached the server|Basilisk/,
            'once the server is running the factory must send the request',
        );
        assert.strictEqual(double.requests(), 1, 'exactly one request, sent after readiness');
    });

    test('a server that never becomes ready fails with a diagnosable error, not a hang', async () => {
        const factory = createDebugAdapterFactory(
            async (): Promise<Result<LanguageClient>> => ({
                ok: false,
                error: new Error('LSP client did not reach Running state within 1ms'),
            }),
        );

        await assert.rejects(
            Promise.resolve(factory.createDebugAdapterDescriptor(sessionWith(LAUNCH_CONFIG), undefined)),
            (err: Error) => {
                assert.match(
                    err.message,
                    /Running state|not ready|did not/i,
                    `the failure must name the unready server, got: ${err.message}`,
                );
                return true;
            },
            'an unready server must reject the debug session rather than hang forever',
        );
    });
});
