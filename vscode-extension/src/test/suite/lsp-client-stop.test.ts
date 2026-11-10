// Tests for [LSPARCH-CMDREG] client lifecycle — see docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDREG
// Covers src/lsp-client-stop.ts.
/**
 * A `LanguageClient` caught mid-start must still be stoppable.
 *
 * `vscode-languageclient`'s `shutdown` rejects unless the state is exactly
 * `Running`, so `stop()` on a starting client throws rather than stopping it,
 * and `isRunning()` — the guard the shutdown paths used to use — reads `false`
 * for a client that has already spawned its server process. The two together
 * mean a client shut down during its own start either throws out of
 * `deactivate()` or is dropped while its server keeps running (GitHub #264).
 *
 * The window is wide on win32, where spawning the server binary is slow enough
 * that a deactivate/activate cycle routinely lands inside it.
 *
 * The double below reproduces that contract exactly — `stop()`/`dispose()`
 * reject with the real message unless the state is `running` — so a helper
 * that skips the settle step fails these tests instead of passing them.
 */

import * as assert from 'assert';
import type { LanguageClient } from 'vscode-languageclient/node';
import { stopClientSettled } from '../../lsp-client-stop';
import { fakeLanguageClient } from './test-helpers';

type FakeState = 'initial' | 'starting' | 'running' | 'stopped';

interface Harness {
    readonly client: LanguageClient;
    state(): FakeState;
    stops(): number;
    disposes(): number;
    startCalls(): number;
    finishStart(): void;
    failStart(error: Error): void;
}

/**
 * A client double honouring the real lifecycle contract:
 * `needsStop()` covers starting AND running, `isRunning()` covers running
 * alone, and shutting down from any state but `running` rejects.
 */
function makeClient(initial: FakeState): Harness {
    let state: FakeState = initial;
    let stops = 0;
    let disposes = 0;
    let startCalls = 0;
    let settleStart: (() => void) | undefined;
    let breakStart: ((error: Error) => void) | undefined;

    const pendingStart = new Promise<void>((resolve, reject) => {
        settleStart = (): void => { state = 'running'; resolve(); };
        breakStart = (error: Error): void => { state = 'stopped'; reject(error); };
    });
    // A start that is never awaited must not surface as an unhandled rejection.
    pendingStart.catch(() => { /* observed by the helper, or not at all */ });

    async function shutdown(tally: () => void): Promise<void> {
        if (state !== 'running') {
            throw new Error(
                `Client is not running and can't be stopped. It's current state is: ${state}`,
            );
        }
        tally();
        state = 'stopped';
    }

    const client = fakeLanguageClient({
        needsStop: (): boolean => state === 'starting' || state === 'running',
        isRunning: (): boolean => state === 'running',
        start: async (): Promise<void> => {
            startCalls += 1;
            // The real `start()` hands back the in-flight start promise rather
            // than beginning a second one.
            if (state === 'starting') { await pendingStart; }
        },
        stop: async (): Promise<void> => shutdown(() => { stops += 1; }),
        dispose: async (): Promise<void> => shutdown(() => { disposes += 1; }),
    });

    return {
        client,
        state: (): FakeState => state,
        stops: (): number => stops,
        disposes: (): number => disposes,
        startCalls: (): number => startCalls,
        finishStart: (): void => { settleStart?.(); },
        failStart: (error: Error): void => { breakStart?.(error); },
    };
}

suite('LSP client shutdown settles a start in flight [LSPARCH-CMDREG]', () => {

    test('a RUNNING client stops directly', async () => {
        const harness = makeClient('running');

        await stopClientSettled(harness.client);

        assert.strictEqual(harness.stops(), 1, 'a running client must be stopped');
        assert.strictEqual(harness.state(), 'stopped');
    });

    test('a STARTING client is stopped once its start settles — never dropped', async () => {
        const harness = makeClient('starting');

        const stopped = stopClientSettled(harness.client);
        // The helper must be waiting on the start, not calling stop() into a
        // rejection and not abandoning the client.
        assert.strictEqual(harness.stops(), 0, 'stop() must not be called while starting');

        harness.finishStart();
        await stopped;

        assert.strictEqual(harness.stops(), 1, 'the settled client must then be stopped');
        assert.strictEqual(harness.state(), 'stopped', 'the server process must not be left running');
    });

    test('a STARTING client is never shut down from the starting state', async () => {
        const harness = makeClient('starting');

        const stopped = stopClientSettled(harness.client);
        harness.finishStart();

        // A helper that called stop() while starting would reject here with
        // "Client is not running and can't be stopped" — the CI failure.
        await stopped;

        assert.strictEqual(harness.startCalls(), 1, 'the in-flight start must be awaited exactly once');
    });

    test('a failed start is swallowed — that client is already stopped', async () => {
        const harness = makeClient('starting');

        const stopped = stopClientSettled(harness.client);
        harness.failStart(new Error('server binary missing'));

        // deactivate() must not reject because the client it is tearing down
        // failed to start in the first place.
        await stopped;

        assert.strictEqual(harness.stops(), 0, 'a client that never started has nothing to stop');
        assert.strictEqual(harness.state(), 'stopped');
    });

    test('a STOPPED client is left alone', async () => {
        const harness = makeClient('stopped');

        await stopClientSettled(harness.client);

        assert.strictEqual(harness.stops(), 0, 'needsStop() is false — nothing to do');
        assert.strictEqual(harness.startCalls(), 0, 'a stopped client must never be started to stop it');
    });

    test('an INITIAL client is left alone', async () => {
        const harness = makeClient('initial');

        await stopClientSettled(harness.client);

        assert.strictEqual(harness.stops(), 0);
        assert.strictEqual(harness.startCalls(), 0);
    });

    test('concurrent shutdowns of one client collapse into a single stop', async () => {
        const harness = makeClient('starting');

        // deactivate() stops the client and then calls store.reset(), which
        // also wants it gone. The second must join the first, not race it into
        // a rejection.
        const first = stopClientSettled(harness.client);
        const second = stopClientSettled(harness.client);

        harness.finishStart();
        await Promise.all([first, second]);

        assert.strictEqual(harness.stops(), 1, 'the client must be stopped exactly once');
        assert.strictEqual(harness.disposes(), 0, 'the second caller must not shut it down again');
    });

    test("dispose mode tears the client down so it cannot be restarted", async () => {
        const harness = makeClient('starting');

        const stopped = stopClientSettled(harness.client, 'dispose');
        harness.finishStart();
        await stopped;

        assert.strictEqual(harness.disposes(), 1, 'dispose mode must dispose');
        assert.strictEqual(harness.stops(), 0, 'dispose mode must not also plain-stop');
    });

    test('a client stopped once can be stopped again later without throwing', async () => {
        const harness = makeClient('running');

        await stopClientSettled(harness.client);
        // The in-flight entry must be released on completion, and the second
        // call must see a stopped client and no-op rather than reject.
        await stopClientSettled(harness.client);

        assert.strictEqual(harness.stops(), 1);
    });
});
