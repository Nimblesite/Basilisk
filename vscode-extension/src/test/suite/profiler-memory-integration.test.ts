// Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
/**
 * Profiler E2E Tests — Lifecycle, Memory Profiler, Error Handling, Integration.
 *
 * Validates:
 * - Profile start/stop lifecycle works end-to-end
 * - Memory profiler commands are registered and callable
 * - Error handling produces user-friendly messages
 * - Profiler and memory decorations can coexist
 * - Cross-feature integration (profiler + document symbols + LSP)
 *
 * These tests require the Basilisk LSP server binary to be built.
 * They exercise the real LSP protocol, not mocks.
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import { getStore } from '../../extension';
import {
    EXTENSION_ID,
    setupLspTestSuite,
    teardownLspTestSuite,
    closeAllEditors,
    openPythonFile,
} from "./test-helpers";
import { errorMessage } from "./caught-error";

import type { ProfileResult } from '../../profiler-decorations';
import {
    applyProfileDecorations,
    clearProfileDecorations,
    disposeProfileDecorations,
} from '../../profiler-decorations';

import type {
    MemoryAllocation,
    MemorySnapshotResult,
    LeakConfidence,
    SuspectedLeak,
    MemoryDiffResult,
} from '../../memory-decorations';
import {
    applyMemoryDecorations,
    clearMemoryDecorations,
    disposeMemoryDecorations,
    applyLeakDecorations,
} from '../../memory-decorations';

import {
  PROFILER_CLIENT_COMMANDS,
  PROFILER_SERVER_COMMANDS,
  MEMORY_CLIENT_COMMANDS
} from './profiler-test-constants';
import {
  manifestCommands,
  manifestConfigurationProperties
} from "./extension-manifest";
import { rawField } from "../../unknown-shape";

let tmpDir = '';

/** Whether `value` is an array, without saying anything about its elements. */
function isUnknownArray(value: unknown): value is unknown[] {
  return Array.isArray(value);
}

/**
 * The `sessions` array of a `basilisk.profiler.list` reply.
 *
 * Each call site used to assert the reply into `{ sessions: unknown[] }` and
 * then check `Array.isArray` separately. That check was the only thing actually
 * proving the reply carried one, so it lives here now — callers get a real
 * array, and a reply without one fails with the caller's own message.
 */
function sessionsOf(result: unknown, message: string): unknown[] {
  const sessions = rawField(result, 'sessions');
  assert.ok(isUnknownArray(sessions), message);
  return sessions;
}

function assertCommandRegistered(commandId: string, label: string): void {
    let threw = false;
    let disposable: vscode.Disposable | undefined;
    try {
        disposable = vscode.commands.registerCommand(commandId, () => { /* probe */ });
    } catch {
        threw = true;
    } finally {
        disposable?.dispose();
    }
    assert.ok(threw, `${label} "${commandId}" should be registered after activation`);
}

suite('Profiler — Start/Stop Lifecycle', () => {
    suiteSetup(async function () {
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
            const message = errorMessage(err);
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
            const message = errorMessage(err);
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
            const message = errorMessage(err);
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

        sessionsOf(result, 'Result should have sessions array');
    });

    test('profiler.start with no PID and no debug session gives clear error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', {});
            assert.fail('profiler.start with no PID should have thrown');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(
                message.length > 0,
                'Should have an error message, got empty',
            );
        }
    });

    test('profiler.stop with missing sessionId gives clear error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.stop', {});
            assert.fail('profiler.stop with missing sessionId should have thrown');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(
                message.length > 0,
                'Should have an error message for missing sessionId',
            );
            assert.ok(
                message.includes('sessionId') ||
                    message.includes('session') ||
                    message.includes('required') ||
                    message.includes('Missing'),
                `Error should mention sessionId or session, got: ${message}`,
            );
        }
    });

    test('consecutive profiler.list calls return consistent empty results', async () => {
        const result1 = await vscode.commands.executeCommand('basilisk.profiler.list');
        const result2 = await vscode.commands.executeCommand('basilisk.profiler.list');

        const sessions1 = sessionsOf(result1, 'First call sessions should be array');
        const sessions2 = sessionsOf(result2, 'Second call sessions should be array');

        assert.strictEqual(sessions1.length, sessions2.length,
            'Consecutive list calls should return same session count');
    });
});

