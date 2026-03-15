/**
 * LSP Fix-All E2E Tests for the Basilisk VS Code Extension.
 *
 * Tests file-level mass autofix and per-rule fix-all:
 * - `basilisk.fixFile` command applies edits and clears diagnostics
 * - `source.fixAll.basilisk` code action kind returned by server
 * - Multiple diagnostics fixed in a single action
 * - Per-rule "Fix all <BSK-XXXX> in this file" quickfix action
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 *   - The binary must be on PATH or the test will fail hard
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { execFileSync } from 'child_process';

const EXTENSION_ID = 'basilisk-lang.basilisk';
const DIAGNOSTIC_TIMEOUT_MS = 15_000;
const NO_DIAGNOSTIC_WAIT_MS = 5_000;

function findBasiliskBinary(): string | undefined {
    const envPath = process.env.BASILISK_EXECUTABLE_PATH;
    if (envPath && fs.existsSync(envPath)) {
        return envPath;
    }
    const workspaceRoot = path.resolve(__dirname, '../../../..');
    for (const profile of ['release', 'debug']) {
        const bin = path.join(workspaceRoot, 'target', profile, 'basilisk');
        if (fs.existsSync(bin)) return bin;
    }
    try {
        execFileSync('basilisk', ['--version'], { timeout: 5000 });
        return 'basilisk';
    } catch {
        return undefined;
    }
}

function waitForDiagnostics(
    uri: vscode.Uri,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS
): Promise<vscode.Diagnostic[]> {
    return new Promise((resolve) => {
        const existing = vscode.languages.getDiagnostics(uri);
        if (existing.length > 0) { resolve(existing); return; }
        const timeout = setTimeout(() => {
            sub.dispose();
            resolve(vscode.languages.getDiagnostics(uri));
        }, timeoutMs);
        const sub = vscode.languages.onDidChangeDiagnostics((e) => {
            if (e.uris.some((u) => u.toString() === uri.toString())) {
                const d = vscode.languages.getDiagnostics(uri);
                if (d.length > 0) { clearTimeout(timeout); sub.dispose(); resolve(d); }
            }
        });
    });
}

function waitForDiagnosticsCleared(
    uri: vscode.Uri,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS
): Promise<vscode.Diagnostic[]> {
    return new Promise((resolve) => {
        const existing = vscode.languages.getDiagnostics(uri);
        if (existing.length === 0) { resolve([]); return; }
        const timeout = setTimeout(() => {
            sub.dispose();
            resolve(vscode.languages.getDiagnostics(uri));
        }, timeoutMs);
        const sub = vscode.languages.onDidChangeDiagnostics((e) => {
            if (e.uris.some((u) => u.toString() === uri.toString())) {
                const d = vscode.languages.getDiagnostics(uri);
                if (d.length === 0) { clearTimeout(timeout); sub.dispose(); resolve([]); }
            }
        });
    });
}

async function openPythonFile(
    tmpDir: string,
    filename: string,
    content: string
): Promise<{ doc: vscode.TextDocument; uri: vscode.Uri }> {
    const filePath = path.join(tmpDir, filename);
    fs.writeFileSync(filePath, content, 'utf8');
    const uri = vscode.Uri.file(filePath);
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc, { preview: false });
    return { doc, uri };
}

async function closeAllEditors(): Promise<void> {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
}

function diagCode(d: vscode.Diagnostic): string | undefined {
    if (typeof d.code === 'object' && d.code !== null && 'value' in d.code) {
        return String(d.code.value);
    }
    if (typeof d.code === 'string') return d.code;
    return undefined;
}

function filterByCode(diagnostics: vscode.Diagnostic[], code: string): vscode.Diagnostic[] {
    return diagnostics.filter((d) => diagCode(d) === code);
}

suite('LSP Fix-All Tests', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(30_000);
        const binary = findBasiliskBinary();
        if (!binary) {
            throw new Error('Basilisk binary not found. Build with: cargo build -p basilisk-cli');
        }
        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-fixall-test-'));
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        if (ext && !ext.isActive) await ext.activate();

        // Poll until LSP is responsive.
        const dummyPath = path.join(tmpDir, '__init__.py');
        fs.writeFileSync(dummyPath, '', 'utf8');
        const dummyUri = vscode.Uri.file(dummyPath);
        const dummyDoc = await vscode.workspace.openTextDocument(dummyUri);
        await vscode.window.showTextDocument(dummyDoc);
        const deadline = Date.now() + 10_000;
        while (Date.now() < deadline) {
            try {
                const syms = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                    'vscode.executeDocumentSymbolProvider', dummyUri
                );
                if (syms !== null && syms !== undefined) break;
            } catch { /* not ready */ }
            await new Promise<void>((r) => setTimeout(r, 200));
        }
        await closeAllEditors();
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        if (tmpDir && fs.existsSync(tmpDir)) {
            fs.rmSync(tmpDir, { recursive: true, force: true });
        }
    });

    teardown(async () => { await closeAllEditors(); });

    // ----------------------------------------------------------------
    // 1. fixFile command applies edits and clears diagnostics
    // ----------------------------------------------------------------
    test('fixFile command applies edits and clears diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + 10_000);

        const { uri } = await openPythonFile(tmpDir, 'test_fix_file.py', 'x: int = 42\n');
        const diagnostics = await waitForDiagnostics(uri);
        assert.ok(
            filterByCode(diagnostics, 'BSK-W0050').length > 0,
            `Expected BSK-W0050, got: ${diagnostics.map((d) => diagCode(d)).join(', ')}`
        );

        await vscode.commands.executeCommand('basilisk.fixFile');

        const cleared = await waitForDiagnosticsCleared(uri);
        assert.strictEqual(
            filterByCode(cleared, 'BSK-W0050').length, 0,
            'W0050 should clear after fixFile'
        );
    });

    // ----------------------------------------------------------------
    // 2. source.fixAll code action returned for fixable diagnostics
    // ----------------------------------------------------------------
    test('source.fixAll code action returned for fixable diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);

        const { uri } = await openPythonFile(tmpDir, 'test_fixall_action.py', 'x: int = 42\n');
        await waitForDiagnostics(uri);

        const range = new vscode.Range(new vscode.Position(0, 0), new vscode.Position(1, 0));
        const actions = await vscode.commands.executeCommand<vscode.CodeAction[]>(
            'vscode.executeCodeActionProvider', uri, range,
            vscode.CodeActionKind.SourceFixAll.value
        );

        assert.ok(actions && actions.length > 0, 'Expected source.fixAll code actions');
        assert.ok(
            actions.some((a) => a.title.includes('Fix all auto-fixable issues')),
            `Expected fix-all action. Got: ${actions.map((a) => a.title).join(', ')}`
        );
    });

    // ----------------------------------------------------------------
    // 3. fixFile fixes multiple diagnostics across lines
    // ----------------------------------------------------------------
    test('fixFile fixes multiple diagnostics across lines', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + 10_000);

        const { uri } = await openPythonFile(
            tmpDir, 'test_fix_multi.py', 'x: int = 42\ny: str = "hello"\n'
        );
        const diagnostics = await waitForDiagnostics(uri);
        assert.ok(
            filterByCode(diagnostics, 'BSK-W0050').length >= 2,
            `Expected >= 2 W0050, got ${filterByCode(diagnostics, 'BSK-W0050').length}`
        );

        await vscode.commands.executeCommand('basilisk.fixFile');

        const cleared = await waitForDiagnosticsCleared(uri);
        assert.strictEqual(
            filterByCode(cleared, 'BSK-W0050').length, 0,
            'All W0050 should clear after fixFile'
        );
    });

    // ----------------------------------------------------------------
    // 4. fixFile on clean file is a no-op
    // ----------------------------------------------------------------
    test('fixFile on clean file is a no-op', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);

        const { doc, uri } = await openPythonFile(
            tmpDir, 'test_fix_noop.py', 'def clean(x: int) -> int:\n    return x\n'
        );
        await waitForDiagnosticsCleared(uri, NO_DIAGNOSTIC_WAIT_MS);
        const before = doc.getText();

        await vscode.commands.executeCommand('basilisk.fixFile');
        await new Promise<void>((r) => setTimeout(r, 1_000));

        assert.strictEqual(doc.getText(), before, 'fixFile should not modify clean file');
    });

    // ----------------------------------------------------------------
    // 5. Per-rule "Fix all <BSK-XXXX>" appears in quickfix menu
    // ----------------------------------------------------------------
    test('per-rule fix-all appears in quickfix menu for 2+ instances', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);

        const { uri } = await openPythonFile(
            tmpDir, 'test_fix_rule.py',
            'x: int = 42\ny: str = "hello"\nz: bool = True\n'
        );
        const diagnostics = await waitForDiagnostics(uri);
        const w0050s = filterByCode(diagnostics, 'BSK-W0050');
        assert.ok(
            w0050s.length >= 2,
            `Expected >= 2 W0050 diagnostics, got ${w0050s.length}`
        );

        // Request code actions at the range of the first W0050 diagnostic.
        const actions = await vscode.commands.executeCommand<vscode.CodeAction[]>(
            'vscode.executeCodeActionProvider', uri, w0050s[0].range
        );

        assert.ok(actions && actions.length > 0, 'Expected code actions');

        const ruleFixAll = actions.find((a) =>
            a.title.includes('Fix all `BSK-W0050` in this file')
        );
        assert.ok(
            ruleFixAll,
            `Expected per-rule fix-all action. Got: ${actions.map((a) => a.title).join(', ')}`
        );
        assert.ok(
            ruleFixAll.title.includes('fixes'),
            `Title should include fix count: ${ruleFixAll.title}`
        );
    });

    // ----------------------------------------------------------------
    // 6. Per-rule fix-all does NOT appear for single instance
    // ----------------------------------------------------------------
    test('per-rule fix-all does not appear for single instance', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);

        const { uri } = await openPythonFile(
            tmpDir, 'test_fix_rule_single.py', 'x: int = 42\n'
        );
        const diagnostics = await waitForDiagnostics(uri);
        const w0050s = filterByCode(diagnostics, 'BSK-W0050');
        assert.ok(w0050s.length > 0, 'Expected at least 1 W0050 diagnostic');

        const actions = await vscode.commands.executeCommand<vscode.CodeAction[]>(
            'vscode.executeCodeActionProvider', uri, w0050s[0].range
        );

        const ruleFixAll = (actions ?? []).find((a) =>
            a.title.includes('Fix all `BSK-W0050` in this file')
        );
        assert.strictEqual(
            ruleFixAll, undefined,
            'Per-rule fix-all should NOT appear for a single instance'
        );
    });
});
