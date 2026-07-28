// Tests for [LSPARCH-CMDREG]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDREG
/**
 * Command Registration Tests for the Basilisk VS Code Extension.
 *
 * Proves compliance with the VS Code API contract for commands:
 *
 *   - registerCommand() returns a Disposable whose dispose() unregisters it
 *   - Registering a command with an existing identifier twice throws
 *   - After dispose(), the same identifier can be re-registered
 *   - Client commands survive a full dispose/re-register cycle (LSP restart)
 *   - Server commands are never pre-registered (ExecuteCommandFeature removed)
 *   - No double-dispose: disposables live in ONE collection only
 *   - Server + client commands survive full deactivate/activate cycles
 *
 * Reference: https://code.visualstudio.com/api/references/vscode-api#commands
 */

import { delay } from '../../timeouts';
import * as assert from 'assert';
import * as vscode from 'vscode';
import { getStore, activate, deactivate } from '../../extension';
import {
    LSP_RESTART_WAIT_MS,
    EXTENSION_ID,
    POLL_INTERVAL_MS,
    closeAllEditors,
    setupLspTestSuite,
    teardownLspTestSuite,
} from "./test-helpers";
import {
  manifestCommands
} from "./extension-manifest";

/** Number of consecutive deactivate/activate cycles to test. */
const MULTI_CYCLE_COUNT = 3;

/** Brief settle time after restart. */
const RESTART_SETTLE_MS = 500;

/**
 * Mocha budget for ONE test that fully restarts the language server.
 *
 * The suite default (45s, .vscode-test.mjs) is sized for tests that do not
 * respawn the server binary. These do, and a cold win32 runner spends real
 * time on it — so a test driving three restarts needs three times the budget,
 * not the same one. Exceeding the default reports as a bare Mocha timeout that
 * names nothing; `pollUntilReady` inside it reports the state it observed.
 */
const RESTART_TEST_TIMEOUT_MS = 60_000;

/** Budget for a replaced LSP client to stop after store.reset() (#264). */
const ZOMBIE_STOP_TIMEOUT_MS = 5_000;

/**
 * All commands declared in package.json contributes.commands — read from the
 * REAL manifest, never a hand-copied list. A hand-maintained copy silently
 * drifts: it never included `basilisk.profileDiff`, which shipped contributed
 * but unregistered ("command not found" in the palette) and no test noticed.
 */
function manifestCommandIds(): readonly string[] {
    return manifestCommands().map((entry) => entry.command);
}

/** Commands registered client-side (not by the LSP server). */
const CLIENT_COMMANDS = [
    'basilisk.restartServer',
    'basilisk.showOutput',
] as const;

/**
 * Commands advertised by the LSP server via executeCommandProvider.
 *
 * This list MUST match `basilisk_common::commands::ALL` in
 * `crates/basilisk-common/src/lib.rs`. If the server adds a new command,
 * add it here too — the cross-session tests below will catch drift.
 */
const SERVER_COMMANDS = [
    'basilisk.organizeImports',
    'basilisk.startDebugSession',
    'basilisk.stopDebugSession',
    'basilisk.disableRule',
    'basilisk.fixFile',
    'basilisk.fixFileAll',
    'basilisk.fixWorkspace',
    'basilisk.fixWorkspaceAll',
    'basilisk.adoptFile',
    'basilisk.adoptWorkspace',
    'basilisk.unadoptFile',
    'basilisk.uv.sync',
    'basilisk.uv.add',
    'basilisk.uv.addDev',
    'basilisk.uv.remove',
    'basilisk.uv.lock',
    'basilisk.uv.createEnv',
    'basilisk.moveSymbol',
    'basilisk.stubs.createLocal',
    'basilisk.stubs.addMember',
    'basilisk.discoverTests',
    'basilisk.runTests',
    'basilisk.runTestFile',
    'basilisk.debugTest',
    'basilisk.runTestsCoverage',
    'basilisk.workspaceModules',
    'basilisk.typeHealth',
    'basilisk.profiler.start',
    'basilisk.profiler.stop',
    'basilisk.profiler.snapshot',
    'basilisk.profiler.list',
    'basilisk.profiler.processes',
    'basilisk.profiler.cooperativeScript',
    'basilisk.profiler.cooperativeAttach',
    'basilisk.memory.start',
    'basilisk.memory.snapshot',
    'basilisk.memory.diff',
    'basilisk.memory.references',
    'basilisk.memory.objectsByType',
    'basilisk.memory.gcCollect',
    'basilisk.memory.ingest',
] as const;

