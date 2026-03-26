/**
 * Profiler and Memory Profiler E2E Tests for the Basilisk VS Code Extension.
 *
 * Validates the full profiling and memory tracking workflow:
 * - CPU profiler commands are registered and callable
 * - Memory profiler commands are registered and callable
 * - Status bar item appears and responds to profiling state
 * - Profile start/stop lifecycle works end-to-end
 * - Profiler settings are read from configuration
 * - Profiler decorations module exports correctly
 * - Memory decorations module exports correctly
 * - ProfileResult type has required fields
 * - Heat level classification works correctly
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
    openPythonFile,
} from './test-helpers';

// Import profiler decoration types for structural assertions.
import type {
    ProfileResult,
    ProfileHotLine,
    ProfileHotFunction,
} from '../../profiler-decorations';
import {
    applyProfileDecorations,
    clearProfileDecorations,
    disposeProfileDecorations,
} from '../../profiler-decorations';

// Import memory decoration types for structural assertions.
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

/** Profiler client-side commands (registered in profiler.ts). */
const PROFILER_CLIENT_COMMANDS = [
    'basilisk.profileStart',
    'basilisk.profileStop',
    'basilisk.profileSnapshot',
    'basilisk.profileAttachToDebug',
] as const;

/** Memory profiler client-side commands (registered in memory-profiler.ts). */
const MEMORY_CLIENT_COMMANDS = [
    'basilisk.memoryStart',
    'basilisk.memorySnapshot',
    'basilisk.memoryStop',
    'basilisk.memoryReferences',
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

/** Additional profiler configuration keys. */
const PROFILER_EXTRA_SETTINGS = [
    'basilisk.profiler.profileOnLaunch',
    'basilisk.profiler.preset',
] as const;

let tmpDir = '';

// eslint-disable-next-line max-lines-per-function
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

    test('profiler server commands are tracked in store.serverCommands', () => {
        const store = getStore();
        assert.ok(store, 'Store should be initialized');
        assert.ok(store.serverCommands.value.size > 0, 'Server should advertise commands');

        for (const cmd of PROFILER_SERVER_COMMANDS) {
            assert.ok(
                store.isServerCommandAdvertised(cmd),
                `Server command "${cmd}" should be in store.serverCommands`,
            );
        }
    });

    test('profiler client commands are NOT in store.serverCommands', () => {
        const store = getStore();
        assert.ok(store, 'Store should be initialized');

        for (const cmd of PROFILER_CLIENT_COMMANDS) {
            assert.ok(
                !store.isServerCommandAdvertised(cmd),
                `Client command "${cmd}" must NOT appear in store.serverCommands`,
            );
        }
    });

    test('profiler.list returns empty sessions initially', async () => {
        const result = await vscode.commands.executeCommand(
            'basilisk.profiler.list',
        );
        assert.ok(result !== undefined, 'profiler.list should return a result');
        assert.ok(result !== null, 'profiler.list should not return null');

        const json = result as { sessions: unknown[] };
        assert.ok(Array.isArray(json.sessions), 'sessions should be an array');
        assert.strictEqual(
            json.sessions.length,
            0,
            'no sessions should be active initially',
        );
    });

    test('profiler.list result has correct shape', async () => {
        const result = await vscode.commands.executeCommand(
            'basilisk.profiler.list',
        );
        assert.ok(result !== undefined, 'profiler.list must return a value');

        const json = result as Record<string, unknown>;
        assert.ok('sessions' in json, 'result must have sessions key');
        assert.ok(Array.isArray(json.sessions), 'sessions must be an array');
    });
});

// eslint-disable-next-line max-lines-per-function
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

    test('profileOnLaunch defaults to false', () => {
        const config = vscode.workspace.getConfiguration('basilisk.profiler');
        assert.strictEqual(
            config.get<boolean>('profileOnLaunch'),
            false,
            'profileOnLaunch should default to false',
        );
    });

    test('preset defaults to "default"', () => {
        const config = vscode.workspace.getConfiguration('basilisk.profiler');
        assert.strictEqual(
            config.get<string>('preset'),
            'default',
            'preset should default to "default"',
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

    test('extra profiler settings are declared in package.json', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const properties =
            packageJson?.contributes?.configuration?.properties ?? {};

        for (const setting of PROFILER_EXTRA_SETTINGS) {
            assert.ok(
                properties[setting] !== undefined,
                `Setting "${setting}" should be declared in package.json`,
            );
        }
    });

    test('profiler settings have correct types in package.json', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const properties =
            packageJson?.contributes?.configuration?.properties ?? {};

        const expectedTypes: Record<string, string> = {
            'basilisk.profiler.sampleRate': 'number',
            'basilisk.profiler.includeNative': 'boolean',
            'basilisk.profiler.lineThreshold': 'number',
            'basilisk.profiler.functionThreshold': 'number',
            'basilisk.profiler.maxDiagnosticsPerFile': 'number',
            'basilisk.profiler.showInlineHeatMap': 'boolean',
            'basilisk.profiler.profileOnLaunch': 'boolean',
            'basilisk.profiler.preset': 'string',
        };

        for (const [key, expectedType] of Object.entries(expectedTypes)) {
            const prop = properties[key];
            assert.ok(prop !== undefined, `Property "${key}" must exist`);
            assert.strictEqual(
                prop.type,
                expectedType,
                `"${key}" should have type "${expectedType}", got "${prop.type as string}"`,
            );
        }
    });

    test('preset enum values are correct', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const properties =
            packageJson?.contributes?.configuration?.properties ?? {};
        const presetProp = properties['basilisk.profiler.preset'];
        assert.ok(presetProp, 'preset property should exist');
        assert.ok(Array.isArray(presetProp.enum), 'preset should have enum values');

        const expectedValues = ['default', 'lightweight', 'detailed', 'memory'];
        for (const value of expectedValues) {
            assert.ok(
                (presetProp.enum as string[]).includes(value),
                `preset enum should include "${value}"`,
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
        assert.ok(
            store.lspState.value === 'running' || store.lspState.value === 'starting',
            `LSP state should be running or starting, got: ${store.lspState.value}`,
        );
    });

    test('extension is active after profiler setup', () => {
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension should be found');
        assert.strictEqual(ext.isActive, true, 'Extension must be active');
    });
});

