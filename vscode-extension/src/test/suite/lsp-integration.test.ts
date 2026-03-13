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
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { execFileSync } from 'child_process';

const EXTENSION_ID = 'basilisk-lang.basilisk';

/** Maximum time (ms) to wait for diagnostics from the LSP server. */
const DIAGNOSTIC_TIMEOUT_MS = 15_000;

/** Time (ms) to wait for "no diagnostics" assertions. */
const NO_DIAGNOSTIC_WAIT_MS = 2_000;

/** Time (ms) to wait for the LSP server to fully start. */
const SERVER_START_WAIT_MS = 5_000;

/**
 * Poll an async function until it returns a truthy, non-empty result.
 * Avoids fixed 3-second sleeps by retrying at short intervals.
 */
async function pollUntilResult<T>(
    fn: () => PromiseLike<T>,
    predicate: (result: T) => boolean,
    timeoutMs: number = 5_000,
    intervalMs: number = 100
): Promise<T> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        const result = await fn();
        if (predicate(result)) return result;
        await new Promise<void>((r) => setTimeout(r, intervalMs));
    }
    // Final attempt
    return fn();
}

/**
 * Resolves the absolute path to the basilisk binary built from Cargo.
 * Returns undefined if the binary does not exist.
 */
function findBasiliskBinary(): string | undefined {
    // Check the workspace-root debug build first.
    // __dirname at runtime is vscode-extension/out/test/suite/
    // Go up 4 levels to reach the repo root (Basilisk/).
    const workspaceRoot = path.resolve(__dirname, '../../../..');
    const debugBinary = path.join(workspaceRoot, 'target', 'debug', 'basilisk');
    if (fs.existsSync(debugBinary)) {
        return debugBinary;
    }

    // Fall back to checking if `basilisk` is on PATH.
    try {
        execFileSync('basilisk', ['--version'], { timeout: 5000 });
        return 'basilisk';
    } catch {
        return undefined;
    }
}

/**
 * Wait until at least one diagnostic appears for the given URI,
 * or until the timeout elapses — whichever comes first.
 */
function waitForDiagnostics(
    uri: vscode.Uri,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS
): Promise<vscode.Diagnostic[]> {
    return new Promise((resolve) => {
        // If diagnostics already exist, resolve immediately.
        const existing = vscode.languages.getDiagnostics(uri);
        if (existing.length > 0) {
            resolve(existing);
            return;
        }

        const timeout = setTimeout(() => {
            disposable.dispose();
            resolve(vscode.languages.getDiagnostics(uri));
        }, timeoutMs);

        const disposable = vscode.languages.onDidChangeDiagnostics((event) => {
            if (event.uris.some((u) => u.toString() === uri.toString())) {
                const diags = vscode.languages.getDiagnostics(uri);
                if (diags.length > 0) {
                    clearTimeout(timeout);
                    disposable.dispose();
                    resolve(diags);
                }
            }
        });
    });
}

/**
 * Wait for diagnostics to clear (reach zero) for the given URI,
 * or until the timeout elapses.
 */
function waitForDiagnosticsCleared(
    uri: vscode.Uri,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS
): Promise<vscode.Diagnostic[]> {
    return new Promise((resolve) => {
        const existing = vscode.languages.getDiagnostics(uri);
        if (existing.length === 0) {
            resolve([]);
            return;
        }

        const timeout = setTimeout(() => {
            disposable.dispose();
            resolve(vscode.languages.getDiagnostics(uri));
        }, timeoutMs);

        const disposable = vscode.languages.onDidChangeDiagnostics((event) => {
            if (event.uris.some((u) => u.toString() === uri.toString())) {
                const diags = vscode.languages.getDiagnostics(uri);
                if (diags.length === 0) {
                    clearTimeout(timeout);
                    disposable.dispose();
                    resolve([]);
                }
            }
        });
    });
}

/**
 * Create a temporary Python file, open it in the editor, and return
 * the document + URI. Caller is responsible for cleanup via tmpDir.
 */
async function openPythonFile(
    tmpDir: string,
    filename: string,
    content: string
): Promise<{ doc: vscode.TextDocument; uri: vscode.Uri }> {
    const filePath = path.join(tmpDir, filename);
    fs.writeFileSync(filePath, content, 'utf8');
    const uri = vscode.Uri.file(filePath);
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc);
    return { doc, uri };
}

/**
 * Close all open editors to avoid cross-test pollution.
 */