/** Assert that registering a command succeeds (it was NOT already registered). */
function assertCanRegister(cmd: string, context: string): void {
    let threw = false;
    let disposable: vscode.Disposable | undefined;
    try {
        disposable = vscode.commands.registerCommand(cmd, () => { /* probe */ });
    } catch {
        threw = true;
    } finally {
        disposable?.dispose();
    }
    assert.ok(!threw, `${context}: "${cmd}" should be registerable (not already registered)`);
}

/** Assert that registering a command throws (it IS already registered). */
function assertCannotRegister(cmd: string, context: string): void {
    let threw = false;
    try {
        vscode.commands.registerCommand(cmd, () => { /* noop */ });
    } catch {
        threw = true;
    }
    assert.ok(threw, `${context}: re-registering "${cmd}" should throw (already registered)`);
}

/** Assert that executing a command does not throw. */
async function assertExecutable(cmd: string, context: string): Promise<void> {
    let threw = false;
    try {
        await vscode.commands.executeCommand(cmd);
    } catch {
        threw = true;
    }
    assert.ok(!threw, `${context}: "${cmd}" should be executable`);
}

/**
 * Wait until the store has re-registered its client commands AND the server
 * has re-advertised its own.
 *
 * Throws on timeout rather than returning quietly. A silent give-up turned
 * "the server did not come back within the budget" into whichever assertion
 * happened to run next — `old client should be running before reset`,
 * `Baseline should have server commands` — none of which named the real
 * cause. The message below reports the state it actually observed.
 */
async function pollUntilReady(
    timeoutMs: number,
): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        const store = getStore();
        if (store) {
            const clientReady = CLIENT_COMMANDS.every((cmd) => store.isClientCommandRegistered(cmd));
            const serverReady = store.serverCommands.value.size > 0;
            if (clientReady && serverReady) { return; }
        }
        await delay(POLL_INTERVAL_MS);
    }
    const store = getStore();
    const missing = store === undefined
        ? 'no store'
        : CLIENT_COMMANDS.filter((cmd) => !store.isClientCommandRegistered(cmd)).join(', ') || 'none';
    throw new Error(
        `LSP client did not become ready within ${timeoutMs}ms — ` +
        `lspState=${store?.lspState.value ?? 'n/a'}, ` +
        `client=${store?.client.value !== undefined}, ` +
        `serverCommands=${store?.serverCommands.value.size ?? 0}, ` +
        `unregistered client commands=[${missing}]`
    );
}

