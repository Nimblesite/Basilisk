// Tests for [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * End-to-end regression for the "Type Checking" toggle (`basilisk.enabled`).
 *
 * GitHub #65 / #119. This drives a REAL VS Code window and a REAL Basilisk LSP
 * (not a mock, not a direct `executeCommand("basilisk.toggleFeature")` poke):
 * it flips the actual `basilisk.enabled` setting the toggle writes, then asserts
 * the observable downstream effect a user sees — Basilisk diagnostics clear from
 * the editor when type checking is disabled and return when it is re-enabled.
 *
 * The toggle kept getting reported as broken because previous "fixes" were
 * validated by static code reads / mock-level tests that only checked the row
 * label flipped to "Disabled" (issue #65 comment). Those never proved the
 * diagnostics actually cleared. This test does. Implements the
 * [EXTACT-INFO-FEATURE-STATUS] "Type Checking" effect and the client half of
 * [ANALYSIS-ENABLED].
 */

import { delay } from '../../timeouts';
import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { getStore } from '../../extension';
import {
    ModuleTreeItem,
    workspaceHealthBadge,
    workspaceHealthMessage,
} from '../../module-explorer';
import type { HealthStats, ModuleNode } from '../../module-explorer-render';
import {
    closeAllEditors,
    DIAGNOSTIC_TIMEOUT_MS,
    openPythonFile,
    removeTestDir,
    waitForDiagnostics,
    waitForDiagnosticsCleared,
} from './test-helpers';

/** A snippet that produces Basilisk diagnostics in the test workspace. */
const ERRORING_SOURCE = 'def greet(name):\n    return f"Hello, {name}!"\n';

/** Buffer (ms) added on top of the multiple diagnostic waits this suite makes. */
const TIMEOUT_BUFFER_MS = 20_000;

// ── Panel-payload shapes, loosely typed on purpose ─────────────────────────
// The test asserts on raw wire JSON so it pins what the server actually serves,
// independent of the client-side interface declarations under test.

interface LooseHealthStats {
    readonly typeCheckingEnabled?: boolean;
    readonly coveragePercent?: number;
    readonly errors?: number;
    readonly warnings?: number;
    readonly totalFiles?: number;
}

interface LooseModuleNode {
    readonly name: string;
    readonly path: string;
    readonly symbols?: readonly unknown[];
    readonly coveragePercent?: number;
    readonly errors?: number;
    readonly warnings?: number;
    readonly adopted?: boolean;
}

interface LoosePanelResponse {
    readonly modules: readonly LooseModuleNode[];
    readonly workspace: LooseHealthStats;
}

// The wire shapes above stay independent of the client interfaces on purpose.
// Where a payload is handed to the production renderers, it is CONVERTED here
// rather than asserted into their types: an `as never` would let the wire drift
// away from what those renderers actually require and still compile, which is
// the very drift this suite exists to catch.

/** Supply the one field the renderer requires that the wire may omit. */
function asHealthStats(wire: LooseHealthStats): HealthStats {
    return { ...wire, totalFiles: wire.totalFiles ?? 0 };
}

/**
 * Build the node `ModuleTreeItem` needs from a wire node.
 *
 * `symbols` is emptied and `kind` fixed: the row rendering these assertions
 * inspect (the coverage tint on `iconPath`) is derived from `coveragePercent`
 * alone, so neither field can affect the outcome.
 */
function asModuleNode(wire: LooseModuleNode): ModuleNode {
    return { ...wire, kind: 'module', symbols: [] };
}

/** The icon a row resolved to, proven to be a themed icon rather than assumed. */
function themeIcon(item: vscode.TreeItem): vscode.ThemeIcon {
    assert.ok(
        item.iconPath instanceof vscode.ThemeIcon,
        'a module row renders a ThemeIcon, which is what carries the coverage tint',
    );
    return item.iconPath;
}

/** Fetch a panel payload from the REAL running LSP via executeCommand. */
async function fetchPanelPayload(command: string): Promise<LoosePanelResponse> {
    const client = getStore()?.client.value;
    assert.ok(client, 'LSP client must exist to fetch panel data');
    assert.ok(client.isRunning(), 'LSP client must be running to fetch panel data');
    const result = await client.sendRequest<LoosePanelResponse>(
        'workspace/executeCommand',
        { command, arguments: [{}] },
    );
    assert.ok(result, `${command} must return a payload`);
    return result;
}

/** Poll until `probe()` is true or the timeout elapses; returns the final value. */
async function pollUntil(probe: () => boolean, timeoutMs: number): Promise<boolean> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        if (probe()) { return true; }
        await delay(200);
    }
    return probe();
}