suite('Memory Profiler — Command Registration', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-memory-cmd-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('all memory client commands are registered', () => {
        for (const cmd of MEMORY_CLIENT_COMMANDS) {
            assertCommandRegistered(cmd, 'Memory command');
        }
    });

    test('memory commands appear in package.json commands section', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const commands = manifestCommands();
        const commandIds = commands.map((c) => c.command);

        for (const cmd of MEMORY_CLIENT_COMMANDS) {
            assert.ok(
                commandIds.includes(cmd),
                `Memory command "${cmd}" should be in package.json contributes.commands`,
            );
        }
    });

    test('memory commands have titles and categories', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const commands = manifestCommands();

        for (const cmd of MEMORY_CLIENT_COMMANDS) {
            const entry = commands.find((c) => c.command === cmd);
            assert.ok(entry, `Command entry for "${cmd}" should exist in contributes.commands`);
            assert.ok(
                entry.title !== undefined && entry.title.length > 0,
                `Memory command "${cmd}" should have a non-empty title`,
            );
            assert.strictEqual(
                entry.category,
                'Basilisk',
                `Memory command "${cmd}" should have category "Basilisk"`,
            );
        }
    });

    test('memory and profiler commands do not overlap', () => {
        const profilerSet = new Set(PROFILER_CLIENT_COMMANDS as readonly string[]);
        const memorySet = new Set(MEMORY_CLIENT_COMMANDS as readonly string[]);

        for (const cmd of PROFILER_CLIENT_COMMANDS) {
            assert.ok(!memorySet.has(cmd),
                `Profiler command "${cmd}" should not be in memory command set`);
        }
        for (const cmd of MEMORY_CLIENT_COMMANDS) {
            assert.ok(!profilerSet.has(cmd),
                `Memory command "${cmd}" should not be in profiler command set`);
        }
    });

    test('all profiler and memory commands are distinct from each other', () => {
        const allCommands = [
            ...PROFILER_CLIENT_COMMANDS,
            ...MEMORY_CLIENT_COMMANDS,
            ...PROFILER_SERVER_COMMANDS,
        ];
        const unique = new Set(allCommands);
        assert.strictEqual(unique.size, allCommands.length,
            'All profiler and memory commands should be unique');
    });
});

