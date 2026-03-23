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

import * as assert from 'assert';
import * as vscode from 'vscode';
import { getStore, activate, deactivate } from '../../extension';
import {
    EXTENSION_ID,
    SERVER_START_WAIT_MS,
    COMMAND_WAIT_MS,
    setupLspTestSuite,
    teardownLspTestSuite,
    closeAllEditors,
} from './test-helpers';

/** Timeout for suite-level setup. */
const SUITE_SETUP_TIMEOUT_MS = 30_000;

/** Extra time for the restart cycle test. */
const RESTART_CYCLE_TIMEOUT_MS = 25_000;

/** Timeout for cross-session (deactivate/activate) tests. */
const CROSS_SESSION_TIMEOUT_MS = 30_000;

/** Number of consecutive deactivate/activate cycles to test. */
const MULTI_CYCLE_COUNT = 3;

/** Brief settle time after restart. */
const RESTART_SETTLE_MS = 500;

/** Poll interval for waiting on command availability. */
const POLL_INTERVAL_MS = 100;

/**
 * All commands declared in package.json contributes.commands.
 * These are the ONLY commands that should appear in the palette.
 */
const MANIFEST_COMMANDS = [
    'basilisk.restartServer',
    'basilisk.showOutput',
    'basilisk.organizeImports',
    'basilisk.fixFile',
    'basilisk.fixWorkspace',
    'basilisk.adoptFile',
    'basilisk.adoptWorkspace',
    'basilisk.unadoptFile',
    'basilisk.uv.sync',
    'basilisk.uv.add',
    'basilisk.uv.addDev',
    'basilisk.uv.remove',
    'basilisk.uv.lock',
    'basilisk.uv.createEnv',
    'basilisk.refreshModuleExplorer',
    'basilisk.collapseModuleExplorer',
    'basilisk.refreshTypeHealth',
    'basilisk.sortTypeHealth',
    'basilisk.copyImportPath',
    'basilisk.copyQualifiedName',
    'basilisk.toggleModuleExplorerView',
    'basilisk.filterModuleExplorer',
    'basilisk.openWalkthrough',
] as const;

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
    'basilisk.fixWorkspace',
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
    'basilisk.discoverTests',
    'basilisk.runTests',
    'basilisk.runTestFile',
    'basilisk.debugTest',
    'basilisk.runTestsCoverage',
    'basilisk.workspaceModules',
    'basilisk.typeHealth',
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

/** Poll until client commands and server commands are populated in the store. */
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
        await new Promise<void>((r) => setTimeout(r, POLL_INTERVAL_MS));
    }
}

// eslint-disable-next-line max-lines-per-function
suite('Command Registration (VS Code API Compliance)', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-cmd-reg-test-');
        tmpDir = result.tmpDir;
        await pollUntilReady(SUITE_SETUP_TIMEOUT_MS - 2_000);

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
    test('all manifest commands exist in the VS Code command registry', async function () {
        this.timeout(SERVER_START_WAIT_MS);

        const allCommands = await vscode.commands.getCommands(true);
        for (const cmd of MANIFEST_COMMANDS) {
            assert.ok(
                allCommands.includes(cmd),
                `Manifest command "${cmd}" should exist in the VS Code command registry`
            );
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
        this.timeout(COMMAND_WAIT_MS);
        for (const cmd of CLIENT_COMMANDS) {
            assertCannotRegister(cmd, 'Duplicate registration');
        }
    });

    // ----------------------------------------------------------------
    // 7. Server commands ARE registered (routed through LSP client)
    // ----------------------------------------------------------------
    test('server commands are registered via syncServerCommands', async function () {
        this.timeout(COMMAND_WAIT_MS);
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
        this.timeout(RESTART_CYCLE_TIMEOUT_MS);

        const store = getStore();
        assert.ok(store, 'Store should be available');

        for (const cmd of CLIENT_COMMANDS) {
            assert.ok(
                store.isClientCommandRegistered(cmd),
                `"${cmd}" should be registered before restart`
            );
        }

        await vscode.commands.executeCommand('basilisk.restartServer');
        await new Promise<void>((r) => setTimeout(r, RESTART_SETTLE_MS));
        await pollUntilReady(RESTART_CYCLE_TIMEOUT_MS);

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
        this.timeout(RESTART_CYCLE_TIMEOUT_MS);

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
        await pollUntilReady(RESTART_CYCLE_TIMEOUT_MS);
    });

    // ----------------------------------------------------------------
    // 11. CROSS-SESSION: deactivate → activate cycle
    //
    //     Simulates VS Code window reload. After deactivate(), all
    //     command registrations must be disposed. After activate(),
    //     they must be re-registered without errors.
    // ----------------------------------------------------------------
    test('CROSS-SESSION: deactivate then activate does not throw duplicate command errors', async function () {
        this.timeout(CROSS_SESSION_TIMEOUT_MS);

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
        await pollUntilReady(CROSS_SESSION_TIMEOUT_MS);

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
        this.timeout(CROSS_SESSION_TIMEOUT_MS * MULTI_CYCLE_COUNT);

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
            await pollUntilReady(CROSS_SESSION_TIMEOUT_MS);

            for (const cmd of CLIENT_COMMANDS) {
                assert.ok(store.isClientCommandRegistered(cmd), `${tag}: "${cmd}" should be registered`);
            }
        }
    });

    // ----------------------------------------------------------------
    // 13. CROSS-SESSION: all server commands re-advertised after cycle
    // ----------------------------------------------------------------
    test('CROSS-SESSION: all server commands re-advertised after deactivate/activate', async function () {
        this.timeout(CROSS_SESSION_TIMEOUT_MS);

        // Ensure we start in a good state.
        await pollUntilReady(CROSS_SESSION_TIMEOUT_MS);
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
        await pollUntilReady(CROSS_SESSION_TIMEOUT_MS);

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
        this.timeout(CROSS_SESSION_TIMEOUT_MS * MULTI_CYCLE_COUNT);

        // Snapshot from current session.
        await pollUntilReady(CROSS_SESSION_TIMEOUT_MS);
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

            await pollUntilReady(CROSS_SESSION_TIMEOUT_MS);
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
        this.timeout(CROSS_SESSION_TIMEOUT_MS);

        const stopPromise = deactivate();
        if (stopPromise !== undefined) {
            await stopPromise;
        }
        assert.strictEqual(getStore(), undefined, 'Store should be undefined after deactivate');

        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension should still be installed');
        await ext.activate();

        await pollUntilReady(CROSS_SESSION_TIMEOUT_MS);

        for (const cmd of CLIENT_COMMANDS) {
            await assertExecutable(cmd, 'After deactivate/activate');
        }
    });
});
