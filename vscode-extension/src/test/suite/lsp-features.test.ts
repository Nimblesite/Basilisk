/**
 * LSP Feature Tests for the Basilisk VS Code Extension.
 *
 * These tests exercise additional LSP capabilities (find references,
 * rename, inlay hints, formatting, document highlights) through the
 * VS Code extension command APIs.
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

/** Maximum time (ms) to wait for the LSP server to become responsive. */
const SERVER_START_WAIT_MS = 10_000;

/**
 * Poll an async function until it returns a truthy, non-empty result.
 * Avoids fixed sleeps by retrying at short intervals.
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
    return fn() as Promise<T>;
}

/**
 * Poll an async function until it returns a truthy, non-empty result.
 * Avoids fixed sleeps by retrying at short intervals.
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
    return fn() as Promise<T>;
}

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
 * or until the timeout elapses -- whichever comes first.
 */
function waitForDiagnostics(
    uri: vscode.Uri,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS
): Promise<vscode.Diagnostic[]> {
    return new Promise((resolve) => {
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

suite('LSP Feature Tests', () => {
    let tmpDir: string;
    let basiliskBinary: string | undefined;

    suiteSetup(async function () {
        this.timeout(30_000);

        basiliskBinary = findBasiliskBinary();
        if (!basiliskBinary) {
            console.warn(
                'Basilisk binary not found. LSP feature tests will be skipped. ' +
                'Build with: cargo build -p basilisk-cli'
            );
        }

        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-lsp-features-'));

        // Ensure the extension is activated.
        const ext = vscode.extensions.getExtension(EXTENSION_ID);
        if (ext && !ext.isActive) {
            await ext.activate();
        }

        // Poll until the LSP server is responsive.
        const dummyPath = path.join(tmpDir, '__init__.py');
        fs.writeFileSync(dummyPath, '', 'utf8');
        const dummyUri = vscode.Uri.file(dummyPath);
        const dummyDoc = await vscode.workspace.openTextDocument(dummyUri);
        await vscode.window.showTextDocument(dummyDoc);
        await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                'vscode.executeDocumentSymbolProvider', dummyUri
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
    // 1. Find references works through extension
    // ----------------------------------------------------------------
    test('find references works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);
        if (!basiliskBinary) {
            this.skip();
            return;
        }

        const source = [
            'def compute(x: int) -> int:',
            '    return x * 2',
            '',
            'result1: int = compute(10)',
            'result2: int = compute(20)',
            '',
        ].join('\n');

        const { uri } = await openPythonFile(tmpDir, 'test_references.py', source);

        // Poll until the server has indexed and returns reference results.
        const defPosition = new vscode.Position(0, 4);
        const locations = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.Location[]>(
                'vscode.executeReferenceProvider', uri, defPosition
            ).then((r) => r, () => [] as vscode.Location[]),
            (r) => r !== null && r !== undefined && r.length >= 3
        );

        assert.ok(locations, 'Expected reference results to be defined');
        assert.ok(
            Array.isArray(locations),
            'Expected reference results to be an array'
        );
        assert.ok(
            locations.length >= 3,
            `Expected at least 3 references (1 definition + 2 call sites), ` +
            `but got ${locations.length}: ${locations.map((loc) => `L${loc.range.start.line}:${loc.range.start.character}`).join(', ')}`
        );

        // Verify all locations point to the same file.
        const allSameFile = locations.every(
            (loc) => loc.uri.toString() === uri.toString()
        );
        assert.ok(
            allSameFile,
            'Expected all reference locations to be in the same file'
        );
    });

    // ----------------------------------------------------------------
    // 2. Rename symbol works through extension
    // ----------------------------------------------------------------
    test('rename symbol works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);
        if (!basiliskBinary) {
            this.skip();
            return;
        }

        const source = [
            'def old_name(x: int) -> int:',
            '    return x + 1',
            '',
            'value: int = old_name(5)',
            '',
        ].join('\n');

        const { uri } = await openPythonFile(tmpDir, 'test_rename.py', source);

        // Poll until the server has indexed and returns rename results.
        const defPosition = new vscode.Position(0, 4);
        const workspaceEdit = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.WorkspaceEdit>(
                'vscode.executeDocumentRenameProvider', uri, defPosition, 'new_name'
            ).then((r) => r, () => new vscode.WorkspaceEdit()),
            (r) => r !== null && r !== undefined && r.get(uri).length > 0
        );

        assert.ok(workspaceEdit, 'Expected workspace edit to be defined');

        // Get the text edits for our file.
        const edits = workspaceEdit.get(uri);
        assert.ok(
            edits && edits.length > 0,
            `Expected at least one text edit for the renamed file, ` +
            `but got ${edits ? edits.length : 0} edits`
        );

        // Verify that edits replace "old_name" with "new_name".
        const renameEdits = edits.filter(
            (edit) => edit.newText === 'new_name'
        );
        assert.ok(
            renameEdits.length >= 2,
            `Expected at least 2 rename edits (definition + call site), ` +
            `but got ${renameEdits.length}. All edits: ` +
            edits.map((e) => `"${e.newText}" at L${e.range.start.line}:${e.range.start.character}`).join(', ')
        );

        // Verify the edits cover both the definition line and the call-site line.
        const editLines = new Set(renameEdits.map((edit) => edit.range.start.line));
        assert.ok(
            editLines.has(0),
            'Expected a rename edit on line 0 (function definition)'
        );
        assert.ok(
            editLines.has(3),
            'Expected a rename edit on line 3 (call site)'
        );
    });

    // ----------------------------------------------------------------
    // 3. Inlay hints appear for unannotated variables
    // ----------------------------------------------------------------
    test('inlay hints appear for unannotated variables', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);
        if (!basiliskBinary) {
            this.skip();
            return;
        }

        const source = [
            'x = 42',
            'y = "hello"',
            'z = [1, 2, 3]',
            '',
        ].join('\n');

        const { doc, uri } = await openPythonFile(tmpDir, 'test_inlay_hints.py', source);

        // Poll until the server returns inlay hints.
        const fullRange = new vscode.Range(
            new vscode.Position(0, 0),
            new vscode.Position(doc.lineCount - 1, doc.lineAt(doc.lineCount - 1).text.length)
        );
        const hints = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.InlayHint[]>(
                'vscode.executeInlayHintProvider', uri, fullRange
            ).then((r) => r, () => [] as vscode.InlayHint[]),
            (r) => r !== null && r !== undefined && r.length >= 2
        );

        assert.ok(hints, 'Expected inlay hints result to be defined');
        assert.ok(
            Array.isArray(hints),
            'Expected inlay hints result to be an array'
        );
        assert.ok(
            hints.length >= 2,
            `Expected at least 2 inlay hints for unannotated variables (x, y), ` +
            `but got ${hints.length}`
        );

        // Verify hints have label content.
        const nonEmptyHints = hints.filter((hint) => {
            const label = typeof hint.label === 'string'
                ? hint.label
                : hint.label.map((part) => part.value).join('');
            return label.length > 0;
        });
        assert.ok(
            nonEmptyHints.length >= 2,
            `Expected at least 2 non-empty inlay hint labels, ` +
            `but got ${nonEmptyHints.length}`
        );
    });

    // ----------------------------------------------------------------
    // 4. Format document works through extension
    // ----------------------------------------------------------------
    test('format document works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);
        if (!basiliskBinary) {
            this.skip();
            return;
        }

        // Intentionally badly formatted Python code.
        const source = [
            'x=1',
            'y  =   "hello"',
            'def   foo(  a:int,b:str   )->None:',
            '    pass',
            '',
        ].join('\n');

        const { uri } = await openPythonFile(tmpDir, 'test_format.py', source);

        // Poll until the server returns formatting edits.
        const edits = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.TextEdit[]>(
                'vscode.executeFormatDocumentProvider', uri,
                { tabSize: 4, insertSpaces: true }
            ).then((r) => r, () => [] as vscode.TextEdit[]),
            (r) => r !== null && r !== undefined && r.length > 0
        );

        assert.ok(edits, 'Expected format edits to be defined');
        assert.ok(
            Array.isArray(edits),
            'Expected format edits to be an array'
        );
        assert.ok(
            edits.length > 0,
            'Expected at least one formatting edit for the badly formatted file'
        );

        // Verify at least one edit changes something (new text differs from original range).
        const meaningfulEdits = edits.filter((edit) => edit.newText.length > 0);
        assert.ok(
            meaningfulEdits.length > 0,
            `Expected at least one meaningful formatting edit, ` +
            `but all ${edits.length} edits had empty replacement text`
        );
    });

    // ----------------------------------------------------------------
    // 5. Document highlight works for symbol
    // ----------------------------------------------------------------
    test('document highlight works for symbol', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + 10_000);
        if (!basiliskBinary) {
            this.skip();
            return;
        }

        const source = [
            'def process(data: str) -> str:',
            '    return data.upper()',
            '',
            'output1: str = process("hello")',
            'output2: str = process("world")',
            '',
        ].join('\n');

        const { uri } = await openPythonFile(tmpDir, 'test_highlight.py', source);

        // Poll until the server returns document highlights.
        const defPosition = new vscode.Position(0, 4);
        const highlights = await pollUntilResult(
            () => vscode.commands.executeCommand<vscode.DocumentHighlight[]>(
                'vscode.executeDocumentHighlights', uri, defPosition
            ).then((r) => r, () => [] as vscode.DocumentHighlight[]),
            (r) => r !== null && r !== undefined && r.length >= 3
        );

        assert.ok(highlights, 'Expected document highlights to be defined');
        assert.ok(
            Array.isArray(highlights),
            'Expected document highlights to be an array'
        );
        assert.ok(
            highlights.length >= 3,
            `Expected at least 3 highlights (1 definition + 2 call sites), ` +
            `but got ${highlights.length}: ` +
            highlights.map((h) => `L${h.range.start.line}:${h.range.start.character}`).join(', ')
        );

        // Verify highlights span multiple lines.
        const highlightLines = new Set(highlights.map((h) => h.range.start.line));
        assert.ok(
            highlightLines.size >= 2,
            `Expected highlights on at least 2 different lines, ` +
            `but all highlights were on lines: ${[...highlightLines].join(', ')}`
        );
    });
});
