// Implements [VSIX-LSP-CLIENT-CONFIGURATION]. See docs/specs/VSIX-SPEC.md#VSIX-LSP-CLIENT-CONFIGURATION
/**
 * LSP Integration Tests for the Basilisk VS Code Extension.
 *
 * These tests exercise REAL LSP functionality by opening Python files,
 * waiting for the language server to respond, and asserting on actual
 * diagnostics, hover info, completions, and document symbols.
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 *   - The binary must be on PATH or the test will fail hard
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import {
    DIAGNOSTIC_TIMEOUT_MS,
    NO_DIAGNOSTIC_WAIT_MS,
    SUITE_SETUP_TIMEOUT_MS,
    closeAllEditors,
    extractHoverText,
    findBasiliskBinary,
    openPythonFile,
    pollUntilResult,
    waitForDiagnostics,
    waitForDiagnosticsCleared,
    waitForLspReady,
} from './test-helpers';
import { captureScreenshot } from './screenshot';

/** Extra buffer (ms) added to test-level timeouts beyond the core wait. */
const TIMEOUT_BUFFER_MS = 5_000;

/** Large buffer (ms) for tests that do multiple diagnostic waits. */
const LARGE_TIMEOUT_BUFFER_MS = 10_000;

// ── Test-specific line/column positions ──────────────────────────────

/** Line number where the hover target ("result = helper(42)") appears. */
const HOVER_TARGET_LINE = 3;

/** Column of "helper" in the hover target line. */
const HOVER_TARGET_COLUMN = 10;

/** Line number of the completion trigger ("my_\n"). */
const COMPLETION_TRIGGER_LINE = 3;

/** Column of the completion trigger. */
const COMPLETION_TRIGGER_COLUMN = 3;

/** Max completion items to display in assertion messages. */
const COMPLETION_PREVIEW_LIMIT = 10;

/**
 * Filter diagnostics to only those produced by the Basilisk LSP server.
 */
function filterBasiliskDiagnostics(diags: vscode.Diagnostic[]): vscode.Diagnostic[] {
    return diags.filter(
        (d) =>
            d.source === 'basilisk' ||
            (typeof d.code === 'object' &&
                d.code !== null &&
                'value' in d.code &&
                typeof d.code.value === 'string' &&
                d.code.value.startsWith('BSK'))
    );
}

/**
 * Recursively flatten document symbol names so we can search
 * through nested symbols (e.g. methods inside classes).
 */
function flattenSymbolNames(symbols: vscode.DocumentSymbol[]): string[] {
    const names: string[] = [];
    for (const sym of symbols) {
        names.push(sym.name);
        if (sym.children.length > 0) {
            names.push(...flattenSymbolNames(sym.children));
        }
    }
    return names;
}

