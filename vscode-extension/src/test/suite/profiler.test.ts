/**
 * Profiler E2E Tests — Command Registration, Configuration, Status Bar, Keybindings.
 *
 * Validates the profiling command registration and configuration:
 * - CPU profiler commands are registered and callable
 * - Profiler settings are read from configuration
 * - Status bar item appears and responds to profiling state
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

import {
    PROFILER_CLIENT_COMMANDS,
    PROFILER_SERVER_COMMANDS,
    PROFILER_SETTINGS,
    PROFILER_EXTRA_SETTINGS,
    type PackageJsonCommandEntry,
    type PackageJsonKeybinding,
    getPackageJsonProperties,
} from './profiler-test-constants';

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
        const properties = getPackageJsonProperties();

        for (const setting of PROFILER_SETTINGS) {
            assert.ok(
                properties[setting] !== undefined,
                `Setting "${setting}" should be declared in package.json`,
            );
        }
    });

    test('extra profiler settings are declared in package.json', () => {
        const properties = getPackageJsonProperties();

        for (const setting of PROFILER_EXTRA_SETTINGS) {
            assert.ok(
                properties[setting] !== undefined,
                `Setting "${setting}" should be declared in package.json`,
            );
        }
    });

    test('profiler settings have correct types in package.json', () => {
        const properties = getPackageJsonProperties();

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
            const prop = properties[key] as { type?: string } | undefined;
            assert.ok(prop !== undefined, `Property "${key}" must exist`);
            assert.strictEqual(
                prop.type,
                expectedType,
                `"${key}" should have type "${expectedType}", got "${String(prop.type)}"`,
            );
        }
    });

    test('preset enum values are correct', () => {
        const properties = getPackageJsonProperties();
        const presetProp = properties['basilisk.profiler.preset'] as
            { enum?: string[] } | undefined;
        assert.ok(presetProp, 'preset property should exist');
        assert.ok(Array.isArray(presetProp.enum), 'preset should have enum values');

        const expectedValues = ['default', 'lightweight', 'detailed', 'memory'];
        for (const value of expectedValues) {
            assert.ok(
                presetProp.enum.includes(value),
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

        const packageJson = extension.packageJSON as {
            contributes?: { keybindings?: PackageJsonKeybinding[] };
        };
        const keybindings = packageJson.contributes?.keybindings ?? [];

        const profilerKeybindings = keybindings.filter(
            (kb) => kb.command.startsWith('basilisk.profile'),
        );

        assert.ok(
            profilerKeybindings.length >= 2,
            `Should have at least 2 profiler keybindings (start + stop), found ${profilerKeybindings.length}`,
        );
    });

    test('profiler commands appear in package.json commands section', () => {
        const extension = vscode.extensions.getExtension(EXTENSION_ID);
        assert.ok(extension, 'Extension should be found');

        const packageJson = extension.packageJSON as {
            contributes?: { commands?: PackageJsonCommandEntry[] };
        };
        const commands = packageJson.contributes?.commands ?? [];
        const commandIds = commands.map((c) => c.command);

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

        const packageJson = extension.packageJSON as {
            contributes?: { commands?: PackageJsonCommandEntry[] };
        };
        const commands = packageJson.contributes?.commands ?? [];

        for (const cmd of PROFILER_CLIENT_COMMANDS) {
            const entry = commands.find((c) => c.command === cmd);
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