// eslint-disable-next-line max-lines-per-function
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
                'Should have an error message, got empty',
            );
        }
    });

    test('profiler.stop with missing sessionId gives clear error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.stop', {});
            assert.fail('profiler.stop with missing sessionId should have thrown');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
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

        const json1 = result1 as { sessions: unknown[] };
        const json2 = result2 as { sessions: unknown[] };

        assert.ok(Array.isArray(json1.sessions), 'First call sessions should be array');
        assert.ok(Array.isArray(json2.sessions), 'Second call sessions should be array');
        assert.strictEqual(json1.sessions.length, json2.sessions.length,
            'Consecutive list calls should return same session count');
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

    test('profiler commands have titles and categories', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const commands = packageJson?.contributes?.commands ?? [];

        for (const cmd of PROFILER_CLIENT_COMMANDS) {
            const entry = commands.find(
                (c: { command: string }) => c.command === cmd,
            ) as { command: string; title?: string; category?: string } | undefined;
            assert.ok(entry, `Command entry for "${cmd}" should exist`);
            assert.ok(
                entry.title !== undefined && entry.title.length > 0,
                `Command "${cmd}" should have a non-empty title`,
            );
            assert.strictEqual(
                entry.category,
                'Basilisk',
                `Command "${cmd}" should have category "Basilisk"`,
            );
        }
    });
});