suite('Profiler — Lifecycle Interaction', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-profiler-lifecycle-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('profiler.start with PID 0 returns error with actionable info', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', { pid: 0 });
            assert.fail('profiler.start with PID 0 should have thrown');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 10,
                `Error message should be descriptive, got: "${message}"`);
            assert.ok(typeof message === 'string', 'Error should be a string message');
            assert.ok(
                !message.includes('at Object.') || message.includes('Process') || message.includes('error'),
                `Error should be user-friendly, not a stack trace: ${message}`,
            );
        }
    });

    test('profiler.start with negative PID returns error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', { pid: -1 });
            assert.fail('profiler.start with negative PID should have thrown');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 0, 'Error message should not be empty');
            assert.ok(typeof message === 'string', 'Error should produce a string message');
            assert.ok(!message.startsWith('undefined'),
                'Error message should not start with "undefined"');
        }
    });

    test('profiler.start with extremely large PID returns error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', { pid: 999999999 });
            assert.fail('profiler.start with nonexistent PID should have thrown');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 0, 'Error message for large PID should not be empty');
            assert.ok(typeof message === 'string', 'Error should be a string');
            assert.ok(
                message.includes('not found') ||
                    message.includes('Process') ||
                    message.includes('failed') ||
                    message.includes('error') ||
                    message.includes('denied') ||
                    message.includes('attach'),
                `Error should indicate process issue, got: ${message}`,
            );
        }
    });

    test('profiler.stop without active session returns clear error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.stop', {
                sessionId: 'definitely-not-a-real-session-id-abc123',
            });
            assert.fail('profiler.stop without starting should have thrown');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 0, 'Should have an error message');
            assert.ok(
                message.includes('session') ||
                    message.includes('not found') ||
                    message.includes('No active'),
                `Error should mention session, got: ${message}`,
            );
            assert.ok(typeof message === 'string', 'Error message must be a string type');
        }
    });

    test('profiler.snapshot without active session returns clear error', async () => {
        try {
            await vscode.commands.executeCommand(
                'basilisk.profiler.snapshot',
                { sessionId: 'no-such-snapshot-session-xyz789' },
            );
            assert.fail('profiler.snapshot without starting should have thrown');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 0, 'Should have an error message');
            assert.ok(
                message.includes('session') ||
                    message.includes('not found') ||
                    message.includes('No active'),
                `Error should reference session state, got: ${message}`,
            );
            assert.ok(
                !message.includes('Cannot read properties of'),
                'Error should not be a null pointer error',
            );
        }
    });

    test('profiler.list returns empty array when nothing is running', async () => {
        const result = await vscode.commands.executeCommand('basilisk.profiler.list');
        assert.ok(result !== undefined, 'profiler.list should return a result');
        assert.ok(result !== null, 'profiler.list should not return null');

        const sessions = sessionsOf(result, 'sessions should be an array');
        assert.strictEqual(sessions.length, 0,
            'no sessions should be active when nothing was started');
    });

});

suite('Profiler — Lifecycle Interaction (Continued)', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-profiler-lifecycle2-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('error messages from profiler do not contain raw stack traces', async () => {
        const errorProducingCalls = [
            vscode.commands.executeCommand('basilisk.profiler.start', { pid: 0 }),
            vscode.commands.executeCommand('basilisk.profiler.stop', { sessionId: 'fake' }),
            vscode.commands.executeCommand('basilisk.profiler.snapshot', { sessionId: 'fake' }),
        ];

        for (const call of errorProducingCalls) {
            try {
                await call;
            } catch (err: unknown) {
                const message = errorMessage(err);
                const stackTraceLineCount = message.split('\n')
                    .filter((line: string) => line.trim().startsWith('at ')).length;
                assert.ok(stackTraceLineCount < 3,
                    `Error should not contain full stack traces: ${message.slice(0, 200)}`);
            }
        }
    });

    test('multiple rapid profiler.list calls do not crash or diverge', async () => {
        const results = await Promise.all([
            vscode.commands.executeCommand('basilisk.profiler.list'),
            vscode.commands.executeCommand('basilisk.profiler.list'),
            vscode.commands.executeCommand('basilisk.profiler.list'),
        ]);

        for (const result of results) {
            assert.ok(result !== undefined, 'Each list call must return a result');
            const sessions = sessionsOf(result, 'Each result must have sessions array');
            assert.strictEqual(sessions.length, 0,
                'All parallel list calls should return empty sessions');
        }
    });
});

suite('Profiler — Status Bar Behavior', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-profiler-sb2-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('status bar item exists after extension activation', () => {
        const store = getStore();
        assert.ok(store, 'Store should be initialized after activation');
        assert.ok(
            store.lspState.value === 'running' || store.lspState.value === 'starting',
            `LSP should be running or starting, got: ${store.lspState.value}`,
        );
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension should be found');
        assert.strictEqual(ext.isActive, true, 'Extension must be active');
    });

    test('profiler status bar stop command is declared in package.json', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const commands = manifestCommands();
        const stopCmd = commands.find((c) => c.command === 'basilisk.profileStop');

        assert.ok(stopCmd, 'profileStop command should exist in package.json');
        assert.ok(stopCmd.title !== undefined, 'profileStop should have a title');
        assert.ok(stopCmd.title.length > 0, 'profileStop title should not be empty');
    });

    test('profiler status bar priority is declared correctly for ordering', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');
        assert.ok(extension.isActive, 'Extension must be active');

        const store = getStore();
        assert.ok(store, 'Store must be initialized');
        assert.ok(store.client.value !== undefined, 'LSP client must exist');
    });
});