/** Enabled state: grading is served, flagged, and renders "% typed" + a red tint. */
function assertGradingServed(payload: LoosePanelResponse, moduleNeedle: string): void {
    assert.strictEqual(
        payload.workspace.typeCheckingEnabled, true,
        'enabled payload must stamp typeCheckingEnabled=true',
    );
    assert.strictEqual(
        typeof payload.workspace.coveragePercent, 'number',
        'enabled payload must carry the workspace coverage rollup',
    );
    const module = payload.modules.find((m) => m.path.includes(moduleNeedle));
    assert.ok(module, 'the opened module must appear in the panel payload');
    assert.strictEqual(
        typeof module.coveragePercent, 'number',
        'enabled module nodes carry coverage',
    );
    assert.match(
        workspaceHealthMessage(asHealthStats(payload.workspace)),
        /% typed/,
        'enabled header renders "NN% typed"',
    );
    const item = new ModuleTreeItem(asModuleNode(module));
    assert.ok(
        themeIcon(item).color !== undefined,
        'enabled low-coverage module row is coverage-tinted (red)',
    );
}

/** Disabled state (#119): payload, header chrome, and row tint all neutral. */
function assertDisabledStateNeutral(payload: LoosePanelResponse, moduleNeedle: string): void {
    assert.strictEqual(
        payload.workspace.typeCheckingEnabled, false,
        'disabled payload must stamp typeCheckingEnabled=false (#119)',
    );
    assert.strictEqual(
        payload.workspace.coveragePercent, undefined,
        'disabled workspace rollup must not carry a coverage % — the "63% typed" header source (#119)',
    );
    assert.strictEqual(
        payload.workspace.errors, undefined,
        'disabled workspace rollup must not carry error tallies (#119)',
    );
    const module = payload.modules.find((m) => m.path.includes(moduleNeedle));
    assert.ok(module, 'modules stay listed for navigation while disabled');
    for (const field of ['coveragePercent', 'errors', 'warnings', 'adopted'] as const) {
        assert.strictEqual(
            module[field], undefined,
            `disabled module nodes must omit grading field '${field}' (#119)`,
        );
    }

    // Header chrome: no "% typed", explicit disabled wording, no badge.
    const message = workspaceHealthMessage(asHealthStats(payload.workspace));
    assert.doesNotMatch(
        message, /% typed/,
        'disabled header must NOT display "NN% typed" (#119)',
    );
    assert.match(
        message, /disabled/i,
        'disabled header must say type checking is off',
    );
    assert.strictEqual(
        workspaceHealthBadge(asHealthStats(payload.workspace)), undefined,
        'disabled view must carry no diagnostics badge (#119)',
    );

    // Row rendering: no coverage tint — the "red rows" from the report.
    const item = new ModuleTreeItem(asModuleNode(module));
    assert.strictEqual(
        themeIcon(item).color, undefined,
        'disabled module rows must not be coverage-tinted red (#119)',
    );
}