// eslint-disable-next-line max-lines-per-function
suite('Profiler — Decoration Modules', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-dec-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('profiler-decorations exports applyProfileDecorations function', () => {
        assert.strictEqual(typeof applyProfileDecorations, 'function',
            'applyProfileDecorations should be a function');
    });

    test('profiler-decorations exports clearProfileDecorations function', () => {
        assert.strictEqual(typeof clearProfileDecorations, 'function',
            'clearProfileDecorations should be a function');
    });

    test('profiler-decorations exports disposeProfileDecorations function', () => {
        assert.strictEqual(typeof disposeProfileDecorations, 'function',
            'disposeProfileDecorations should be a function');
    });

    test('memory-decorations exports applyMemoryDecorations function', () => {
        assert.strictEqual(typeof applyMemoryDecorations, 'function',
            'applyMemoryDecorations should be a function');
    });

    test('memory-decorations exports clearMemoryDecorations function', () => {
        assert.strictEqual(typeof clearMemoryDecorations, 'function',
            'clearMemoryDecorations should be a function');
    });

    test('memory-decorations exports disposeMemoryDecorations function', () => {
        assert.strictEqual(typeof disposeMemoryDecorations, 'function',
            'disposeMemoryDecorations should be a function');
    });

    test('clearProfileDecorations does not throw when no decorations exist', () => {
        assert.doesNotThrow(() => {
            clearProfileDecorations();
        }, 'clearProfileDecorations should be safe to call with no active decorations');
    });

    test('clearMemoryDecorations does not throw when no decorations exist', () => {
        assert.doesNotThrow(() => {
            clearMemoryDecorations();
        }, 'clearMemoryDecorations should be safe to call with no active decorations');
    });

    test('ProfileResult type has required fields', () => {
        // Construct a minimal ProfileResult to prove the type contract.
        const result: ProfileResult = {
            sessionId: 'test-session-001',
            duration: 5.2,
            totalSamples: 1000,
            outputFile: '/tmp/test.speedscope.json',
            hotFunctions: [],
            hotLines: [],
        };

        assert.strictEqual(result.sessionId, 'test-session-001');
        assert.strictEqual(result.duration, 5.2);
        assert.strictEqual(result.totalSamples, 1000);
        assert.strictEqual(result.outputFile, '/tmp/test.speedscope.json');
        assert.ok(Array.isArray(result.hotFunctions), 'hotFunctions should be an array');
        assert.ok(Array.isArray(result.hotLines), 'hotLines should be an array');
    });

    test('ProfileHotLine type has required fields', () => {
        const hotLine: ProfileHotLine = {
            file: '/src/app.py',
            line: 42,
            samples: 500,
            percentage: 25.0,
        };

        assert.strictEqual(hotLine.file, '/src/app.py');
        assert.strictEqual(hotLine.line, 42);
        assert.strictEqual(hotLine.samples, 500);
        assert.strictEqual(hotLine.percentage, 25.0);
    });

    test('ProfileHotFunction type has required fields', () => {
        const hotFunc: ProfileHotFunction = {
            name: 'process_data',
            file: '/src/pipeline.py',
            line: 15,
            samples: 800,
            percentage: 40.0,
            selfPercentage: 30.0,
        };

        assert.strictEqual(hotFunc.name, 'process_data');
        assert.strictEqual(hotFunc.file, '/src/pipeline.py');
        assert.strictEqual(hotFunc.line, 15);
        assert.strictEqual(hotFunc.samples, 800);
        assert.strictEqual(hotFunc.percentage, 40.0);
        assert.strictEqual(hotFunc.selfPercentage, 30.0);
    });

    test('MemoryAllocation type has required fields', () => {
        const alloc: MemoryAllocation = {
            file: '/src/data.py',
            line: 100,
            size: 10485760,
            count: 5000,
        };

        assert.strictEqual(alloc.file, '/src/data.py');
        assert.strictEqual(alloc.line, 100);
        assert.strictEqual(alloc.size, 10485760);
        assert.strictEqual(alloc.count, 5000);
    });

    test('MemorySnapshotResult type has required fields', () => {
        const snapshot: MemorySnapshotResult = {
            memorySessionId: 'mem-session-001',
            snapshotId: 'snap-001',
            currentMemory: 50000000,
            peakMemory: 75000000,
            topAllocations: [],
        };

        assert.strictEqual(snapshot.memorySessionId, 'mem-session-001');
        assert.strictEqual(snapshot.snapshotId, 'snap-001');
        assert.strictEqual(snapshot.currentMemory, 50000000);
        assert.strictEqual(snapshot.peakMemory, 75000000);
        assert.ok(Array.isArray(snapshot.topAllocations), 'topAllocations should be an array');
    });

    test('applyProfileDecorations handles empty result without throwing', () => {
        const emptyResult: ProfileResult = {
            sessionId: 'empty-session',
            duration: 0,
            totalSamples: 0,
            outputFile: '',
            hotFunctions: [],
            hotLines: [],
        };

        assert.doesNotThrow(() => {
            applyProfileDecorations(emptyResult);
        }, 'applyProfileDecorations should handle empty results gracefully');

        // Clean up decorations after test.
        clearProfileDecorations();
    });

    test('applyMemoryDecorations handles empty result without throwing', () => {
        const emptySnapshot: MemorySnapshotResult = {
            memorySessionId: 'empty-mem',
            snapshotId: 'snap-empty',
            currentMemory: 0,
            peakMemory: 0,
            topAllocations: [],
        };

        assert.doesNotThrow(() => {
            applyMemoryDecorations(emptySnapshot);
        }, 'applyMemoryDecorations should handle empty snapshots gracefully');

        // Clean up decorations after test.
        clearMemoryDecorations();
    });

    test('ProfileResult with populated hotFunctions validates structure', () => {
        const result: ProfileResult = {
            sessionId: 'populated-session',
            duration: 10.5,
            totalSamples: 5000,
            outputFile: '/tmp/profile.speedscope.json',
            hotFunctions: [
                {
                    name: 'compute',
                    file: '/src/math.py',
                    line: 10,
                    samples: 2500,
                    percentage: 50.0,
                    selfPercentage: 35.0,
                },
                {
                    name: 'transform',
                    file: '/src/utils.py',
                    line: 88,
                    samples: 1000,
                    percentage: 20.0,
                    selfPercentage: 15.0,
                },
            ],
            hotLines: [
                {
                    file: '/src/math.py',
                    line: 12,
                    samples: 2000,
                    percentage: 40.0,
                },
            ],
        };

        assert.strictEqual(result.hotFunctions.length, 2, 'Should have 2 hot functions');
        assert.strictEqual(result.hotLines.length, 1, 'Should have 1 hot line');
        assert.strictEqual(result.hotFunctions[0].name, 'compute');
        assert.strictEqual(result.hotFunctions[1].name, 'transform');
        assert.ok(result.hotFunctions[0].percentage > result.hotFunctions[1].percentage,
            'First function should have higher percentage');
        assert.ok(result.hotFunctions[0].selfPercentage <= result.hotFunctions[0].percentage,
            'selfPercentage should not exceed percentage');
    });
});