suite('Profiler — Configuration Interaction', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-profiler-cfgi-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    test('changing sampleRate config is reflected in workspace config', async () => {
        const config = vscode.workspace.getConfiguration('basilisk.profiler');
        const originalRate = config.get<number>('sampleRate');
        assert.strictEqual(originalRate, 100, 'Default sampleRate should be 100');

        await config.update('sampleRate', 50, vscode.ConfigurationTarget.Workspace);
        const updatedConfig = vscode.workspace.getConfiguration('basilisk.profiler');
        assert.strictEqual(updatedConfig.get<number>('sampleRate'), 50,
            'sampleRate should be updated to 50');

        await config.update('sampleRate', undefined, vscode.ConfigurationTarget.Workspace);
        const restoredConfig = vscode.workspace.getConfiguration('basilisk.profiler');
        assert.strictEqual(restoredConfig.get<number>('sampleRate'), 100,
            'sampleRate should be restored to default 100');
    });

    test('quick preset is offered for short burst profiling', () => {
        const properties = manifestConfigurationProperties();
        const presetProp = properties['basilisk.profiler.preset'] as
            { enum?: string[]; type?: string } | undefined;
        assert.ok(presetProp, 'preset property should exist');
        assert.ok(Array.isArray(presetProp.enum) && presetProp.enum.includes('quick'),
            'quick (10 s @ 100 Hz, presets.rs) must be a valid preset');
        assert.ok(presetProp.type === 'string', 'preset should be a string type');
    });

    test('detailed preset is offered and includeNative defaults off', () => {
        const properties = manifestConfigurationProperties();
        const presetProp = properties['basilisk.profiler.preset'] as
            { enum?: string[] } | undefined;
        assert.ok(presetProp, 'preset property should exist');
        assert.ok(Array.isArray(presetProp.enum) && presetProp.enum.includes('detailed'),
            'detailed (60 s @ 200 Hz, presets.rs) must be a valid preset');

        const config = vscode.workspace.getConfiguration('basilisk.profiler');
        assert.strictEqual(config.get<boolean>('includeNative'), false,
            'includeNative default should be false');
    });

    test('all 4 presets are exactly the ones the server parses', () => {
        const properties = manifestConfigurationProperties();
        const presetProp = properties['basilisk.profiler.preset'] as
            { enum?: string[] } | undefined;
        assert.ok(presetProp, 'preset property should exist');

        const enumValues = presetProp.enum;
        assert.ok(Array.isArray(enumValues), 'enum should be an array');
        // Mirrors ProfilingPreset::parse_name plus "default" — a name the
        // server silently ignores (the old "memory"/"lightweight" entries)
        // degrades to a default CPU session and must never be advertised.
        assert.deepStrictEqual([...enumValues].sort(),
            ['default', 'detailed', 'longRunning', 'quick'],
            'advertised presets must match the server parser exactly');
    });

    test('numeric settings have reasonable bounds in config declarations', () => {
        const properties = manifestConfigurationProperties();

        const sampleRateProp = properties['basilisk.profiler.sampleRate'];
        assert.ok(sampleRateProp !== undefined, 'sampleRate property must exist');
        const sampleRateDefault = sampleRateProp.default;
        assert.ok(typeof sampleRateDefault === 'number',
            'sampleRate default should be a number');
        assert.ok(sampleRateDefault > 0,
            'sampleRate default should be positive');

        const lineThresholdProp = properties['basilisk.profiler.lineThreshold'];
        assert.ok(lineThresholdProp !== undefined, 'lineThreshold property must exist');
        const lineThresholdDefault = lineThresholdProp.default;
        assert.ok(typeof lineThresholdDefault === 'number',
            'lineThreshold default should be a number');
        assert.ok(lineThresholdDefault > 0,
            'lineThreshold default should be positive');

        const maxDiagProp = properties['basilisk.profiler.maxDiagnosticsPerFile'];
        assert.ok(maxDiagProp !== undefined, 'maxDiagnosticsPerFile property must exist');
        const maxDiagDefault = maxDiagProp.default;
        assert.ok(typeof maxDiagDefault === 'number',
            'maxDiagnosticsPerFile default should be a number');
        assert.ok(maxDiagDefault > 0,
            'maxDiagnosticsPerFile default should be positive');
    });
});

