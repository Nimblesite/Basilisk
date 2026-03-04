/**
 * LSP Integration Tests for the Basilisk VS Code Extension.
 *
 * These tests exercise REAL LSP functionality by opening Python files,
 * waiting for the language server to respond, and asserting on actual
 * diagnostics, hover info, completions, and document symbols.
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 *   - The binary must be on PATH or the test will skip gracefully
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
const NO_DIAGNOSTIC_WAIT_MS = 5_000;

/** Time (ms) to wait for the LSP server to fully start. */
const SERVER_START_WAIT_MS = 5_000;

/**
 * Resolves the absolute path to the basilisk binary built from Cargo.
 * Returns undefined if the binary does not exist.
 */
function findBasiliskBinary(): string | undefined {
    // Check the workspace-root debug build first.
    const workspaceRoot = path.resolve(__dirname, '../../../../..');
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

suite('LSP Integration Tests', () => {
    let tmpDir: string;
    let basiliskBinary: string | undefined;

    suiteSetup(async function () {
        // Increase suite-level timeout for server startup.
        this.timeout(30_000);

        basiliskBinary = findBasiliskBinary();
        if (!basiliskBinary) {
            // Cannot run LSP tests without the binary. All tests will
            // skip individually, but we set up the directory anyway.
            console.warn(
                'Basilisk binary not found. LSP integration tests will be skipped. ' +
                'Build with: cargo build -p basilisk-cli'
            );
        }

        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-lsp-test-'));

        // Ensure the extension is activated.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        if (ext && !ext.isActive) {
            await ext.activate();
        }

        // Give the LSP server time to fully initialize.
        await new Promise<void>((resolve) => setTimeout(resolve, SERVER_START_WAIT_MS));
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
        if (!basiliskBinary) {
            this.skip();
            return;
        }

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
        const basiliskDiags = diagnostics.filter(
            (d) =>
                d.source === 'basilisk' ||
                (typeof d.code === 'object' &&
                    d.code !== null &&
                    'value' in d.code &&
                    typeof d.code.value === 'string' &&
                    d.code.value.startsWith('BSK-E'))
        );

        assert.ok(
            basiliskDiags.length > 0,
            `Expected diagnostics from Basilisk (source="basilisk" or code starting with BSK-E). ` +
            `Got: ${diagnostics.map((d) => `source=${d.source}, code=${JSON.stringify(d.code)}`).join('; ')}`
        );
    });

    // ----------------------------------------------------------------
    // 2. Diagnostics clear when the file is closed
    // ----------------------------------------------------------------
    test('diagnostics clear on file close', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS * 2 + 5_000);
        if (!basiliskBinary) {
            this.skip();
            return;
        }

        const { uri } = await openPythonFile(
            tmpDir,
            'test_close.py',
            'def broken(x):\n    return x\n'
        );

        // Wait for diagnostics to appear first.
        const diagsBefore = await waitForDiagnostics(uri, DIAGNOSTIC_TIMEOUT_MS);
        assert.ok(
            diagsBefore.length > 0,
            'Expected diagnostics before closing the file'
        );

        // Close all editors (which triggers textDocument/didClose).
        await closeAllEditors();

        // Wait for diagnostics to clear.
        const diagsAfter = await waitForDiagnosticsCleared(uri, DIAGNOSTIC_TIMEOUT_MS);

        assert.strictEqual(
            diagsAfter.length,
            0,
            `Expected diagnostics to be cleared after closing the file, ` +
            `but found ${diagsAfter.length}`
        );
    });

    // ----------------------------------------------------------------
    // 3. No diagnostics for fully typed code
    // ----------------------------------------------------------------
    test('no diagnostics for clean, fully typed code', async function () {
        this.timeout(NO_DIAGNOSTIC_WAIT_MS + 5_000);
        if (!basiliskBinary) {
            this.skip();
            return;
        }

        const { uri } = await openPythonFile(
            tmpDir,
            'test_clean.py',
            'def greet(name: str) -> str:\n    return name\n'
        );

        // Wait a reasonable amount of time for the server to analyze.
        await new Promise<void>((resolve) => setTimeout(resolve, NO_DIAGNOSTIC_WAIT_MS));

        const diagnostics = vscode.languages.getDiagnostics(uri);
        const basiliskDiags = diagnostics.filter(
            (d) =>
                d.source === 'basilisk' ||
                (typeof d.code === 'object' &&
                    d.code !== null &&
                    'value' in d.code &&
                    typeof d.code.value === 'string' &&
                    d.code.value.startsWith('BSK-'))
        );

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
        if (!basiliskBinary) {
            this.skip();
            return;
        }

        const { uri } = await openPythonFile(
            tmpDir,
            'test_hover.py',
            'def helper(x: int) -> int:\n    return x + 1\n\nresult = helper(42)\n'
        );

        // Give the server time to index the file.
        await new Promise<void>((resolve) => setTimeout(resolve, 3_000));

        // Request hover at the call site "helper" on line 3, col ~10.
        const position = new vscode.Position(3, 10);
        const hovers = await vscode.commands.executeCommand<vscode.Hover[]>(
            'vscode.executeHoverProvider',
            uri,
            position
        );

        assert.ok(hovers, 'Expected hover result to be defined');
        assert.ok(
            hovers.length > 0,
            'Expected at least one hover result for the function call'
        );

        // Verify hover content contains something meaningful (function name or signature).
        const hoverTexts = hovers.flatMap((h) =>
            h.contents.map((c) => {
                if (typeof c === 'string') return c;
                if ('value' in c) return c.value;
                return '';
            })
        );

        const combinedHover = hoverTexts.join('\n');
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
        if (!basiliskBinary) {
            this.skip();
            return;
        }

        const { uri } = await openPythonFile(
            tmpDir,
            'test_completion.py',
            'def my_helper_function(x: int) -> int:\n    return x\n\nmy_\n'
        );

        // Give the server time to index.
        await new Promise<void>((resolve) => setTimeout(resolve, 3_000));

        // Request completions at the "my_" prefix on line 3.
        const position = new vscode.Position(3, 3);
        const completions = await vscode.commands.executeCommand<vscode.CompletionList>(
            'vscode.executeCompletionItemProvider',
            uri,
            position
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
        if (!basiliskBinary) {
            this.skip();
            return;
        }

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

        // Give the server time to index.
        await new Promise<void>((resolve) => setTimeout(resolve, 3_000));

        const symbols = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
            'vscode.executeDocumentSymbolProvider',
            uri
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