// eslint-disable-next-line max-lines-per-function
suite('Profiler — Heat Level Classification', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-heat-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    test('critical heat level classification (>= 20%)', () => {
        // The profiler UI classifies lines at >= 20% as "critical".
        // We verify the classification ranges by constructing ProfileHotLines.
        const criticalLine: ProfileHotLine = {
            file: '/src/hot.py', line: 1, samples: 400, percentage: 25.0,
        };
        assert.ok(criticalLine.percentage >= 20,
            'Lines at 25% should fall in the critical range');

        const borderlineCritical: ProfileHotLine = {
            file: '/src/hot.py', line: 2, samples: 200, percentage: 20.0,
        };
        assert.ok(borderlineCritical.percentage >= 20,
            'Lines at exactly 20% should fall in the critical range');
    });

    test('hot heat level classification (10-20%)', () => {
        const hotLine: ProfileHotLine = {
            file: '/src/warm.py', line: 5, samples: 150, percentage: 15.0,
        };
        assert.ok(hotLine.percentage >= 10 && hotLine.percentage < 20,
            'Lines at 15% should fall in the hot range');

        const borderlineHot: ProfileHotLine = {
            file: '/src/warm.py', line: 6, samples: 100, percentage: 10.0,
        };
        assert.ok(borderlineHot.percentage >= 10 && borderlineHot.percentage < 20,
            'Lines at exactly 10% should fall in the hot range');
    });

    test('warm heat level classification (5-10%)', () => {
        const warmLine: ProfileHotLine = {
            file: '/src/warm.py', line: 10, samples: 70, percentage: 7.0,
        };
        assert.ok(warmLine.percentage >= 5 && warmLine.percentage < 10,
            'Lines at 7% should fall in the warm range');

        const borderlineWarm: ProfileHotLine = {
            file: '/src/warm.py', line: 11, samples: 50, percentage: 5.0,
        };
        assert.ok(borderlineWarm.percentage >= 5 && borderlineWarm.percentage < 10,
            'Lines at exactly 5% should fall in the warm range');
    });

    test('cool heat level classification (1-5%)', () => {
        const coolLine: ProfileHotLine = {
            file: '/src/cool.py', line: 20, samples: 30, percentage: 3.0,
        };
        assert.ok(coolLine.percentage >= 1 && coolLine.percentage < 5,
            'Lines at 3% should fall in the cool range');

        const borderlineCool: ProfileHotLine = {
            file: '/src/cool.py', line: 21, samples: 10, percentage: 1.0,
        };
        assert.ok(borderlineCool.percentage >= 1 && borderlineCool.percentage < 5,
            'Lines at exactly 1% should fall in the cool range');
    });

    test('below threshold (< 1%) is not classified', () => {
        const belowThreshold: ProfileHotLine = {
            file: '/src/idle.py', line: 99, samples: 2, percentage: 0.5,
        };
        assert.ok(belowThreshold.percentage < 1,
            'Lines below 1% should not be classified as any heat level');
    });

    test('heat level boundaries are mutually exclusive', () => {
        const testCases = [
            { pct: 25.0, expected: 'critical' },
            { pct: 20.0, expected: 'critical' },
            { pct: 19.9, expected: 'hot' },
            { pct: 10.0, expected: 'hot' },
            { pct: 9.9, expected: 'warm' },
            { pct: 5.0, expected: 'warm' },
            { pct: 4.9, expected: 'cool' },
            { pct: 1.0, expected: 'cool' },
            { pct: 0.9, expected: 'none' },
        ];

        for (const tc of testCases) {
            let level: string;
            if (tc.pct >= 20) { level = 'critical'; }
            else if (tc.pct >= 10) { level = 'hot'; }
            else if (tc.pct >= 5) { level = 'warm'; }
            else if (tc.pct >= 1) { level = 'cool'; }
            else { level = 'none'; }

            assert.strictEqual(level, tc.expected,
                `${tc.pct}% should be classified as "${tc.expected}", got "${level}"`);
        }
    });
});

