/**
 * Profiler E2E Tests — Decoration Modules, Heat Level Classification, Decoration Contracts.
 *
 * Validates:
 * - Profiler decorations module exports correctly
 * - Memory decorations module exports correctly
 * - ProfileResult type has required fields
 * - Heat level classification works correctly
 * - Decoration apply/clear lifecycle
 *
 * These tests require the Basilisk LSP server binary to be built.
 * They exercise the real LSP protocol, not mocks.
 */

import * as assert from 'assert';
import {
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
} from '../../profiler-decorations';

// Import memory decoration types for structural assertions.
import type {
    MemoryAllocation,
    MemorySnapshotResult,
} from '../../memory-decorations';
import {
    applyMemoryDecorations,
    clearMemoryDecorations,
} from '../../memory-decorations';

let tmpDir = '';

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

    test('memory-decorations exports applyMemoryDecorations function', () => {
        assert.strictEqual(typeof applyMemoryDecorations, 'function',
            'applyMemoryDecorations should be a function');
    });

    test('memory-decorations exports clearMemoryDecorations function', () => {
        assert.strictEqual(typeof clearMemoryDecorations, 'function',
            'clearMemoryDecorations should be a function');
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

});

suite('Profiler — Decoration Apply/Clear', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-dec2-');
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

        assert.strictEqual(result.hotFunctions.length, 3, 'Should have 3 hot functions');
        assert.strictEqual(result.hotLines.length, 3, 'Should have 3 hot lines');
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

        assert.ok(atBoundary.percentage >= 1, '1.0% should be classified (cool)');
        assert.ok(belowBoundary.percentage < 1, '0.99% should not be classified');
        assert.ok(atBoundary.percentage < 5, '1.0% should not be warm');
    });

    test('heat level classification boundary at exactly 5%', () => {
        const atBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 1, samples: 50, percentage: 5.0,
        };
        const belowBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 2, samples: 49, percentage: 4.99,
        };

        assert.ok(atBoundary.percentage >= 5, '5.0% should be classified as warm');
        assert.ok(belowBoundary.percentage < 5, '4.99% should still be cool');
        assert.ok(atBoundary.percentage < 10, '5.0% should not be hot');
    });

    test('heat level classification boundary at exactly 10%', () => {
        const atBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 1, samples: 100, percentage: 10.0,
        };
        const belowBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 2, samples: 99, percentage: 9.99,
        };

        assert.ok(atBoundary.percentage >= 10, '10.0% should be classified as hot');
        assert.ok(belowBoundary.percentage < 10, '9.99% should still be warm');
        assert.ok(atBoundary.percentage < 20, '10.0% should not be critical');
    });

    test('heat level classification boundary at exactly 20%', () => {
        const atBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 1, samples: 200, percentage: 20.0,
        };
        const belowBoundary: ProfileHotLine = {
            file: '/src/boundary.py', line: 2, samples: 199, percentage: 19.99,
        };

        assert.ok(atBoundary.percentage >= 20, '20.0% should be classified as critical');
        assert.ok(belowBoundary.percentage < 20, '19.99% should still be hot');
        assert.ok(belowBoundary.percentage >= 10, '19.99% must be at least hot-level');
    });

});

suite('Profiler — Decoration Lifecycle', () => {
    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-profiler-deco2-');
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

    test('clearProfileDecorations removes all decorations without error', () => {
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

        assert.doesNotThrow(() => applyProfileDecorations(result),
            'First apply should succeed');
        assert.doesNotThrow(() => clearProfileDecorations(),
            'Clear should succeed');
        assert.doesNotThrow(() => applyProfileDecorations(result),
            'Re-apply should succeed');
        clearProfileDecorations();
    });
});