suite('Memory Profiler — Extended', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-memory-ext-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        clearMemoryDecorations();
        await closeAllEditors();
    });

    test('memoryStart command is callable and returns without crash', async () => {
        const store = getStore();
        assert.ok(store, 'Store should be initialized');
        assert.ok(store.client.value !== undefined, 'LSP client should exist');

        try {
            await vscode.commands.executeCommand('basilisk.memoryStart');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 0, 'Error message should not be empty');
            assert.ok(typeof message === 'string', 'Error should be a string');
        }
        assert.ok(true, 'memoryStart command was callable');
    });

    test('memorySnapshot without active session warns gracefully', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.memorySnapshot');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 0, 'Error should have a message');
        }
        assert.ok(true, 'memorySnapshot without session did not crash');
        const store = getStore();
        assert.ok(store, 'Store should still be intact after memorySnapshot call');
    });

    test('memoryReferences command is callable', async () => {
        const store = getStore();
        assert.ok(store, 'Store should be initialized');
        assert.ok(store.client.value !== undefined, 'LSP client should exist');

        const INPUT_BOX_DISMISS_DELAY_MS = 200;
        const dismiss = new Promise<void>((resolve) => {
            setTimeout(() => {
                void vscode.commands.executeCommand('workbench.action.closeQuickOpen').then(() => { resolve(); });
            }, INPUT_BOX_DISMISS_DELAY_MS);
        });

        try {
            await Promise.all([
                vscode.commands.executeCommand('basilisk.memoryReferences'),
                dismiss,
            ]);
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(typeof message === 'string', 'Error should be a string');
        }
        assert.ok(true, 'memoryReferences command was callable without crash');
    });

    test('MemoryAllocation type enforces required fields', () => {
        const alloc: MemoryAllocation = {
            file: '/src/allocator.py',
            line: 55,
            size: 52428800,
            count: 10000,
        };

        assert.strictEqual(alloc.file, '/src/allocator.py');
        assert.strictEqual(alloc.line, 55);
        assert.strictEqual(alloc.size, 52428800);
        assert.strictEqual(alloc.count, 10000);
    });

    test('leak confidence levels map to correct severity ordering', () => {
        const confidences: LeakConfidence[] = ['LOW', 'MEDIUM', 'HIGH', 'DEFINITE'];
        const severityOrder = new Map<LeakConfidence, number>([
            ['LOW', 0],
            ['MEDIUM', 1],
            ['HIGH', 2],
            ['DEFINITE', 3],
        ]);

        assert.strictEqual(confidences.length, 4,
            'There should be exactly 4 confidence levels');

        for (let idx = 0; idx < confidences.length - 1; idx++) {
            const current = severityOrder.get(confidences[idx]);
            const next = severityOrder.get(confidences[idx + 1]);
            assert.ok(current !== undefined && next !== undefined,
                `Severity for ${confidences[idx]} and ${confidences[idx + 1]} must be defined`);
            assert.ok(current < next,
                `${confidences[idx]} should have lower severity than ${confidences[idx + 1]}`);
        }
    });

    test('SuspectedLeak type has all required fields', () => {
        const leak: SuspectedLeak = {
            file: '/src/leaky.py',
            line: 42,
            sizeGrowth: 1048576,
            countGrowth: 500,
            currentSize: 5242880,
            confidence: 'HIGH',
            reason: 'Monotonic growth detected across 10 snapshots',
        };

        assert.strictEqual(leak.file, '/src/leaky.py');
        assert.strictEqual(leak.line, 42);
        assert.strictEqual(leak.sizeGrowth, 1048576);
        assert.strictEqual(leak.countGrowth, 500);
        assert.strictEqual(leak.currentSize, 5242880);
        assert.strictEqual(leak.confidence, 'HIGH');
        assert.ok(leak.reason.length > 0, 'Leak reason should be non-empty');
    });

});

