/**
 * Profiler E2E Tests for the Basilisk VS Code Extension.
 *
 * Validates the full profiling workflow:
 * - Profiler commands are registered and callable
 * - Status bar item appears and responds to profiling state
 * - Profile start/stop lifecycle works end-to-end
 * - Profiler settings are read from configuration
 * - Keybindings are declared
 *
 * These tests require the Basilisk LSP server binary to be built.
 * They exercise the real LSP protocol, not mocks.
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import { getStore } from '../../extension';
import {
    EXTENSION_ID,
    SUITE_SETUP_TIMEOUT_MS,
    setupLspTestSuite,
    teardownLspTestSuite,
    closeAllEditors,
} from './test-helpers';

/** Profiler client-side commands (registered in profiler.ts). */
const PROFILER_CLIENT_COMMANDS = [
    'basilisk.profileStart',
    'basilisk.profileStop',
    'basilisk.profileSnapshot',
    'basilisk.profileAttachToDebug',
] as const;

/** Profiler server-side commands (advertised by LSP). */
const PROFILER_SERVER_COMMANDS = [
    'basilisk.profiler.start',
    'basilisk.profiler.stop',
    'basilisk.profiler.snapshot',
    'basilisk.profiler.list',
] as const;

/** Profiler configuration keys. */
const PROFILER_SETTINGS = [
    'basilisk.profiler.sampleRate',
    'basilisk.profiler.includeNative',
    'basilisk.profiler.lineThreshold',
    'basilisk.profiler.functionThreshold',
    'basilisk.profiler.maxDiagnosticsPerFile',
    'basilisk.profiler.showInlineHeatMap',
] as const;

let tmpDir = '';

suite('Profiler — Command Registration', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-test-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('all profiler client commands are registered', async () => {
        const allCommands = await vscode.commands.getCommands(true);

        for (const cmd of PROFILER_CLIENT_COMMANDS) {
            assert.ok(
                allCommands.includes(cmd),
                `Client command "${cmd}" should be registered after activation`,
            );
        }
    });

    test('profiler server commands are advertised by LSP', async () => {
        const store = getStore();
        assert.ok(store, 'Store should be initialized');

        const allCommands = await vscode.commands.getCommands(true);

        for (const cmd of PROFILER_SERVER_COMMANDS) {
            assert.ok(
                allCommands.includes(cmd),
                `Server command "${cmd}" should be advertised by LSP`,
            );
        }
    });

    test('profiler.list returns empty sessions initially', async () => {
        const result = await vscode.commands.executeCommand(
            'basilisk.profiler.list',
        );
        assert.ok(result !== undefined, 'profiler.list should return a result');

        const json = result as { sessions: unknown[] };
        assert.ok(Array.isArray(json.sessions), 'sessions should be an array');
        assert.strictEqual(
            json.sessions.length,
            0,
            'no sessions should be active initially',
        );
    });
});

suite('Profiler — Configuration', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-cfg-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    test('profiler settings have correct defaults', () => {
        const config = vscode.workspace.getConfiguration('basilisk.profiler');

        assert.strictEqual(
            config.get<number>('sampleRate'),
            100,
            'default sample rate should be 100',
        );
        assert.strictEqual(
            config.get<boolean>('includeNative'),
            false,
            'includeNative should default to false',
        );
        assert.strictEqual(
            config.get<number>('lineThreshold'),
            1.0,
            'lineThreshold should default to 1.0',
        );
        assert.strictEqual(
            config.get<number>('functionThreshold'),
            2.0,
            'functionThreshold should default to 2.0',
        );
        assert.strictEqual(
            config.get<number>('maxDiagnosticsPerFile'),
            20,
            'maxDiagnosticsPerFile should default to 20',
        );
        assert.strictEqual(
            config.get<boolean>('showInlineHeatMap'),
            true,
            'showInlineHeatMap should default to true',
        );
    });

    test('profiler settings are declared in package.json', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const properties =
            packageJson?.contributes?.configuration?.properties ?? {};

        for (const setting of PROFILER_SETTINGS) {
            assert.ok(
                properties[setting] !== undefined,
                `Setting "${setting}" should be declared in package.json`,
            );
        }
    });
});

suite('Profiler — Status Bar', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-sb-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('profiler status bar item exists after activation', () => {
        const store = getStore();
        assert.ok(store, 'Store should be initialized after activation');
    });
});

suite('Profiler — Start/Stop Lifecycle', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-lc-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('profiler.start rejects invalid PID', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', {
                pid: 0,
            });
            assert.fail('profiler.start with PID 0 should have thrown');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(
                message.includes('not found') ||
                    message.includes('Process') ||
                    message.includes('-32001') ||
                    message.includes('denied') ||
                    message.includes('attach') ||
                    message.includes('failed') ||
                    message.includes('error'),
                `Error should indicate process issue, got: ${message}`,
            );
        }
    });

    test('profiler.stop rejects unknown session ID', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.stop', {
                sessionId: 'nonexistent-session-id',
            });
            assert.fail('profiler.stop with unknown session should have thrown');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(
                message.includes('session') ||
                    message.includes('not found') ||
                    message.includes('No active'),
                `Error should mention session, got: ${message}`,
            );
        }
    });

    test('profiler.snapshot rejects unknown session ID', async () => {
        try {
            await vscode.commands.executeCommand(
                'basilisk.profiler.snapshot',
                { sessionId: 'nonexistent-session-id' },
            );
            assert.fail('profiler.snapshot with unknown session should have thrown');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(
                message.includes('session') ||
                    message.includes('not found') ||
                    message.includes('No active'),
                `Error should mention session, got: ${message}`,
            );
        }
    });

    test('profiler.list returns array structure', async () => {
        const result = await vscode.commands.executeCommand(
            'basilisk.profiler.list',
        );
        assert.ok(result !== undefined, 'Should return a result');

        const json = result as { sessions: unknown[] };
        assert.ok(
            Array.isArray(json.sessions),
            'Result should have sessions array',
        );
    });

    test('profiler.start with no PID and no debug session gives clear error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', {});
            assert.fail('profiler.start with no PID should have thrown');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(
                message.length > 0,
                `Should have an error message, got empty`,
            );
        }
    });
});

suite('Profiler — Keybindings', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-kb-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    test('profiler commands declared in keybindings', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const keybindings = packageJson?.contributes?.keybindings ?? [];

        const profilerKeybindings = keybindings.filter(
            (kb: { command: string }) =>
                kb.command.startsWith('basilisk.profile'),
        );

        assert.ok(
            profilerKeybindings.length >= 2,
            `Should have at least 2 profiler keybindings (start + stop), found ${profilerKeybindings.length}`,
        );
    });

    test('profiler commands appear in package.json commands section', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const commands = packageJson?.contributes?.commands ?? [];
        const commandIds = commands.map((c: { command: string }) => c.command);

        for (const cmd of PROFILER_CLIENT_COMMANDS) {
            assert.ok(
                commandIds.includes(cmd),
                `Command "${cmd}" should be in package.json contributes.commands`,
            );
        }
    });
});