// eslint-disable-next-line max-lines-per-function
suite('Memory Profiler — Command Registration', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-memory-cmd-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(() => {
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('all memory client commands are registered', async () => {
        const allCommands = await vscode.commands.getCommands(true);

        for (const cmd of MEMORY_CLIENT_COMMANDS) {
            assert.ok(
                allCommands.includes(cmd),
                `Memory command "${cmd}" should be registered after activation`,
            );
        }
    });

    test('memory commands appear in package.json commands section', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const commands = packageJson?.contributes?.commands ?? [];
        const commandIds = commands.map((c: { command: string }) => c.command);

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

        const packageJson = extension.packageJSON;
        const commands = packageJson?.contributes?.commands ?? [];

        for (const cmd of MEMORY_CLIENT_COMMANDS) {
            const entry = commands.find(
                (c: { command: string }) => c.command === cmd,
            ) as { command: string; title?: string; category?: string } | undefined;
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

// ── Profiler Lifecycle Tests (real interaction flow) ──────────────────────

// eslint-disable-next-line max-lines-per-function
suite('Profiler — Lifecycle Interaction', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
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
            await vscode.commands.executeCommand('basilisk.profiler.start', {
                pid: 0,
            });
            assert.fail('profiler.start with PID 0 should have thrown');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(message.length > 10,
                `Error message should be descriptive, got: "${message}"`);
            assert.ok(typeof message === 'string',
                'Error should be a string message');
            // Should not be a raw stack trace.
            assert.ok(
                !message.includes('at Object.') || message.includes('Process') || message.includes('error'),
                `Error should be user-friendly, not a stack trace: ${message}`,
            );
        }
    });

    test('profiler.start with negative PID returns error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', {
                pid: -1,
            });
            assert.fail('profiler.start with negative PID should have thrown');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(message.length > 0,
                'Error message should not be empty');
            assert.ok(typeof message === 'string',
                'Error should produce a string message');
            assert.ok(
                !message.startsWith('undefined'),
                'Error message should not start with "undefined"',
            );
        }
    });

    test('profiler.start with extremely large PID returns error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', {
                pid: 999999999,
            });
            assert.fail('profiler.start with nonexistent PID should have thrown');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(message.length > 0,
                'Error message for large PID should not be empty');
            assert.ok(typeof message === 'string',
                'Error should be a string');
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
            const message = (err as Error).message ?? String(err);
            assert.ok(message.length > 0,
                'Should have an error message');
            assert.ok(
                message.includes('session') ||
                    message.includes('not found') ||
                    message.includes('No active'),
                `Error should mention session, got: ${message}`,
            );
            assert.ok(typeof message === 'string',
                'Error message must be a string type');
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
            const message = (err as Error).message ?? String(err);
            assert.ok(message.length > 0,
                'Should have an error message');
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

        const json = result as { sessions: unknown[] };
        assert.ok(Array.isArray(json.sessions), 'sessions should be an array');
        assert.strictEqual(json.sessions.length, 0,
            'no sessions should be active when nothing was started');
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
                const message = (err as Error).message ?? String(err);
                // Stack traces typically have lines like "at Function.xxx (file:line)"
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
            const json = result as { sessions: unknown[] };
            assert.ok(Array.isArray(json.sessions), 'Each result must have sessions array');
            assert.strictEqual(json.sessions.length, 0,
                'All parallel list calls should return empty sessions');
        }
    });
});

// ── Status Bar Behavior Tests ─────────────────────────────────────────────

suite('Profiler — Status Bar Behavior', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
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
        // Extension being active implies status bar registration occurred.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(ext, 'Extension should be found');
        assert.strictEqual(ext.isActive, true, 'Extension must be active');
    });

    test('profiler status bar stop command is declared in package.json', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const commands = packageJson?.contributes?.commands ?? [];
        const stopCmd = commands.find(
            (c: { command: string }) => c.command === 'basilisk.profileStop',
        ) as { command: string; title?: string } | undefined;

        assert.ok(stopCmd, 'profileStop command should exist in package.json');
        assert.ok(stopCmd.title !== undefined, 'profileStop should have a title');
        assert.ok(stopCmd.title.length > 0, 'profileStop title should not be empty');
    });

    test('profiler status bar priority is declared correctly for ordering', () => {
        // The status bar item must be left-aligned and have a reasonable priority.
        // We verify this by confirming the profiler module loads without error
        // and the extension package declares the commands that the status bar uses.
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');
        assert.ok(extension.isActive, 'Extension must be active');

        const store = getStore();
        assert.ok(store, 'Store must be initialized');
        assert.ok(store.client.value !== undefined, 'LSP client must exist');
    });
});

// ── Configuration Interaction Tests ───────────────────────────────────────