// eslint-disable-next-line max-lines-per-function
suite('LSP Integration Tests', () => {
    let tmpDir: string;
    let _basiliskBinary: string | undefined;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);

        _basiliskBinary = findBasiliskBinary();
        if (_basiliskBinary === undefined) {
            throw new Error(
                'Basilisk binary not found. Build with: cargo build -p basilisk-cli'
            );
        }

        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-lsp-test-'));

        await waitForLspReady();
        await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        if (tmpDir !== undefined && tmpDir !== '' && fs.existsSync(tmpDir)) {
            fs.rmSync(tmpDir, { recursive: true, force: true });
        }
    });

    teardown(async () => {
        await closeAllEditors();
    });

    // ----------------------------------------------------------------
    // 1. Diagnostics appear on a Python file with type errors
    // ----------------------------------------------------------------
    test('diagnostics appear for untyped function parameter', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_BUFFER_MS);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_untyped.py',
            'def greet(name):\n    return name\n'
        );

        const diagnostics = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);

        assert.ok(
            diagnostics.length > 0,
            'Expected at least one diagnostic for an untyped function parameter'
        );

        // Verify the diagnostic is from Basilisk.
        const basiliskDiags = filterBasiliskDiagnostics(diagnostics);

        assert.ok(
            basiliskDiags.length > 0,
            `Expected diagnostics from Basilisk. ` +
            `Got: ${diagnostics.map((d) => `source=${d.source}, code=${JSON.stringify(d.code)}`).join('; ')}`
        );

        // Capture a picture of the editor with live Basilisk diagnostics into
        // the gitignored .screenshots/ folder for local debugging. Best-effort:
        // never fails the test, never uploaded as a CI artifact.
        await captureScreenshot('diagnostics-untyped-parameter');
    });

    // ----------------------------------------------------------------
    // 2. Diagnostics clear when the file is closed
    // ----------------------------------------------------------------
    test('diagnostics clear when errors are fixed', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + TIMEOUT_BUFFER_MS);

        const { doc, uri } = await openPythonFile(
            tmpDir,
            'test_clear.py',
            'def broken(x):\n    return x\n'
        );

        // Wait for diagnostics to appear first.
        const diagsBefore = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
        assert.ok(
            diagsBefore.length > 0,
            'Expected diagnostics for code with missing type annotations'
        );

        // Fix the code by adding type annotations.
        const edit = new vscode.WorkspaceEdit();
        const fullRange = new vscode.Range(
            new vscode.Position(0, 0),
            new vscode.Position(doc.lineCount, 0)
        );
        edit.replace(uri, fullRange, 'def broken(x: int) -> int:\n    return x\n');
        const applied = await vscode.workspace.applyEdit(edit);
        assert.ok(applied, 'Expected the edit to be applied');

        // Wait for diagnostics to clear after the fix.
        const diagsAfter = await waitForDiagnosticsCleared(uri, DIAGNOSTIC_TIMEOUT_MS);

        assert.strictEqual(
            diagsAfter.length,
            0,
            `Expected diagnostics to be cleared after fixing the code, ` +
            `but found ${diagsAfter.length}`
        );
    });

    // ----------------------------------------------------------------
    // 3. No diagnostics for fully typed code
    // ----------------------------------------------------------------
    test('no diagnostics for clean, fully typed code', async function () {
        this.timeout(NO_DIAGNOSTIC_WAIT_MS + TIMEOUT_BUFFER_MS);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_clean.py',
            'def greet(name: str) -> str:\n    return name\n'
        );

        // Wait for the server to process the file (no diagnostics expected).
        await waitForDiagnosticsCleared(uri, NO_DIAGNOSTIC_WAIT_MS);

        const diagnostics = vscode.languages.getDiagnostics(uri);
        const basiliskDiags = filterBasiliskDiagnostics(diagnostics);

        assert.strictEqual(
            basiliskDiags.length,
            0,
            `Expected zero Basilisk diagnostics for clean code, ` +
            `but found ${basiliskDiags.length}: ${
            basiliskDiags.map((d) => d.message).join('; ')}`
        );
    });

    // ----------------------------------------------------------------
    // 4. Hover provides type information
    // ----------------------------------------------------------------
    test('hover provides type information for a function', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_BUFFER_MS);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_hover.py',
            'def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n'
        );

        // Poll until the server has indexed and returns hover results.
        const position = new vscode.Position(HOVER_TARGET_LINE, HOVER_TARGET_COLUMN);
        const hovers = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.Hover[]>(
                'vscode.executeHoverProvider', uri, position
            ).then((r) => r, () => [] as vscode.Hover[]),
            predicate: (r) => r !== null && r !== undefined && r.length > 0,
        });

        assert.ok(hovers !== undefined, 'Expected hover result to be defined');
        assert.ok(
            hovers.length > 0,
            'Expected at least one hover result for the function call'
        );

        // Verify hover content contains something meaningful (function name or signature).
        const combinedHover = extractHoverText(hovers);
        assert.ok(
            combinedHover.length > 0,
            `Expected hover to contain text about the function, but got empty hover content`
        );
    });

    // ----------------------------------------------------------------
    // 5. Completions include local symbols
    // ----------------------------------------------------------------
    test('completions include local function names', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_BUFFER_MS);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_completion.py',
            'def my_helper_function(x: int) -> int:\n    return x\n\nmy_\n'
        );

        // Poll until the server returns completions.
        const position = new vscode.Position(COMPLETION_TRIGGER_LINE, COMPLETION_TRIGGER_COLUMN);
        const completions = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.CompletionList>(
                'vscode.executeCompletionItemProvider', uri, position
            ).then((r) => r, () => null),
            predicate: (r) => r !== null && r !== undefined && r.items.length > 0,
        });

        assert.ok(completions !== null && completions !== undefined, 'Expected completion result to be defined');

        const items = completions.items;
        assert.ok(
            items.length > 0,
            'Expected at least one completion item'
        );

        // Check if our function appears in the completions.
        const hasHelper = items.some((item) => {
            const label = typeof item.label === 'string' ? item.label : item.label.label;
            return label.includes('my_helper_function');
        });

        assert.ok(
            hasHelper,
            `Expected completions to include 'my_helper_function'. ` +
            `Got: ${items.slice(0, COMPLETION_PREVIEW_LIMIT).map((i) => (typeof i.label === 'string' ? i.label : i.label.label)).join(', ')}`
        );
    });

    // ----------------------------------------------------------------
    // 6. Document symbols include classes and functions
    // ----------------------------------------------------------------
    test('document symbols include class and function names', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_BUFFER_MS);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_symbols.py',
            [
                'class MyClass:',
                '    def method(self) -> None:',
                '        pass',
                '',
                'def standalone_function(x: int) -> int:',
                '    return x',
                '',
            ].join('\n')
        );

        // Poll until the server returns document symbols.
        const symbols = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                'vscode.executeDocumentSymbolProvider', uri
            ).then((r) => r, () => [] as vscode.DocumentSymbol[]),
            predicate: (r) => r !== null && r !== undefined && r.length > 0,
        });

        assert.ok(symbols !== undefined, 'Expected document symbols to be defined');
        assert.ok(symbols.length > 0, 'Expected at least one document symbol');

        // Flatten symbols (classes may nest their methods).
        const allNames = flattenSymbolNames(symbols);

        assert.ok(
            allNames.includes('MyClass'),
            `Expected symbols to include 'MyClass'. Got: ${allNames.join(', ')}`
        );

        assert.ok(
            allNames.includes('standalone_function'),
            `Expected symbols to include 'standalone_function'. Got: ${allNames.join(', ')}`
        );
    });

    // ----------------------------------------------------------------
    // 7. didChange updates diagnostics
    // ----------------------------------------------------------------
    test('did_change updates diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + LARGE_TIMEOUT_BUFFER_MS);

        // Open a fully typed file — should produce zero Basilisk diagnostics.
        const { doc, uri } = await openPythonFile(
            tmpDir,
            'test_didchange.py',
            'def greet(name: str) -> str:\n    return name\n'
        );

        // Wait for the server to process the clean file (no diagnostics expected).
        await waitForDiagnosticsCleared(uri, NO_DIAGNOSTIC_WAIT_MS);

        const diagsBefore = vscode.languages.getDiagnostics(uri);
        const basiliskBefore = filterBasiliskDiagnostics(diagsBefore);
        assert.strictEqual(
            basiliskBefore.length,
            0,
            `Expected zero Basilisk diagnostics for clean code before edit, ` +
            `but found ${basiliskBefore.length}`
        );

        // Apply an edit that removes the type annotation, introducing an error.
        const edit = new vscode.WorkspaceEdit();
        const fullRange = new vscode.Range(
            new vscode.Position(0, 0),
            new vscode.Position(doc.lineCount, 0)
        );
        edit.replace(uri, fullRange, 'def greet(name):\n    return name\n');
        const applied = await vscode.workspace.applyEdit(edit);
        assert.ok(applied, 'Expected the workspace edit to be applied successfully');

        // Wait for diagnostics to appear after the change.
        const diagsAfter = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);

        assert.ok(
            diagsAfter.length > 0,
            'Expected at least one diagnostic after removing the type annotation'
        );

        const basiliskAfter = filterBasiliskDiagnostics(diagsAfter);
        assert.ok(
            basiliskAfter.length > 0,
            `Expected Basilisk diagnostics after removing annotation. ` +
            `Got: ${diagsAfter.map((d) => `source=${d.source}, code=${JSON.stringify(d.code)}`).join('; ')}`
        );
    });
});
