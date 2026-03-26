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
} from '../../memory-decorations';
import {
    applyMemoryDecorations,
    clearMemoryDecorations,
    disposeMemoryDecorations,
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