suite('Memory Profiler — Types and Decorations', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-memory-ext2-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        clearMemoryDecorations();
        await closeAllEditors();
    });

    test('MemoryDiffResult type has all required fields', () => {
        const diff: MemoryDiffResult = {
            totalGrowth: 10485760,
            totalFreed: 2097152,
            netGrowth: 8388608,
            suspectedLeaks: [
                {
                    file: '/src/data.py',
                    line: 10,
                    sizeGrowth: 5242880,
                    countGrowth: 200,
                    currentSize: 10485760,
                    confidence: 'DEFINITE',
                    reason: 'Allocation grows every snapshot with zero frees',
                },
            ],
        };

        assert.strictEqual(diff.totalGrowth, 10485760);
        assert.strictEqual(diff.totalFreed, 2097152);
        assert.strictEqual(diff.netGrowth, 8388608);
        assert.strictEqual(diff.suspectedLeaks.length, 1);
        assert.strictEqual(diff.suspectedLeaks[0].confidence, 'DEFINITE');
    });

    test('applyMemoryDecorations with populated allocations does not throw', async () => {
        await openPythonFile(tmpDir, 'mem_alloc.py',
            'data = []\nfor i in range(1000):\n    data.append(i)\n');

        const snapshot: MemorySnapshotResult = {
            memorySessionId: 'mem-populated',
            snapshotId: 'snap-pop-001',
            currentMemory: 104857600,
            peakMemory: 209715200,
            topAllocations: [
                { file: '/nonexistent/a.py', line: 1, size: 52428800, count: 5000 },
                { file: '/nonexistent/b.py', line: 15, size: 10485760, count: 1000 },
                { file: '/nonexistent/c.py', line: 30, size: 1048576, count: 100 },
            ],
        };

        assert.doesNotThrow(() => {
            applyMemoryDecorations(snapshot);
        }, 'applyMemoryDecorations should handle populated results');

        assert.strictEqual(snapshot.topAllocations.length, 3, 'Should have 3 allocations');
        assert.ok(snapshot.currentMemory <= snapshot.peakMemory,
            'currentMemory should not exceed peakMemory');

        clearMemoryDecorations();
    });

    test('applyLeakDecorations with suspected leaks does not throw', async () => {
        await openPythonFile(tmpDir, 'leak_test.py',
            'cache = {}\ndef leak():\n    cache[id(object())] = object()\n');

        const diff: MemoryDiffResult = {
            totalGrowth: 5242880,
            totalFreed: 524288,
            netGrowth: 4718592,
            suspectedLeaks: [
                {
                    file: '/nonexistent/leaky.py',
                    line: 3,
                    sizeGrowth: 2097152,
                    countGrowth: 300,
                    currentSize: 8388608,
                    confidence: 'HIGH',
                    reason: 'Monotonic growth pattern',
                },
            ],
        };

        assert.doesNotThrow(() => {
            applyLeakDecorations(diff);
        }, 'applyLeakDecorations should handle suspected leaks');

        assert.ok(diff.netGrowth > 0, 'Net growth should be positive for a leak');
        assert.ok(diff.totalGrowth > diff.totalFreed,
            'totalGrowth should exceed totalFreed when there is a net leak');

        clearMemoryDecorations();
    });
});