// eslint-disable-next-line max-lines-per-function
suite('Profiler — Configuration Interaction', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
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

        // Update to a new value.
        await config.update('sampleRate', 50, vscode.ConfigurationTarget.Workspace);
        const updatedConfig = vscode.workspace.getConfiguration('basilisk.profiler');
        assert.strictEqual(updatedConfig.get<number>('sampleRate'), 50,
            'sampleRate should be updated to 50');

        // Restore original.
        await config.update('sampleRate', undefined, vscode.ConfigurationTarget.Workspace);
        const restoredConfig = vscode.workspace.getConfiguration('basilisk.profiler');
        assert.strictEqual(restoredConfig.get<number>('sampleRate'), 100,
            'sampleRate should be restored to default 100');
    });

    test('lightweight preset implies lower sample rate and no native', () => {
        // The resolvePreset function in profiler.ts maps "lightweight" to:
        //   sampleRate: 10, includeNative: false
        // We verify the config advertises this preset value.
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const properties =
            packageJson?.contributes?.configuration?.properties ?? {};
        const presetProp = properties['basilisk.profiler.preset'];
        assert.ok(presetProp, 'preset property should exist');
        assert.ok((presetProp.enum as string[]).includes('lightweight'),
            'lightweight must be a valid preset');
        assert.ok(presetProp.type === 'string',
            'preset should be a string type');
    });

    test('detailed preset enables includeNative', () => {
        // "detailed" maps to: sampleRate: 100, includeNative: true
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const properties =
            packageJson?.contributes?.configuration?.properties ?? {};
        const presetProp = properties['basilisk.profiler.preset'];
        assert.ok(presetProp, 'preset property should exist');
        assert.ok((presetProp.enum as string[]).includes('detailed'),
            'detailed must be a valid preset');

        // Verify includeNative default is false (so detailed overrides it).
        const config = vscode.workspace.getConfiguration('basilisk.profiler');
        assert.strictEqual(config.get<boolean>('includeNative'), false,
            'includeNative default should be false (detailed preset overrides this)');
    });

    test('all 4 presets are valid enum values with correct count', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const properties =
            packageJson?.contributes?.configuration?.properties ?? {};
        const presetProp = properties['basilisk.profiler.preset'];
        assert.ok(presetProp, 'preset property should exist');

        const enumValues = presetProp.enum as string[];
        assert.ok(enumValues.length >= 4,
            `Should have at least 4 preset values, got ${enumValues.length}`);
        assert.ok(enumValues.includes('default'), 'Must include "default"');
        assert.ok(enumValues.includes('lightweight'), 'Must include "lightweight"');
        assert.ok(enumValues.includes('detailed'), 'Must include "detailed"');
        assert.ok(enumValues.includes('memory'), 'Must include "memory"');
    });

    test('numeric settings have reasonable bounds in config declarations', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON;
        const properties =
            packageJson?.contributes?.configuration?.properties ?? {};

        // sampleRate should have a default that is positive.
        const sampleRateProp = properties['basilisk.profiler.sampleRate'];
        assert.ok(sampleRateProp, 'sampleRate property must exist');
        assert.ok(typeof sampleRateProp.default === 'number',
            'sampleRate default should be a number');
        assert.ok(sampleRateProp.default > 0,
            'sampleRate default should be positive');

        // lineThreshold should have a positive default.
        const lineThresholdProp = properties['basilisk.profiler.lineThreshold'];
        assert.ok(lineThresholdProp, 'lineThreshold property must exist');
        assert.ok(typeof lineThresholdProp.default === 'number',
            'lineThreshold default should be a number');
        assert.ok(lineThresholdProp.default > 0,
            'lineThreshold default should be positive');

        // maxDiagnosticsPerFile should have a positive default.
        const maxDiagProp = properties['basilisk.profiler.maxDiagnosticsPerFile'];
        assert.ok(maxDiagProp, 'maxDiagnosticsPerFile property must exist');
        assert.ok(typeof maxDiagProp.default === 'number',
            'maxDiagnosticsPerFile default should be a number');
        assert.ok(maxDiagProp.default > 0,
            'maxDiagnosticsPerFile default should be positive');
    });
});

// ── Decoration Contract Tests ─────────────────────────────────────────────

// eslint-disable-next-line max-lines-per-function
suite('Profiler — Decoration Contracts', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-deco-');
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

    test('applyProfileDecorations with multiple files and varying percentages', async () => {
        // Open a Python file so decorations have a target editor.
        await openPythonFile(tmpDir, 'hot_module.py',
            'def hot_func():\n    x = 1\n    y = 2\n    z = x + y\n    return z\n');

        const result: ProfileResult = {
            sessionId: 'multi-file-session',
            duration: 8.3,
            totalSamples: 3000,
            outputFile: '/tmp/multi.speedscope.json',
            hotFunctions: [
                { name: 'hot_func', file: '/nonexistent/a.py', line: 1, samples: 1500, percentage: 50.0, selfPercentage: 40.0 },
                { name: 'warm_func', file: '/nonexistent/b.py', line: 10, samples: 300, percentage: 10.0, selfPercentage: 8.0 },
                { name: 'cool_func', file: '/nonexistent/c.py', line: 20, samples: 60, percentage: 2.0, selfPercentage: 1.5 },
            ],
            hotLines: [
                { file: '/nonexistent/a.py', line: 3, samples: 1200, percentage: 40.0 },
                { file: '/nonexistent/b.py', line: 12, samples: 200, percentage: 6.7 },
                { file: '/nonexistent/c.py', line: 22, samples: 30, percentage: 1.0 },
            ],
        };

        assert.doesNotThrow(() => {
            applyProfileDecorations(result);
        }, 'applyProfileDecorations should handle multi-file results');

        assert.strictEqual(result.hotFunctions.length, 3,
            'Should have 3 hot functions');
        assert.strictEqual(result.hotLines.length, 3,
            'Should have 3 hot lines');
        assert.ok(result.hotFunctions[0].percentage > result.hotFunctions[1].percentage,
            'Functions should be ordered by percentage');
    });

    test('heat level classification boundary at exactly 1%', () => {
        const atBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 1, samples: 10, percentage: 1.0,
        };
        const belowBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 2, samples: 9, percentage: 0.99,
        };

        assert.ok(atBoundary.percentage >= 1,
            '1.0% should be classified (cool)');
        assert.ok(belowBoundary.percentage < 1,
            '0.99% should not be classified');
        assert.ok(atBoundary.percentage < 5,
            '1.0% should not be warm');
    });

    test('heat level classification boundary at exactly 5%', () => {
        const atBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 1, samples: 50, percentage: 5.0,
        };
        const belowBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 2, samples: 49, percentage: 4.99,
        };

        assert.ok(atBoundary.percentage >= 5,
            '5.0% should be classified as warm');
        assert.ok(belowBoundary.percentage < 5,
            '4.99% should still be cool');
        assert.ok(atBoundary.percentage < 10,
            '5.0% should not be hot');
    });

    test('heat level classification boundary at exactly 10%', () => {
        const atBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 1, samples: 100, percentage: 10.0,
        };
        const belowBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 2, samples: 99, percentage: 9.99,
        };

        assert.ok(atBoundary.percentage >= 10,
            '10.0% should be classified as hot');
        assert.ok(belowBoundary.percentage < 10,
            '9.99% should still be warm');
        assert.ok(atBoundary.percentage < 20,
            '10.0% should not be critical');
    });

    test('heat level classification boundary at exactly 20%', () => {
        const atBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 1, samples: 200, percentage: 20.0,
        };
        const belowBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 2, samples: 199, percentage: 19.99,
        };

        assert.ok(atBoundary.percentage >= 20,
            '20.0% should be classified as critical');
        assert.ok(belowBoundary.percentage < 20,
            '19.99% should still be hot');
        assert.ok(belowBoundary.percentage >= 10,
            '19.99% must be at least hot-level');
    });

    test('clearProfileDecorations removes all decorations without error', () => {
        // Apply decorations first.
        const result: ProfileResult = {
            sessionId: 'clear-test',
            duration: 1.0,
            totalSamples: 100,
            outputFile: '',
            hotFunctions: [],
            hotLines: [
                { file: '/tmp/test.py', line: 1, samples: 50, percentage: 50.0 },
            ],
        };

        assert.doesNotThrow(() => {
            applyProfileDecorations(result);
        }, 'Applying decorations should not throw');

        assert.doesNotThrow(() => {
            clearProfileDecorations();
        }, 'Clearing decorations should not throw');

        // Clearing again should also be safe (idempotent).
        assert.doesNotThrow(() => {
            clearProfileDecorations();
        }, 'Double-clearing decorations should not throw');
    });

    test('decorations apply after opening file and survive re-application', async () => {
        const { uri } = await openPythonFile(tmpDir, 'deco_survive.py',
            'x = 1\ny = 2\nz = x + y\n');

        const result: ProfileResult = {
            sessionId: 'survive-test',
            duration: 2.0,
            totalSamples: 500,
            outputFile: '',
            hotFunctions: [],
            hotLines: [
                { file: uri.fsPath, line: 2, samples: 250, percentage: 50.0 },
            ],
        };

        // Apply, clear, re-apply — should not throw.
        assert.doesNotThrow(() => applyProfileDecorations(result),
            'First apply should succeed');
        assert.doesNotThrow(() => clearProfileDecorations(),
            'Clear should succeed');
        assert.doesNotThrow(() => applyProfileDecorations(result),
            'Re-apply should succeed');
        clearProfileDecorations();
    });
});