// eslint-disable-next-line max-lines-per-function
suite('Command Registration (VS Code API Compliance)', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-cmd-reg-test-');
        tmpDir = result.tmpDir;
        await pollUntilReady(LSP_RESTART_WAIT_MS);

        const store = getStore();
        assert.ok(store, 'Store should exist after suiteSetup');
        assert.ok(
            store.serverCommands.value.size > 0,
            `suiteSetup: server commands empty (lspState=${store.lspState.value}, ` +
            `client=${store.client.value !== undefined}, ` +
            `cmds=${store.serverCommands.value.size})`
        );
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    // ================================================================
    // NON-DESTRUCTIVE TESTS (run first — no deactivate/activate calls)
    // ================================================================

    // ----------------------------------------------------------------
    // 1. Every manifest command is known to VS Code's command registry
    // ----------------------------------------------------------------
    test('all manifest commands exist in the VS Code command registry', function () {
        const commands = manifestCommandIds();
        assert.ok(commands.length > 0, 'the manifest must contribute commands');
        for (const cmd of commands) {
            assertCannotRegister(cmd, 'Manifest command registration');
        }
    });

    // ----------------------------------------------------------------
    // 2. Client commands are tracked in the store
    // ----------------------------------------------------------------
    test('client commands are tracked in the store after activation', function () {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');

        for (const cmd of CLIENT_COMMANDS) {
            assert.ok(
                store.isClientCommandRegistered(cmd),
                `Client command "${cmd}" should be tracked in store.clientCommands`
            );
        }
    });

    // ----------------------------------------------------------------
    // 3. Server commands are NOT tracked as client commands
    // ----------------------------------------------------------------
    test('server commands are NOT registered as client commands', function () {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');

        for (const cmd of SERVER_COMMANDS) {
            assert.ok(
                !store.isClientCommandRegistered(cmd),
                `Server command "${cmd}" must NOT appear in store.clientCommands — ` +
                `it should only be in store.serverCommands`
            );
        }
    });

    // ----------------------------------------------------------------
    // 4. Server commands are advertised via LSP capabilities
    //
    //    Checks that every command from SERVER_COMMANDS that the server
    //    binary supports is present in the store. Commands missing from
    //    the binary (stale build) are logged but not failed — the drift
    //    guard test below catches those.
    // ----------------------------------------------------------------
    test('server commands are advertised in store.serverCommands', function () {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');
        assert.ok(store.serverCommands.value.size > 0, 'Server should advertise at least one command');

        // Verify that every command the server advertises is in our test list.
        for (const cmd of store.serverCommands.value) {
            assert.ok(
                (SERVER_COMMANDS as readonly string[]).includes(cmd),
                `Server advertises "${cmd}" but it is not in SERVER_COMMANDS`
            );
        }
    });

    // ----------------------------------------------------------------
    // 5. Client commands are NOT in server commands
    // ----------------------------------------------------------------
    test('client-only commands are NOT in server commands', function () {
        const store = getStore();
        assert.ok(store, 'Store should be available after activation');

        for (const cmd of CLIENT_COMMANDS) {
            assert.ok(
                !store.isServerCommandAdvertised(cmd),
                `Client-only command "${cmd}" must NOT appear in store.serverCommands`
            );
        }
    });

    // ----------------------------------------------------------------
    // 6. No duplicate registration — API says this throws
    // ----------------------------------------------------------------
    test('registering an already-registered client command throws', async function () {
        for (const cmd of CLIENT_COMMANDS) {
            assertCannotRegister(cmd, 'Duplicate registration');
        }
    });

    // ----------------------------------------------------------------
    // 7. Server commands ARE registered (routed through LSP client)
    // ----------------------------------------------------------------
    test('server commands are registered via syncServerCommands', async function () {
        for (const cmd of SERVER_COMMANDS) {
            assertCannotRegister(cmd, 'Server command registration');
        }
    });

    // ----------------------------------------------------------------
    // 8. SERVER_COMMANDS list matches what the LSP actually advertises
    //
    //    Guards against drift: every command the server advertises must
    //    be in our SERVER_COMMANDS list. Also verifies that the server
    //    advertises a reasonable number of commands (detects broken binary).
    // ----------------------------------------------------------------
    test('SERVER_COMMANDS list matches server capabilities exactly', function () {
        const store = getStore();
        assert.ok(store, 'Store should be available');

        const serverSet = store.serverCommands.value;
        assert.ok(serverSet.size > 0, 'Server should advertise at least one command');

        // Every command the server advertises must be in our test list.
        const testSet = new Set<string>(SERVER_COMMANDS);
        for (const cmd of serverSet) {
            assert.ok(
                testSet.has(cmd),
                `Server advertises "${cmd}" but it is not in SERVER_COMMANDS — add it`
            );
        }

        // Every command in our test list should be advertised by the server.
        // If not, the binary is stale — log but still fail (rebuild required).
        for (const cmd of SERVER_COMMANDS) {
            assert.ok(
                serverSet.has(cmd),
                `SERVER_COMMANDS contains "${cmd}" but server does not advertise it. ` +
                `Rebuild the binary: cargo build -p basilisk-cli`
            );
        }
    });

    // ================================================================
    // DESTRUCTIVE TESTS (call deactivate/activate — run last)
    // ================================================================

    // ----------------------------------------------------------------
    // 9. Client commands survive a restart cycle (dispose + re-register)
    // ----------------------------------------------------------------
    test('client commands survive a full LSP restart cycle', async function () {
        this.timeout(RESTART_TEST_TIMEOUT_MS);

        const store = getStore();
        assert.ok(store, 'Store should be available');

        for (const cmd of CLIENT_COMMANDS) {
            assert.ok(
                store.isClientCommandRegistered(cmd),
                `"${cmd}" should be registered before restart`
            );
        }

        await vscode.commands.executeCommand('basilisk.restartServer');
        await delay(RESTART_SETTLE_MS);
        await pollUntilReady(LSP_RESTART_WAIT_MS);

        for (const cmd of CLIENT_COMMANDS) {
            assert.ok(store.isClientCommandRegistered(cmd), `"${cmd}" should be re-registered after restart`);
        }
        for (const cmd of CLIENT_COMMANDS) {
            await assertExecutable(cmd, 'After restart');
        }
    });

    // ----------------------------------------------------------------
    // 10. store.reset() disposes all client commands
    // ----------------------------------------------------------------
    test('store.reset() clears all client command tracking', async function () {
        this.timeout(RESTART_TEST_TIMEOUT_MS);

        const store = getStore();
        assert.ok(store, 'Store should be available');

        for (const cmd of CLIENT_COMMANDS) {
            assert.ok(
                store.isClientCommandRegistered(cmd),
                `"${cmd}" should be registered before reset`
            );
        }

        store.reset();

        for (const cmd of CLIENT_COMMANDS) {
            assert.ok(
                !store.isClientCommandRegistered(cmd),
                `"${cmd}" should NOT be tracked after store.reset()`
            );
        }

        for (const cmd of CLIENT_COMMANDS) {
            assertCanRegister(cmd, 'After store.reset()');
        }

        // Re-activate so subsequent tests aren't broken.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        if (ext && !ext.isActive) {
            await ext.activate();
        }
        await pollUntilReady(LSP_RESTART_WAIT_MS);
    });

    // ----------------------------------------------------------------
    // 10b. store.reset() must stop the replaced LSP client (GitHub #264)
    //
    //     reset() drops the client reference and onReset starts a NEW
    //     LanguageClient. If the old client is never stopped it stays a
    //     live zombie: it keeps forwarding didOpen/didClose to its own
    //     server and publishing into its own diagnostics collection,
    //     which VS Code merges into getDiagnostics() — late zombie
    //     republishes then resurrect diagnostics the real server
    //     cleared (the flaky openFilesOnly diagnostics-clear failure).
    // ----------------------------------------------------------------
    test('store.reset() stops the replaced LSP client — no zombie publisher (#264)', async function () {
        this.timeout(RESTART_TEST_TIMEOUT_MS);
        const store = getStore();
        assert.ok(store, 'Store should be available');
        await pollUntilReady(LSP_RESTART_WAIT_MS);

        const oldClient = store.client.value;
        assert.ok(oldClient, 'a running LSP client should exist before reset');
        assert.strictEqual(oldClient.isRunning(), true, 'old client should be running before reset');

        store.reset();

        // The onReset hook must bring up a replacement client.
        await pollUntilReady(LSP_RESTART_WAIT_MS);
        const newClient = store.client.value;
        assert.ok(newClient, 'reset() must start a replacement client');
        assert.notStrictEqual(newClient, oldClient, 'reset() must create a new client instance');

        // The replaced client must stop; a still-running one is a zombie
        // publisher (GitHub #264).
        const deadline = Date.now() + ZOMBIE_STOP_TIMEOUT_MS;
        while (oldClient.isRunning() && Date.now() < deadline) {
            await delay(100);
        }
        assert.strictEqual(
            oldClient.isRunning(),
            false,
            'store.reset() must stop the replaced LSP client — a running one keeps ' +
            'publishing stale diagnostics from its own collection (GitHub #264)'
        );
    });

    // ----------------------------------------------------------------
    // 11. CROSS-SESSION: deactivate → activate cycle
    //
    //     Simulates VS Code window reload. After deactivate(), all
    //     command registrations must be disposed. After activate(),
    //     they must be re-registered without errors.
    // ----------------------------------------------------------------
    test('CROSS-SESSION: deactivate then activate does not throw duplicate command errors', async function () {
        this.timeout(RESTART_TEST_TIMEOUT_MS);

        const storeBefore = getStore();
        assert.ok(storeBefore, 'Store should exist in session 1');
        for (const cmd of CLIENT_COMMANDS) {
            assert.ok(
                storeBefore.isClientCommandRegistered(cmd),
                `Session 1: "${cmd}" should be registered`
            );
        }

        const stopPromise = deactivate();
        if (stopPromise !== undefined) {
            await stopPromise;
        }

        assert.strictEqual(
            getStore(),
            undefined,
            'Store should be undefined after deactivate()'
        );

        for (const cmd of CLIENT_COMMANDS) {
            assertCanRegister(cmd, 'After deactivate() — THIS IS THE BUG if it fails');
        }

        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension should still be installed');
        await ext.activate();

        const storeAfter = getStore();
        assert.ok(storeAfter, 'Store should exist in session 2');
        await pollUntilReady(LSP_RESTART_WAIT_MS);

        for (const cmd of CLIENT_COMMANDS) {
            assert.ok(storeAfter.isClientCommandRegistered(cmd), `Session 2: "${cmd}" should be registered`);
        }
        for (const cmd of CLIENT_COMMANDS) {
            await assertExecutable(cmd, 'Session 2');
        }
        for (const cmd of CLIENT_COMMANDS) {
            assertCannotRegister(cmd, 'Session 2 duplicate check');
        }
    });

    // ----------------------------------------------------------------
    // 12. CROSS-SESSION: three consecutive deactivate/activate cycles
    // ----------------------------------------------------------------
    test('CROSS-SESSION: three consecutive deactivate/activate cycles', async function () {
        this.timeout(MULTI_CYCLE_COUNT * RESTART_TEST_TIMEOUT_MS);

        for (let cycle = 1; cycle <= MULTI_CYCLE_COUNT; cycle++) {
            const tag = `Cycle ${cycle}`;

            const stopPromise = deactivate();
            if (stopPromise !== undefined) {
                await stopPromise;
            }
            assert.strictEqual(getStore(), undefined, `${tag}: store should be undefined after deactivate`);

            for (const cmd of CLIENT_COMMANDS) {
                assertCanRegister(cmd, tag);
            }

            const ext = vscode.extensions.getExtension(EXTENSION_ID);
            assert.ok(ext, `${tag}: extension should still be installed`);
            await ext.activate();

            const store = getStore();
            assert.ok(store, `${tag}: store should exist after activate`);
            await pollUntilReady(LSP_RESTART_WAIT_MS);

            for (const cmd of CLIENT_COMMANDS) {
                assert.ok(store.isClientCommandRegistered(cmd), `${tag}: "${cmd}" should be registered`);
            }
        }
    });

    // ----------------------------------------------------------------
    // 13. CROSS-SESSION: all server commands re-advertised after cycle
    // ----------------------------------------------------------------
    test('CROSS-SESSION: all server commands re-advertised after deactivate/activate', async function () {
        this.timeout(RESTART_TEST_TIMEOUT_MS);

        // Ensure we start in a good state.
        await pollUntilReady(LSP_RESTART_WAIT_MS);
        const storeBefore = getStore();
        assert.ok(storeBefore, 'Store should exist in session 1');
        const session1Commands = new Set(storeBefore.serverCommands.value);
        assert.ok(session1Commands.size > 0, 'Server should advertise at least one command');

        const stopPromise = deactivate();
        if (stopPromise !== undefined) {
            await stopPromise;
        }
        assert.strictEqual(getStore(), undefined, 'Store should be undefined after deactivate');

        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension should still be installed');
        await ext.activate();

        // Trigger re-init and wait for server commands.
        await pollUntilReady(LSP_RESTART_WAIT_MS);

        const storeAfter = getStore();
        assert.ok(storeAfter, 'Store should exist in session 2');

        for (const cmd of session1Commands) {
            assert.ok(
                storeAfter.isServerCommandAdvertised(cmd),
                `Server command "${cmd}" was in session 1 but missing in session 2`
            );
        }

        assert.strictEqual(
            storeAfter.serverCommands.value.size,
            session1Commands.size,
            `Session 2 should have ${session1Commands.size} server commands, ` +
            `got ${storeAfter.serverCommands.value.size}`
        );
    });

    // ----------------------------------------------------------------
    // 14. CROSS-SESSION: server commands survive three rapid cycles
    //
    //     Snapshots the server commands from session 0, then verifies
    //     they all re-appear after each deactivate/activate cycle.
    //     This tests the actual binary's command set, not the hardcoded
    //     SERVER_COMMANDS list (which may include commands from a newer
    //     build).
    // ----------------------------------------------------------------
    test('CROSS-SESSION: server commands survive three rapid deactivate/activate cycles', async function () {
        this.timeout(MULTI_CYCLE_COUNT * RESTART_TEST_TIMEOUT_MS);

        // Snapshot from current session.
        await pollUntilReady(LSP_RESTART_WAIT_MS);
        const baseline = getStore();
        assert.ok(baseline, 'Baseline store should exist');
        const baselineCommands = new Set(baseline.serverCommands.value);
        assert.ok(baselineCommands.size > 0, 'Baseline should have server commands');

        for (let cycle = 1; cycle <= MULTI_CYCLE_COUNT; cycle++) {
            const tag = `Cycle ${cycle}`;

            const stopPromise = deactivate();
            if (stopPromise !== undefined) {
                await stopPromise;
            }
            assert.strictEqual(getStore(), undefined, `${tag}: store should be undefined after deactivate`);

            const ext = vscode.extensions.getExtension(EXTENSION_ID);
            assert.ok(ext, `${tag}: extension should still be installed`);
            await ext.activate();

            await pollUntilReady(LSP_RESTART_WAIT_MS);
            const store = getStore();
            assert.ok(store, `${tag}: store should exist after activate`);

            for (const cmd of baselineCommands) {
                assert.ok(
                    store.isServerCommandAdvertised(cmd),
                    `${tag}: server command "${cmd}" should be advertised`
                );
            }

            for (const cmd of CLIENT_COMMANDS) {
                assert.ok(
                    store.isClientCommandRegistered(cmd),
                    `${tag}: client command "${cmd}" should be registered`
                );
            }
        }
    });

    // ----------------------------------------------------------------
    // 15. CROSS-SESSION: client commands are executable after refresh
    // ----------------------------------------------------------------
    test('CROSS-SESSION: client commands are executable after deactivate/activate', async function () {
        this.timeout(RESTART_TEST_TIMEOUT_MS);

        const stopPromise = deactivate();
        if (stopPromise !== undefined) {
            await stopPromise;
        }
        assert.strictEqual(getStore(), undefined, 'Store should be undefined after deactivate');

        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension should still be installed');
        await ext.activate();

        await pollUntilReady(LSP_RESTART_WAIT_MS);

        for (const cmd of CLIENT_COMMANDS) {
            await assertExecutable(cmd, 'After deactivate/activate');
        }
    });
});