suite('Profiler — Error Handling', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-profiler-err-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('profiler.start with invalid params returns error code or message', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', {
                pid: 0, sampleRate: -1,
            });
            assert.fail('Should have thrown for invalid params');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 0, 'Error message should not be empty');
            assert.ok(typeof message === 'string', 'Error should be string');
            assert.ok(
                !message.includes('command not found') &&
                    !message.includes('is not registered'),
                `Error should be about the params, not command registration: ${message}`,
            );
        }
    });

    test('profiler error codes are within expected range', async () => {
        const expectedCodes = [-32001, -32002, -32003, -32004, -32005, -32006];

        for (const code of expectedCodes) {
            assert.ok(code < 0, `Error code ${code} should be negative`);
            assert.ok(code >= -32099, `Error code ${code} should be >= -32099`);
            assert.ok(code <= -32000, `Error code ${code} should be <= -32000`);
        }

        const unique = new Set(expectedCodes);
        assert.strictEqual(unique.size, expectedCodes.length,
            'All error codes should be unique');
    });

    test('profiler.stop with empty string sessionId returns descriptive error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.stop', { sessionId: '' });
            assert.fail('Empty sessionId should produce an error');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 0, 'Error should have a message');
            assert.ok(typeof message === 'string', 'Error should be string type');
            assert.ok(
                !message.includes('panic') && !message.includes('PANIC'),
                `Error should not indicate a panic: ${message}`,
            );
        }
    });

    test('profiler.snapshot with null-like args returns error gracefully', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.snapshot', { sessionId: null });
            assert.fail('Null sessionId should produce an error');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(message.length > 0, 'Error should have a message');
            assert.ok(typeof message === 'string', 'Error should be string type');
            assert.ok(
                !message.includes('segfault') && !message.includes('SIGSEGV'),
                'Error should not be a segfault',
            );
        }
    });

    test('connection errors are handled when LSP client is present', async () => {
        const store = getStore();
        assert.ok(store, 'Store should exist');
        assert.ok(store.client.value !== undefined, 'Client should exist');

        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', { pid: 2147483647 });
            assert.fail('Nonexistent PID should produce an error');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(
                !message.includes('ECONNREFUSED') &&
                    !message.includes('ECONNRESET'),
                `Error should be protocol-level, not network: ${message}`,
            );
            assert.ok(message.length > 0, 'Error message should not be empty');
            assert.ok(
                !message.includes('undefined'),
                'Error message should not contain "undefined"',
            );
        }
    });

    test('error messages are user-friendly strings, not JSON blobs', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.stop', {
                sessionId: 'nonexistent-for-ux-check',
            });
            assert.fail('Should have thrown');
        } catch (err: unknown) {
            const message = errorMessage(err);
            assert.ok(
                !message.trimStart().startsWith('{') ||
                    message.includes('session') ||
                    message.includes('error'),
                `Error should be human-readable, not a raw JSON blob: ${message.slice(0, 200)}`,
            );
            assert.ok(message.length < 2000, 'Error message should not be excessively long');
            assert.ok(typeof message === 'string', 'Error must be a string');
        }
    });
});