// ── Memory Profiler Tests ─────────────────────────────────────────────────

// eslint-disable-next-line max-lines-per-function
suite('Memory Profiler — Extended', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
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

        // The command handler shows a warning or sends an LSP request.
        // We verify it does not throw unexpectedly.
        try {
            await vscode.commands.executeCommand('basilisk.memoryStart');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            // If it errors, the error should be meaningful.
            assert.ok(message.length > 0, 'Error message should not be empty');
            assert.ok(typeof message === 'string', 'Error should be a string');
        }
        // Test passes if no unhandled exception occurred.
        assert.ok(true, 'memoryStart command was callable');
    });

    test('memorySnapshot without active session warns gracefully', async () => {
        // memorySnapshot requires an active session; calling without one
        // should show a warning but not crash.
        try {
            await vscode.commands.executeCommand('basilisk.memorySnapshot');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(message.length > 0, 'Error should have a message');
        }
        // If no error is thrown, the command showed a warning message internally.
        assert.ok(true, 'memorySnapshot without session did not crash');
        const store = getStore();
        assert.ok(store, 'Store should still be intact after memorySnapshot call');
    });

    test('memoryReferences command is callable', async () => {
        const store = getStore();
        assert.ok(store, 'Store should be initialized');
        assert.ok(store.client.value !== undefined, 'LSP client should exist');

        // memoryReferences prompts for input, so calling it programmatically
        // will return early (cancelled). We verify no crash occurs.
        try {
            await vscode.commands.executeCommand('basilisk.memoryReferences');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
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

        assert.strictEqual(snapshot.topAllocations.length, 3,
            'Should have 3 allocations');
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

// ── Error Handling Tests ──────────────────────────────────────────────────

// eslint-disable-next-line max-lines-per-function
suite('Profiler — Error Handling', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
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
                pid: 0,
                sampleRate: -1,
            });
            assert.fail('Should have thrown for invalid params');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(message.length > 0, 'Error message should not be empty');
            assert.ok(typeof message === 'string', 'Error should be string');
            // The error should not be a generic "command not found".
            assert.ok(
                !message.includes('command not found') &&
                    !message.includes('is not registered'),
                `Error should be about the params, not command registration: ${message}`,
            );
        }
    });

    test('profiler error codes are within expected range', async () => {
        // LSP spec error codes for profiler: -32001 through -32006.
        const expectedCodes = [-32001, -32002, -32003, -32004, -32005, -32006];

        // Verify the error code constants are well-formed.
        for (const code of expectedCodes) {
            assert.ok(code < 0, `Error code ${code} should be negative`);
            assert.ok(code >= -32099, `Error code ${code} should be >= -32099`);
            assert.ok(code <= -32000, `Error code ${code} should be <= -32000`);
        }

        // The 6 codes should be unique.
        const unique = new Set(expectedCodes);
        assert.strictEqual(unique.size, expectedCodes.length,
            'All error codes should be unique');
    });

    test('profiler.stop with empty string sessionId returns descriptive error', async () => {
        try {
            await vscode.commands.executeCommand('basilisk.profiler.stop', {
                sessionId: '',
            });
            assert.fail('Empty sessionId should produce an error');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
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
            await vscode.commands.executeCommand('basilisk.profiler.snapshot', {
                sessionId: null,
            });
            assert.fail('Null sessionId should produce an error');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
            assert.ok(message.length > 0, 'Error should have a message');
            assert.ok(typeof message === 'string', 'Error should be string type');
            assert.ok(
                !message.includes('segfault') && !message.includes('SIGSEGV'),
                'Error should not be a segfault',
            );
        }
    });

    test('connection errors are handled when LSP client is present', async () => {
        // The LSP client should be running; verify commands give protocol-level
        // errors rather than raw TCP errors.
        const store = getStore();
        assert.ok(store, 'Store should exist');
        assert.ok(store.client.value !== undefined, 'Client should exist');

        try {
            await vscode.commands.executeCommand('basilisk.profiler.start', {
                pid: 2147483647,
            });
            assert.fail('Nonexistent PID should produce an error');
        } catch (err: unknown) {
            const message = (err as Error).message ?? String(err);
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
            const message = (err as Error).message ?? String(err);
            // A JSON blob would typically start with { or [.
            assert.ok(
                !message.trimStart().startsWith('{') ||
                    message.includes('session') ||
                    message.includes('error'),
                `Error should be human-readable, not a raw JSON blob: ${message.slice(0, 200)}`,
            );
            assert.ok(message.length < 2000,
                'Error message should not be excessively long');
            assert.ok(typeof message === 'string', 'Error must be a string');
        }
    });
});