async function closeAllEditors(): Promise<void> {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
}

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
 * Extract hover text content from hover results.
 */
function extractHoverText(hovers: vscode.Hover[]): string {
    return hovers
        .flatMap((h) =>
            h.contents.map((c) => {
                if (typeof c === 'string') return c;
                if ('value' in c) return c.value;
                return '';
            })
        )
        .join('\n');
}

/**
 * Wait for the LSP server to index a file before requesting features.
 */
function waitForIndexing(ms: number = 3_000): Promise<void> {
    return new Promise<void>((resolve) => setTimeout(resolve, ms));
}

suite('LSP Integration Tests', () => {
    let tmpDir: string;
    let basiliskBinary: string | undefined;

    suiteSetup(async function () {
        // Increase suite-level timeout for server startup.
        this.timeout(30_000);

        basiliskBinary = findBasiliskBinary();
        if (!basiliskBinary) {
            throw new Error(
                'Basilisk binary not found. Build with: cargo build -p basilisk-cli'
            );
        }

        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-lsp-test-'));

        // Ensure the extension is activated.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        if (ext && !ext.isActive) {
            await ext.activate();
        }

        // Wait for the LSP server to be responsive (poll with a lightweight request).
        const dummyUri = vscode.Uri.file(path.join(tmpDir, '__init__.py'));
        fs.writeFileSync(dummyUri.fsPath, '', 'utf8');
        const dummyDoc = await vscode.workspace.openTextDocument(dummyUri);
        await vscode.window.showTextDocument(dummyDoc);
        await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                'vscode.executeDocumentSymbolProvider',
                dummyUri
            ).then((r) => r, () => null),
            (r) => r !== null && r !== undefined,
            SERVER_START_WAIT_MS,
            200
        );
        await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        if (tmpDir && fs.existsSync(tmpDir)) {
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
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 5_000);

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
    });

    // ----------------------------------------------------------------
    // 2. Diagnostics clear when the file is closed
    // ----------------------------------------------------------------
    test('diagnostics clear when errors are fixed', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + 5_000);

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
        this.timeout(NO_DIAGNOSTIC_WAIT_MS + 5_000);

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
            `but found ${basiliskDiags.length}: ` +
            basiliskDiags.map((d) => d.message).join('; ')
        );
    });

    // ----------------------------------------------------------------
    // 4. Hover provides type information
    // ----------------------------------------------------------------
    test('hover provides type information for a function', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 5_000);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_hover.py',
            'def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n'
        );

        // Poll until the server has indexed and returns hover results.
        const position = new vscode.Position(3, 10);
        const hovers = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.Hover[]>(
                'vscode.executeHoverProvider', uri, position
            ).then((r) => r, () => [] as vscode.Hover[]),
            (r) => r !== null && r !== undefined && r.length > 0
        );

        assert.ok(hovers, 'Expected hover result to be defined');
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
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 5_000);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_completion.py',
            'def my_helper_function(x: int) -> int:\n    return x\n\nmy_\n'
        );

        // Poll until the server returns completions.
        const position = new vscode.Position(3, 3);
        const completions = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.CompletionList>(
                'vscode.executeCompletionItemProvider', uri, position
            ).then((r) => r, () => null),
            (r) => r !== null && r !== undefined && r.items.length > 0
        );

        assert.ok(completions, 'Expected completion result to be defined');

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
            `Got: ${items.slice(0, 10).map((i) => (typeof i.label === 'string' ? i.label : i.label.label)).join(', ')}`
        );
    });

    // ----------------------------------------------------------------
    // 6. Document symbols include classes and functions
    // ----------------------------------------------------------------
    test('document symbols include class and function names', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 5_000);

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
        const symbols = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                'vscode.executeDocumentSymbolProvider', uri
            ).then((r) => r, () => [] as vscode.DocumentSymbol[]),
            (r) => r !== null && r !== undefined && r.length > 0
        );

        assert.ok(symbols, 'Expected document symbols to be defined');
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
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + 10_000);

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

    // ----------------------------------------------------------------
    // 8. Go-to-definition works through extension
    // ----------------------------------------------------------------
    test('go-to-definition works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 5_000);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_goto_def.py',
            [
                'def add_numbers(a: int, b: int) -> int:',
                '    return a + b',
                '',
                'result: int = add_numbers(1, 2)',
                '',
            ].join('\n')
        );

        // Poll until go-to-definition returns results.
        // "add_numbers" starts at column 14 in "result: int = add_numbers(1, 2)".
        const callPosition = new vscode.Position(3, 18);
        const locations = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.Location[]>(
                'vscode.executeDefinitionProvider', uri, callPosition
            ).then((r) => r, () => [] as vscode.Location[]),
            (r) => r !== null && r !== undefined && r.length > 0
        );

        assert.ok(locations, 'Expected definition locations to be defined');
        assert.ok(
            locations.length > 0,
            'Expected at least one definition location for the function call'
        );

        // The definition should point to the function definition on line 0.
        const defLocation = locations[0];
        assert.strictEqual(
            defLocation.uri.toString(),
            uri.toString(),
            'Expected definition to be in the same file'
        );
        assert.strictEqual(
            defLocation.range.start.line,
            0,
            `Expected definition to be on line 0 (the function def), ` +
            `but got line ${defLocation.range.start.line}`
        );
    });

    // ----------------------------------------------------------------
    // 9. Signature help works through extension
    // ----------------------------------------------------------------
    test('signature help works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 5_000);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_sig_help.py',
            [
                'def greet(name: str, age: int) -> str:',
                '    return f"{name} is {age}"',
                '',
                'greet()',
                '',
            ].join('\n')
        );

        // Poll until signature help returns results.
        const position = new vscode.Position(3, 6);
        const signatureHelp = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.SignatureHelp>(
                'vscode.executeSignatureHelpProvider', uri, position, '('
            ).then((r) => r, () => null),
            (r) => r !== null && r !== undefined && r.signatures.length > 0
        );

        assert.ok(signatureHelp, 'Expected signature help result to be defined');
        assert.ok(
            signatureHelp.signatures.length > 0,
            'Expected at least one signature in signature help'
        );

        // Verify the signature contains both parameter names.
        const sig = signatureHelp.signatures[0];
        const paramLabels = sig.parameters.map((p) =>
            typeof p.label === 'string' ? p.label : sig.label.slice(p.label[0], p.label[1])
        );
        const allParamText = paramLabels.join(' ');

        assert.ok(
            allParamText.includes('name'),
            `Expected signature parameters to include 'name'. Got: ${paramLabels.join(', ')}`
        );
        assert.ok(
            allParamText.includes('age'),
            `Expected signature parameters to include 'age'. Got: ${paramLabels.join(', ')}`
        );
    });

    // ----------------------------------------------------------------
    // 10. Code actions provided for diagnostics
    // ----------------------------------------------------------------
    test('code actions provided for diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_code_actions.py',
            'def broken(x):\n    return x\n'
        );

        // Wait for diagnostics to appear (missing type annotation).
        const diagnostics = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);

        assert.ok(
            diagnostics.length > 0,
            'Expected at least one diagnostic for the untyped parameter'
        );

        // Use the range of the first diagnostic to request code actions.
        const diagRange = diagnostics[0].range;
        const codeActions = await vscode.commands.executeCommand<vscode.CodeAction[]>(
            'vscode.executeCodeActionProvider',
            uri,
            diagRange
        );

        assert.ok(codeActions, 'Expected code actions result to be defined');
        assert.ok(
            codeActions.length > 0,
            `Expected at least one code action for the diagnostic. ` +
            `Diagnostic: ${diagnostics[0].message}`
        );

        // Verify the code action has a title (i.e. is well-formed).
        const firstAction = codeActions[0];
        assert.ok(
            firstAction.title && firstAction.title.length > 0,
            `Expected code action to have a non-empty title, got: "${firstAction.title}"`
        );
    });

    // ----------------------------------------------------------------
    // 11. Go-to-declaration works through extension
    // ----------------------------------------------------------------
    test('go-to-declaration works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 5_000);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_goto_decl.py',
            [
                'def compute(x: int) -> int:',
                '    return x * 2',
                '',
                'result: int = compute(10)',
                '',
            ].join('\n')
        );

        // Poll until declaration returns results.
        const callPosition = new vscode.Position(3, 18);
        const locations = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.Location[]>(
                'vscode.executeDeclarationProvider', uri, callPosition
            ).then((r) => r, () => [] as vscode.Location[]),
            (r) => r !== null && r !== undefined && r.length > 0
        );

        assert.ok(locations, 'Expected declaration locations to be defined');
        assert.ok(
            locations.length > 0,
            'Expected at least one declaration location for the function call'
        );

        const declLocation = locations[0];
        assert.strictEqual(
            declLocation.uri.toString(),
            uri.toString(),
            'Expected declaration to be in the same file'
        );
        assert.strictEqual(
            declLocation.range.start.line,
            0,
            `Expected declaration to be on line 0 (the function def), ` +
            `but got line ${declLocation.range.start.line}`
        );
    });

    // ----------------------------------------------------------------
    // 12. Go-to-type-definition works through extension
    // ----------------------------------------------------------------
    test('go-to-type-definition works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 5_000);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_goto_typedef.py',
            [
                'class MyData:',
                '    value: int',
                '',
                'instance: MyData = MyData()',
                '',
            ].join('\n')
        );

        // Poll until type definition returns results.
        const varPosition = new vscode.Position(3, 2);
        const locations = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.Location[]>(
                'vscode.executeTypeDefinitionProvider', uri, varPosition
            ).then((r) => r, () => [] as vscode.Location[]),
            (r) => r !== null && r !== undefined && r.length > 0
        );

        assert.ok(locations, 'Expected type definition locations to be defined');
        assert.ok(
            locations.length > 0,
            'Expected at least one type definition location'
        );

        const typeDefLocation = locations[0];
        assert.strictEqual(
            typeDefLocation.uri.toString(),
            uri.toString(),
            'Expected type definition to be in the same file'
        );
        assert.strictEqual(
            typeDefLocation.range.start.line,
            0,
            `Expected type definition to be on line 0 (class MyData), ` +
            `but got line ${typeDefLocation.range.start.line}`
        );
    });

    // ----------------------------------------------------------------
    // 13. Hover shows docstrings
    // ----------------------------------------------------------------
    test('hover shows docstring for function', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 5_000);

        const { uri } = await openPythonFile(
            tmpDir,
            'test_hover_docstring.py',
            [
                'def calculate(x: int) -> int:',
                '    """Compute the square of x."""',
                '    return x * x',
                '',
                'result: int = calculate(5)',
                '',
            ].join('\n')
        );

        // Poll until hover returns results.
        const position = new vscode.Position(0, 5);
        const hovers = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.Hover[]>(
                'vscode.executeHoverProvider', uri, position
            ).then((r) => r, () => [] as vscode.Hover[]),
            (r) => r !== null && r !== undefined && r.length > 0
        );

        assert.ok(hovers, 'Expected hover result to be defined');
        assert.ok(
            hovers.length > 0,
            'Expected at least one hover result'
        );

        const combinedHover = extractHoverText(hovers);
        assert.ok(
            combinedHover.includes('Compute the square of x'),
            `Expected hover to include docstring, but got: ${combinedHover}`
        );
    });
});

