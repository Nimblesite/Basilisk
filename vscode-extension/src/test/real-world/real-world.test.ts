// Tests for [VSIX-REALWORLD]. See docs/specs/VSIX-REAL-WORLD-SPEC.md#VSIX-REALWORLD
/**
 * Real-world workspace e2e suite: opens a PINNED popular Python repository
 * (flask / rich / fastapi — see test-fixtures/real-world-corpus.json) as the
 * VS Code workspace and drives the extension the way a user does — waiting
 * for whole-workspace analysis, hammering hovers, definitions, completions,
 * references, workspace search, and edit churn — while the basilisk server
 * process is measured from the OS and HELD to hard memory + CPU budgets
 * ([VSIX-REALWORLD-RESOURCES]).
 *
 * One corpus repo per test process: `.vscode-test.mjs` builds a config per
 * repo, sets BSK_REAL_WORLD_REPO, and opens the fetched tree as the
 * workspace folder ([VSIX-REALWORLD-WIRING]).
 */

import * as fs from 'fs';
import * as path from 'path';
import {
    DIAGNOSTIC_TIMEOUT_MS,
    SUITE_SETUP_TIMEOUT_MS,
    closeAllEditors,
    waitForLspReady,
} from '../suite/test-helpers';
import { type FileJourney, activeRepoSpec } from './corpus';
import {
    CHURN_DIAGNOSTIC_TIMEOUT_MS,
    assertDiagnosticInvariants,
    assertionTotal,
    check,
    findServerPid,
    runEditChurn,
    runFileJourney,
    runOpenBlitz,
    runWorkspaceSymbolProbes,
    verifyPinnedWorkspace,
    waitForWorkspaceAnalysis,
    workspaceDiagnostics,
} from './journey';
import { type ProcessSample, ResourceMonitor } from './metrics';

/** Margin added to analysis-scale timeouts for editor/session overhead. */
const TIMEOUT_MARGIN_MS = 60_000;

/**
 * Per-repo floor on counted assertions — the density ratchet. Measured runs
 * count 7.5k (rich) to 28k (fastapi); the floor holds a wide margin under
 * the weakest repo and only ratchets UP.
 */
const MIN_ASSERTIONS_PER_REPO = 2_000;

/**
 * Mocha budget for one file journey: every probe is allowed one full poll
 * deadline, so the test timeout must cover the worst case the journey's
 * own deadlines sanction — a slow-but-in-budget run must fail on the
 * journey's descriptive assertion, never an opaque mocha timeout.
 */
function journeyTimeoutMs(file: FileJourney): number {
    const probes = 1 + file.hovers.length + file.definitions.length +
        file.completions.length + file.references.length;
    return probes * DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_MARGIN_MS;
}

const spec = activeRepoSpec();

suite(`Real-world workspace: ${spec.name} @ ${spec.tag} [VSIX-REALWORLD]`, () => {
    let root = '';
    let monitor: ResourceMonitor;
    let postAnalysisBaseline: ProcessSample;

    suiteSetup(async function (this: Mocha.Context) {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        root = verifyPinnedWorkspace(spec);
        await waitForLspReady();
        monitor = await ResourceMonitor.create(findServerPid, spec.budgets, spec.name);
        await closeAllEditors();
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        // Calibration artifact: the measured peaks for this run, written next
        // to the corpus checkouts (git-ignored). Budgets ratchet DOWN toward
        // these numbers — see [VSIX-REALWORLD-RESOURCES].
        if (root !== '' && monitor !== undefined) {
            const report = {
                repo: spec.name,
                tag: spec.tag,
                ...monitor.report(),
                assertions: assertionTotal(),
            };
            fs.writeFileSync(
                path.join(root, '..', `${spec.name}.metrics.json`),
                JSON.stringify(report, null, 2),
            );
        }
    });

    test('whole-workspace analysis completes, CPU settles, memory in budget', async function (this: Mocha.Context) {
        // The body sequentially spends up to THREE full budgets: the
        // workspace-symbol poll, the diagnostics-settle loop, and the CPU
        // settle windows — the mocha timeout must cover all of them.
        this.timeout(3 * spec.budgets.cpuSettleTimeoutMs + TIMEOUT_MARGIN_MS);
        const settled = await waitForWorkspaceAnalysis(spec, root);
        check(
            settled.length > 0,
            'analysis must publish diagnostics — the corpus is fetched without its dependencies',
        );
        const settledPct = await monitor.assertCpuSettles('post-analysis');
        check(settledPct >= 0, 'settled CPU percentage must be non-negative');
        monitor.assertMemoryWithinBudget('post-analysis');
        postAnalysisBaseline = monitor.last();
    });

    test('every published diagnostic obeys structural invariants', () => {
        // Fresh snapshot at execution time: validating a snapshot captured
        // before the CPU settled could silently check a stale subset.
        const snapshot = workspaceDiagnostics(root);
        check(
            snapshot.length > 0,
            'workspace must hold at least one basilisk diagnostic when invariants run',
        );
        assertDiagnosticInvariants(snapshot);
        monitor.assertMemoryWithinBudget('post-invariants');
    });

    for (const file of spec.files) {
        test(`interaction journey — ${file.path}`, async function (this: Mocha.Context) {
            this.timeout(journeyTimeoutMs(file));
            await runFileJourney(file, root, monitor);
        });
    }

    test('workspace symbol search resolves every pinned symbol', async () => {
        await runWorkspaceSymbolProbes(spec, root);
        monitor.assertMemoryWithinBudget('post-workspace-symbols');
    });

    test(`edit churn keeps diagnostics live and honest — ${spec.editChurn.path}`, async function (this: Mocha.Context) {
        // Each cycle sanctions two full churn polls (error arrives + revert
        // settles) — the mocha budget must cover all of them.
        this.timeout(spec.editChurn.cycles * 2 * CHURN_DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_MARGIN_MS);
        await runEditChurn(spec, root);
        monitor.assertMemoryWithinBudget('post-edit-churn');
    });

    test('open blitz: no leak, CPU settles back to idle', async function (this: Mocha.Context) {
        // One symbol poll per blitzed file, then a full CPU-settle window.
        this.timeout(
            spec.openBlitz.count * DIAGNOSTIC_TIMEOUT_MS +
            spec.budgets.cpuSettleTimeoutMs + TIMEOUT_MARGIN_MS,
        );
        await runOpenBlitz(spec, root, monitor);
        await monitor.assertCpuSettles('post-blitz');
        monitor.assertNoLeakSince(postAnalysisBaseline, 'post-blitz leak check');
        monitor.assertMemoryWithinBudget('final');
    });

    test('assertion density meets the floor', () => {
        check(
            assertionTotal() >= MIN_ASSERTIONS_PER_REPO,
            `suite executed ${assertionTotal()} counted assertions — floor is ${MIN_ASSERTIONS_PER_REPO}`,
        );
    });
});