// eslint-disable-next-line max-lines-per-function -- suite callback contains all tests
suite('Type Checking Toggle (basilisk.enabled)', function () {
    let tmpDir: string;

    suiteSetup(() => {
        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        assert.ok(workspaceRoot, 'toggle integration tests require the fixture workspace');
        // BSK-0001 is intentionally opt-in. Keep the fixture under the real
        // workspace so its pyproject.toml enables the diagnostic this suite
        // toggles; an OS-temp file correctly receives the default rule policy.
        tmpDir = fs.mkdtempSync(path.join(workspaceRoot, '.bsk-enabled-test-'));
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        if (tmpDir !== undefined && tmpDir !== '' && fs.existsSync(tmpDir)) {
            removeTestDir(tmpDir);
        }
    });

    teardown(async () => {
        await closeAllEditors();
    });

    test('disabling clears Basilisk diagnostics; re-enabling restores them', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 3 + TIMEOUT_BUFFER_MS);

        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalEnabled = cfg.get<boolean>('enabled');

        try {
            // Start from a known-enabled state.
            await cfg.update('enabled', true, vscode.ConfigurationTarget.Workspace);

            // Open an erroring file — diagnostics must appear while enabled.
            const { uri } = await openPythonFile(tmpDir, 'type_checking_toggle.py', ERRORING_SOURCE);
            const openDiags = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.ok(
                openDiags.length > 0,
                'precondition: Basilisk diagnostics must be present while type checking is enabled'
            );

            // Flip the Type Checking toggle OFF (the setting the panel writes).
            await cfg.update('enabled', false, vscode.ConfigurationTarget.Workspace);

            // The whole point of the toggle (#119): diagnostics must clear.
            const cleared = await waitForDiagnosticsCleared(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.strictEqual(
                cleared.length,
                0,
                'disabling Type Checking must clear Basilisk diagnostics from the editor (#119)'
            );

            // Flip it back ON — diagnostics must return (the toggle is reversible).
            await cfg.update('enabled', true, vscode.ConfigurationTarget.Workspace);
            const restored = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.ok(
                restored.length > 0,
                're-enabling Type Checking must restore Basilisk diagnostics'
            );
        } finally {
            await cfg.update('enabled', originalEnabled, vscode.ConfigurationTarget.Workspace);
            await closeAllEditors();
        }
    });

    // GitHub #119 showstopper reopen (v0.25.0): the diagnostics gate alone is not
    // enough — the MODULES / Type Health surfaces kept serving "% typed", red
    // rows, and error tallies while Type Checking was disabled. This pins the
    // whole grading pipeline against the REAL LSP: payloads, header chrome, and
    // row tinting must all go neutral on disable and recompute on re-enable.
    test('disabling hides all grading (payloads, header, red rows); re-enabling recomputes', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 3 + TIMEOUT_BUFFER_MS);

        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalEnabled = cfg.get<boolean>('enabled');

        try {
            await cfg.update('enabled', true, vscode.ConfigurationTarget.Workspace);

            // An unannotated function → low coverage that renders a red row while enabled.
            const { uri } = await openPythonFile(tmpDir, 'toggle_modules_panel.py', ERRORING_SOURCE);
            await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);

            // ── Enabled: grading is served and flagged ──────────────────────
            assertGradingServed(
                await fetchPanelPayload('basilisk.workspaceModules'),
                'toggle_modules_panel',
            );

            // ── Disable: every grading surface must go neutral ──────────────
            await cfg.update('enabled', false, vscode.ConfigurationTarget.Workspace);
            await waitForDiagnosticsCleared(uri, DIAGNOSTIC_TIMEOUT_MS);

            assertDisabledStateNeutral(
                await fetchPanelPayload('basilisk.workspaceModules'),
                'toggle_modules_panel',
            );

            // Sibling surface: basilisk.typeHealth must be gated the same way.
            const disabledHealth = await fetchPanelPayload('basilisk.typeHealth');
            assert.strictEqual(
                disabledHealth.workspace.typeCheckingEnabled, false,
                'typeHealth must stamp typeCheckingEnabled=false while disabled (#119)',
            );
            assert.strictEqual(
                disabledHealth.modules.length, 0,
                'typeHealth must serve no per-module grading while disabled (#119)',
            );

            // ── Re-enable: the panel recomputes, not merely un-hides ────────
            await cfg.update('enabled', true, vscode.ConfigurationTarget.Workspace);
            await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);

            const restored = await fetchPanelPayload('basilisk.workspaceModules');
            assert.strictEqual(restored.workspace.typeCheckingEnabled, true);
            assert.strictEqual(
                typeof restored.workspace.coveragePercent, 'number',
                're-enabling must recompute the coverage rollup',
            );
            assert.match(
                workspaceHealthMessage(asHealthStats(restored.workspace)),
                /% typed/,
                're-enabled header renders "NN% typed" again',
            );
        } finally {
            await cfg.update('enabled', originalEnabled, vscode.ConfigurationTarget.Workspace);
            await closeAllEditors();
        }
    });

    // #119 reopen, refresh half: the panel must repaint IMMEDIATELY on the toggle
    // transition. In a diagnostics-free workspace no publishDiagnostics event
    // fires, so the refresh must come from the server's own toggle notification
    // (basilisk/moduleChanged → analysisRevision bump), not as a side effect of
    // diagnostics clearing.
    test('toggle transition refreshes the panel even with zero diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + TIMEOUT_BUFFER_MS);

        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalEnabled = cfg.get<boolean>('enabled');
        const store = getStore();
        assert.ok(store, 'store must exist');

        try {
            await cfg.update('enabled', true, vscode.ConfigurationTarget.Workspace);
            // A fully-annotated, diagnostic-free file: nothing to clear on disable.
            await openPythonFile(tmpDir, 'toggle_clean_refresh.py', 'x: int = 1\n');
            // Let the open/analysis settle so later bumps are toggle-driven.
            await delay(2_000);

            const before = store.analysisRevision.value;
            await cfg.update('enabled', false, vscode.ConfigurationTarget.Workspace);
            const bumped = await pollUntil(
                () => store.analysisRevision.value > before,
                DIAGNOSTIC_TIMEOUT_MS,
            );
            assert.ok(
                bumped,
                'disabling must bump analysisRevision (panel refresh) even with no diagnostics to clear (#119)',
            );

            const afterDisable = store.analysisRevision.value;
            await cfg.update('enabled', true, vscode.ConfigurationTarget.Workspace);
            const bumpedAgain = await pollUntil(
                () => store.analysisRevision.value > afterDisable,
                DIAGNOSTIC_TIMEOUT_MS,
            );
            assert.ok(
                bumpedAgain,
                're-enabling must bump analysisRevision so the panel recomputes immediately (#119)',
            );
        } finally {
            await cfg.update('enabled', originalEnabled, vscode.ConfigurationTarget.Workspace);
            await closeAllEditors();
        }
    });
});