/**
 * Recursively flatten document symbol names so we can search
 * through nested symbols (e.g. methods inside classes).
 */
function flattenSymbolNames(symbols: vscode.DocumentSymbol[]): string[] {
    const names: string[] = [];
    for (const sym of symbols) {
        names.push(sym.name);
        if (sym.children && sym.children.length > 0) {
            names.push(...flattenSymbolNames(sym.children));
        }
    }
    return names;
}

// ============================================================
// Analysis Mode Tests
// ============================================================

suite('Analysis Mode Tests', () => {
    let tmpDir: string;
    let basiliskBinary: string | undefined;

    suiteSetup(function () {
        basiliskBinary = findBasiliskBinary();
        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'bsk-mode-test-'));
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        if (tmpDir && fs.existsSync(tmpDir)) {
            fs.rmSync(tmpDir, { recursive: true, force: true });
        }
    });

    teardown(async () => {
        await closeAllEditors();
    });

    // -------------------------------------------------------
    // Configuration schema tests — verify the setting exists
    // -------------------------------------------------------

    test('basilisk.analysisMode setting has correct default', () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const mode = cfg.get<string>('analysisMode');
        // Default is wholeModule.
        assert.strictEqual(
            mode,
            'wholeModule',
            `Expected default analysisMode to be 'wholeModule', got '${mode}'`
        );
    });

    test('basilisk.analysisMode accepts openFilesOnly', async () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const original = cfg.get<string>('analysisMode');
        try {
            await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);
            const mode = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
            assert.strictEqual(mode, 'openFilesOnly');
        } finally {
            await cfg.update('analysisMode', original, vscode.ConfigurationTarget.Workspace);
        }
    });

    test('basilisk.analysisMode accepts crossModule', async () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const original = cfg.get<string>('analysisMode');
        try {
            await cfg.update('analysisMode', 'crossModule', vscode.ConfigurationTarget.Workspace);
            const mode = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
            assert.strictEqual(mode, 'crossModule');
        } finally {
            await cfg.update('analysisMode', original, vscode.ConfigurationTarget.Workspace);
        }
    });

    test('basilisk.analysisMode can be reset to wholeModule', async () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        // Set to openFilesOnly first, then back to wholeModule.
        await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);
        await cfg.update('analysisMode', 'wholeModule', vscode.ConfigurationTarget.Workspace);
        const mode = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
        assert.strictEqual(mode, 'wholeModule', 'should be able to reset to wholeModule');
    });

    test('basilisk.analysisMode: all three enum values are accepted', async () => {
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const original = cfg.get<string>('analysisMode');
        const modes = ['openFilesOnly', 'wholeModule', 'crossModule'];
        try {
            for (const m of modes) {
                await cfg.update('analysisMode', m, vscode.ConfigurationTarget.Workspace);
                const current = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
                assert.strictEqual(current, m, `setting should accept '${m}'`);
            }
        } finally {
            await cfg.update('analysisMode', original, vscode.ConfigurationTarget.Workspace);
        }
    });

    // -------------------------------------------------------
    // Extension wiring tests — prove the extension reads and
    // forwards the setting to the LSP server correctly.
    // -------------------------------------------------------

    test('wholeModule mode: setting is wired into initializationOptions', async () => {
        // Structural test — the extension must read analysisMode and pass it
        // to the server. The extension source sets initializationOptions.analysisMode.
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const mode = cfg.get<string>('analysisMode') ?? 'wholeModule';
        const validModes = ['openFilesOnly', 'wholeModule', 'crossModule'];
        assert.ok(
            validModes.includes(mode),
            `analysisMode '${mode}' is not a valid mode. Expected one of: ${validModes.join(', ')}`
        );
    });

    test('openFilesOnly mode: disabling whole-module sets setting correctly', async () => {
        // Prove the user can turn OFF whole-module analysis (important for large projects).
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalMode = cfg.get<string>('analysisMode') ?? 'wholeModule';

        try {
            await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);
            const updated = vscode.workspace.getConfiguration('basilisk').get<string>('analysisMode');
            assert.strictEqual(
                updated,
                'openFilesOnly',
                `Expected analysisMode to be 'openFilesOnly' after update, got '${updated}'`
            );
            // Verify that this is a meaningful change from the default.
            assert.notStrictEqual(
                updated,
                'wholeModule',
                'openFilesOnly must be different from wholeModule default'
            );
        } finally {
            await cfg.update('analysisMode', originalMode, vscode.ConfigurationTarget.Workspace);
        }
    });

    // -------------------------------------------------------
    // Whole-module LSP behaviour: a file written to the VS Code
    // workspace root but NEVER opened in the editor must receive
    // diagnostics from the startup scan.
    //
    // The workspace root is test-fixtures/workspace/ (configured
    // in .vscode-test.mjs). Files written there are within the
    // LSP server's rootUri, so the wholeModule startup scan will
    // pick them up.
    // -------------------------------------------------------

    test('wholeModule: startup scan publishes diagnostics for closed file in workspace root', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 15_000);

        // Determine the workspace root that VS Code opened.
        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        assert.ok(
            workspaceRoot,
            'wholeModule scan test: no workspace folder configured. ' +
            'Ensure .vscode-test.mjs sets workspaceFolder.'
        );

        // Ensure wholeModule mode is set BEFORE the extension activates.
        // (The extension reads the setting during activate(), so changing it
        // here affects the server's initializationOptions.)
        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalMode = cfg.get<string>('analysisMode');
        await cfg.update('analysisMode', 'wholeModule', vscode.ConfigurationTarget.Workspace);

        try {
            // Write a Python file with type errors into the workspace root.
            // Do NOT open it — the whole-module scan must find it on its own.
            const closedFilePath = path.join(workspaceRoot, 'wm_scan_target.py');
            fs.writeFileSync(
                closedFilePath,
                'def greet(name):\n    return f"Hello, {name}!"\n',
                'utf8'
            );

            // Activate the extension (or restart to pick up the new file).
            const ext = vscode.extensions.getExtension(EXTENSION_ID);
            if (ext && !ext.isActive) {
                await ext.activate();
            }

            // Wait for the LSP server startup scan to complete.
            await new Promise<void>((resolve) => setTimeout(resolve, SERVER_START_WAIT_MS + 5_000));

            // The startup scan must have published diagnostics for the closed file.
            const closedFileUri = vscode.Uri.file(closedFilePath);
            const diags = await waitForDiagnostics(closedFileUri, DIAGNOSTIC_TIMEOUT_MS);

            assert.ok(
                diags.length > 0,
                'wholeModule: startup scan must publish diagnostics for a closed file ' +
                'that exists in the workspace root. Diagnostics were empty — either ' +
                'the scan did not run or the file was not analysed.'
            );

            // Verify the diagnostics are from Basilisk (not another linter).
            const basiliskDiags = filterBasiliskDiagnostics(diags);
            assert.ok(
                basiliskDiags.length > 0,
                `wholeModule: diagnostics must be from Basilisk (BSK codes), got: ` +
                `${JSON.stringify(diags.map(d => ({ source: d.source, code: d.code })))}`
            );

            // Cleanup the test file from the workspace root.
            fs.unlinkSync(closedFilePath);
        } finally {
            await cfg.update('analysisMode', originalMode, vscode.ConfigurationTarget.Workspace);
        }
    });

    test('openFilesOnly: startup scan does NOT run — closed workspace file gets no diagnostics', async function () {
        this.timeout(NO_DIAGNOSTIC_WAIT_MS + 10_000);

        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        assert.ok(
            workspaceRoot,
            'openFilesOnly test: no workspace folder configured. ' +
            'Ensure .vscode-test.mjs sets workspaceFolder.'
        );

        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalMode = cfg.get<string>('analysisMode');

        // Write a file with type errors into the workspace root.
        const closedFilePath = path.join(workspaceRoot, 'ofo_no_scan_target.py');
        fs.writeFileSync(
            closedFilePath,
            'def greet(name):\n    return f"Hello, {name}!"\n',
            'utf8'
        );

        try {
            await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);

            // Activate (or restart) the extension with openFilesOnly mode.
            const ext = vscode.extensions.getExtension(EXTENSION_ID);
            if (ext && !ext.isActive) {
                await ext.activate();
            }

            // Wait long enough for a scan to have run (if it was going to).
            await new Promise<void>((resolve) => setTimeout(resolve, NO_DIAGNOSTIC_WAIT_MS));

            // In openFilesOnly mode, the closed file must NOT have diagnostics.
            const closedFileUri = vscode.Uri.file(closedFilePath);
            const diags = vscode.languages.getDiagnostics(closedFileUri);
            assert.strictEqual(
                diags.length,
                0,
                'openFilesOnly: startup scan must NOT run — closed file should have zero diagnostics, ' +
                `got: ${JSON.stringify(diags)}`
            );
        } finally {
            fs.unlinkSync(closedFilePath);
            await cfg.update('analysisMode', originalMode, vscode.ConfigurationTarget.Workspace);
        }
    });

    test('openFilesOnly: opening a file produces diagnostics, closing clears them', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);

        const cfg = vscode.workspace.getConfiguration('basilisk');
        const originalMode = cfg.get<string>('analysisMode');

        try {
            await cfg.update('analysisMode', 'openFilesOnly', vscode.ConfigurationTarget.Workspace);

            // Open a file with type errors.
            const { uri } = await openPythonFile(
                tmpDir,
                'ofo_open_close.py',
                'def greet(name):\n    return f"Hello, {name}!"\n'
            );

            // Wait for diagnostics to appear (file is open, so should be analysed
            // regardless of mode).
            const openDiags = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
            assert.ok(
                openDiags.length > 0,
                'openFilesOnly: should have diagnostics while file is open'
            );

            // Close the file.
            await vscode.commands.executeCommand('workbench.action.closeActiveEditor');

            // In openFilesOnly mode the server clears diagnostics when the file is closed.
            const clearedDiags = await waitForDiagnosticsCleared(uri, NO_DIAGNOSTIC_WAIT_MS);
            assert.strictEqual(
                clearedDiags.length,
                0,
                'openFilesOnly: diagnostics should be cleared when file is closed'
            );
        } finally {
            await cfg.update('analysisMode', originalMode, vscode.ConfigurationTarget.Workspace);
        }
    });
});