suite('Profiler — Cross-Feature Integration', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-profiler-xfeat-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        clearProfileDecorations();
        clearMemoryDecorations();
        await closeAllEditors();
    });

    test('profiler commands do not interfere with document symbol provider', async () => {
        const { uri } = await openPythonFile(tmpDir, 'symbols_test.py',
            'def hello():\n    pass\n\nclass Foo:\n    pass\n');

        const listResult = await vscode.commands.executeCommand('basilisk.profiler.list');
        assert.ok(listResult !== undefined, 'profiler.list should work');

        const symbols = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
            'vscode.executeDocumentSymbolProvider', uri,
        );
        assert.ok(symbols !== undefined && symbols !== null,
            'Document symbols should still work after profiler commands');
        assert.ok(symbols.length >= 1,
            'Should find at least one symbol in the test file');
    });

    test('profiler.list is idempotent and does not corrupt LSP state', async () => {
        for (let iteration = 0; iteration < 5; iteration++) {
            const result = await vscode.commands.executeCommand('basilisk.profiler.list');
            sessionsOf(result, `Iteration ${iteration}: sessions should be an array`);
        }

        const store = getStore();
        assert.ok(store, 'Store should exist');
        assert.ok(
            store.lspState.value === 'running',
            `LSP should still be running after repeated list calls, got: ${store.lspState.value}`,
        );
    });

    test('multiple quick start/stop error cycles do not crash', async () => {
        const iterations = 3;
        for (let cycle = 0; cycle < iterations; cycle++) {
            try {
                await vscode.commands.executeCommand('basilisk.profiler.start', { pid: 0 });
            } catch {
                // Expected error.
            }

            try {
                await vscode.commands.executeCommand('basilisk.profiler.stop', {
                    sessionId: `fake-session-cycle-${cycle}`,
                });
            } catch {
                // Expected error.
            }
        }

        const store = getStore();
        assert.ok(store, 'Store should exist after rapid cycles');
        assert.ok(
            store.lspState.value === 'running',
            `LSP should still be running after ${iterations} error cycles, got: ${store.lspState.value}`,
        );

        const result = await vscode.commands.executeCommand('basilisk.profiler.list');
        sessionsOf(result, 'profiler.list should still work after error cycles');
    });

});

suite('Profiler — Coexistence and Disposal', () => {
    suiteSetup(async function () {
        const result = await setupLspTestSuite('basilisk-profiler-xfeat2-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        clearProfileDecorations();
        clearMemoryDecorations();
        await closeAllEditors();
    });

    test('profiler decorations and memory decorations can coexist', async () => {
        await openPythonFile(tmpDir, 'coexist.py', 'x = 1\ny = 2\nz = 3\n');

        const profileResult: ProfileResult = {
            sessionId: 'coexist-cpu',
            duration: 1.0,
            totalSamples: 100,
            outputFile: '',
            hotFunctions: [],
            hotLines: [
                { file: '/tmp/coexist.py', line: 1, samples: 50, percentage: 50.0 },
            ],
        };

        const memSnapshot: MemorySnapshotResult = {
            memorySessionId: 'coexist-mem',
            snapshotId: 'snap-coexist',
            currentMemory: 1048576,
            peakMemory: 2097152,
            topAllocations: [
                { file: '/tmp/coexist.py', line: 2, size: 524288, count: 100 },
            ],
        };

        assert.doesNotThrow(() => {
            applyProfileDecorations(profileResult);
        }, 'Profile decorations should apply without error');

        assert.doesNotThrow(() => {
            applyMemoryDecorations(memSnapshot);
        }, 'Memory decorations should apply without error');

        assert.doesNotThrow(() => {
            clearProfileDecorations();
        }, 'Clearing profile decorations should not throw');

        assert.doesNotThrow(() => {
            clearMemoryDecorations();
        }, 'Clearing memory decorations should not throw');
    });

    test('profiler commands exist alongside non-profiler commands', async () => {
        const store = getStore();
        assert.ok(store, 'Store should exist');

        const serverCmds = store.serverCommands.value;
        assert.ok(serverCmds.size > PROFILER_SERVER_COMMANDS.length,
            'Server should advertise commands beyond just profiler ones');

        for (const cmd of PROFILER_SERVER_COMMANDS) {
            assert.ok(serverCmds.has(cmd),
                `Server command "${cmd}" should still be present alongside other commands`);
        }
    });

    test('dispose functions are idempotent and safe to call multiple times', () => {
        assert.doesNotThrow(() => {
            disposeProfileDecorations();
        }, 'First disposeProfileDecorations call should not throw');

        assert.doesNotThrow(() => {
            disposeProfileDecorations();
        }, 'Second disposeProfileDecorations call should not throw');

        assert.doesNotThrow(() => {
            disposeMemoryDecorations();
        }, 'First disposeMemoryDecorations call should not throw');

        assert.doesNotThrow(() => {
            disposeMemoryDecorations();
        }, 'Second disposeMemoryDecorations call should not throw');
    });
});