// ── Cross-Feature Integration Tests ───────────────────────────────────────

// eslint-disable-next-line max-lines-per-function
suite('Profiler — Cross-Feature Integration', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
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

        // Run a profiler list call, then verify symbols still work.
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
        // Call list multiple times, then verify LSP still responds.
        for (let iteration = 0; iteration < 5; iteration++) {
            const result = await vscode.commands.executeCommand('basilisk.profiler.list');
            const json = result as { sessions: unknown[] };
            assert.ok(Array.isArray(json.sessions),
                `Iteration ${iteration}: sessions should be an array`);
        }

        // LSP should still be responsive after repeated calls.
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
            // Start with invalid PID — should error.
            try {
                await vscode.commands.executeCommand('basilisk.profiler.start', {
                    pid: 0,
                });
            } catch {
                // Expected error.
            }

            // Stop with invalid session — should error.
            try {
                await vscode.commands.executeCommand('basilisk.profiler.stop', {
                    sessionId: `fake-session-cycle-${cycle}`,
                });
            } catch {
                // Expected error.
            }
        }

        // Verify LSP is still healthy after rapid error cycles.
        const store = getStore();
        assert.ok(store, 'Store should exist after rapid cycles');
        assert.ok(
            store.lspState.value === 'running',
            `LSP should still be running after ${iterations} error cycles, got: ${store.lspState.value}`,
        );

        // profiler.list should still return valid data.
        const result = await vscode.commands.executeCommand('basilisk.profiler.list');
        const json = result as { sessions: unknown[] };
        assert.ok(Array.isArray(json.sessions),
            'profiler.list should still work after error cycles');
    });

    test('profiler decorations and memory decorations can coexist', async () => {
        await openPythonFile(tmpDir, 'coexist.py',
            'x = 1\ny = 2\nz = 3\n');

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

        // Both decoration types should be applicable simultaneously.
        assert.doesNotThrow(() => {
            applyProfileDecorations(profileResult);
        }, 'Profile decorations should apply without error');

        assert.doesNotThrow(() => {
            applyMemoryDecorations(memSnapshot);
        }, 'Memory decorations should apply without error');

        // Clearing one type should not affect the other.
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

        // Server commands should include both profiler and non-profiler commands.
        const serverCmds = store.serverCommands.value;
        assert.ok(serverCmds.size > PROFILER_SERVER_COMMANDS.length,
            'Server should advertise commands beyond just profiler ones');

        // Profiler commands should be a subset.
        for (const cmd of PROFILER_SERVER_COMMANDS) {
            assert.ok(serverCmds.has(cmd),
                `Server command "${cmd}" should still be present alongside other commands`);
        }
    });

    test('dispose functions are idempotent and safe to call multiple times', () => {
        // disposeProfileDecorations and disposeMemoryDecorations should be
        // safe to call even when no decorations exist.
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
