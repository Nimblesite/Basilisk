// Tests for [LSPARCH-FEATURES]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES
/**
 * LSP Feature Tests for the Basilisk VS Code Extension.
 *
 * These tests exercise additional LSP capabilities (find references,
 * rename, inlay hints, formatting, document highlights) through the
 * VS Code extension command APIs.
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
import {
    DIAGNOSTIC_TIMEOUT_MS,
    SUITE_SETUP_TIMEOUT_MS,
    pollUntilResult,
    findBasiliskBinary,
    openPythonFile,
    closeAllEditors,
    waitForLspReady,
} from './test-helpers';

/** Additional time (ms) added to DIAGNOSTIC_TIMEOUT_MS for individual test timeouts. */
const EXTRA_TEST_TIMEOUT_MS = 10_000;

/** Column position of the function name in a `def name(...)` declaration. */
const DEF_NAME_COLUMN = 4;

/** Minimum expected reference count: 1 definition + 2 call sites. */
const MIN_REFERENCE_COUNT = 3;

/** Line index of a call site (e.g. `result2 = compute(20)` on line 3). */
const CALL_SITE_LINE = 3;

/** Minimum expected inlay hint count for unannotated variables. */
const MIN_INLAY_HINT_COUNT = 2;

/** Tab size used for formatting requests. */
const FORMAT_TAB_SIZE = 4;

/** Minimum expected highlight count: 1 definition + 2 call sites. */
const MIN_HIGHLIGHT_COUNT = 3;

/** Minimum number of distinct lines expected for document highlights. */
const MIN_HIGHLIGHT_LINE_COUNT = 2;

/** Minimum expected rename edits: definition + at least 1 call site. */
const MIN_RENAME_EDIT_COUNT = 2;

// eslint-disable-next-line max-lines-per-function -- suite callback contains all tests
suite('LSP Feature Tests', () => {
    let tmpDir: string;
    let basiliskBinary: string | undefined;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);

        basiliskBinary = findBasiliskBinary();
        if (basiliskBinary === undefined) {
            throw new Error(
                'Basilisk binary not found. Build with: cargo build -p basilisk-cli'
            );
        }

        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-lsp-features-'));

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
    // 1. Find references works through extension
    // ----------------------------------------------------------------
    test('find references works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

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
        const defPosition = new vscode.Position(0, DEF_NAME_COLUMN);
        const locations = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.Location[]>(
                'vscode.executeReferenceProvider', uri, defPosition
            ).then((r) => r, () => [] as vscode.Location[]),
            predicate: (r) => r !== null && r !== undefined && r.length >= MIN_REFERENCE_COUNT,
        });

        assert.ok(locations !== undefined, 'Expected reference results to be defined');
        assert.ok(
            Array.isArray(locations),
            'Expected reference results to be an array'
        );
        assert.ok(
            locations.length >= MIN_REFERENCE_COUNT,
            `Expected at least ${MIN_REFERENCE_COUNT} references (1 definition + 2 call sites), ` +
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
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

        const source = [
            'def old_name(x: int) -> int:',
            '    return x + 1',
            '',
            'value: int = old_name(5)',
            '',
        ].join('\n');

        const { uri } = await openPythonFile(tmpDir, 'test_rename.py', source);

        // Poll until the server has indexed and returns rename results.
        const defPosition = new vscode.Position(0, DEF_NAME_COLUMN);
        const workspaceEdit = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.WorkspaceEdit>(
                'vscode.executeDocumentRenameProvider', uri, defPosition, 'new_name'
            ).then((r) => r, () => new vscode.WorkspaceEdit()),
            predicate: (r) => r !== null && r !== undefined && r.get(uri).length > 0,
        });

        assert.ok(workspaceEdit !== undefined, 'Expected workspace edit to be defined');

        // Get the text edits for our file.
        const edits = workspaceEdit.get(uri);
        assert.ok(
            edits.length > 0,
            `Expected at least one text edit for the renamed file, ` +
            `but got ${edits.length} edits`
        );

        // Verify that edits replace "old_name" with "new_name".
        const renameEdits = edits.filter(
            (edit) => edit.newText === 'new_name'
        );
        assert.ok(
            renameEdits.length >= MIN_RENAME_EDIT_COUNT,
            `Expected at least ${MIN_RENAME_EDIT_COUNT} rename edits (definition + call site), ` +
            `but got ${renameEdits.length}. All edits: ${
            edits.map((e) => `"${e.newText}" at L${e.range.start.line}:${e.range.start.character}`).join(', ')}`
        );

        // Verify the edits cover both the definition line and the call-site line.
        const editLines = new Set(renameEdits.map((edit) => edit.range.start.line));
        assert.ok(
            editLines.has(0),
            'Expected a rename edit on line 0 (function definition)'
        );
        assert.ok(
            editLines.has(CALL_SITE_LINE),
            `Expected a rename edit on line ${CALL_SITE_LINE} (call site)`
        );
    });

    // ----------------------------------------------------------------
    // 3. Inlay hints appear for unannotated variables
    // ----------------------------------------------------------------
    test('inlay hints appear for unannotated variables', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

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
        const hints = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.InlayHint[]>(
                'vscode.executeInlayHintProvider', uri, fullRange
            ).then((r) => r, () => [] as vscode.InlayHint[]),
            predicate: (r) => r !== null && r !== undefined && r.length >= MIN_INLAY_HINT_COUNT,
        });

        assert.ok(hints !== undefined, 'Expected inlay hints result to be defined');
        assert.ok(
            Array.isArray(hints),
            'Expected inlay hints result to be an array'
        );
        assert.ok(
            hints.length >= MIN_INLAY_HINT_COUNT,
            `Expected at least ${MIN_INLAY_HINT_COUNT} inlay hints for unannotated variables (x, y), ` +
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
            nonEmptyHints.length >= MIN_INLAY_HINT_COUNT,
            `Expected at least ${MIN_INLAY_HINT_COUNT} non-empty inlay hint labels, ` +
            `but got ${nonEmptyHints.length}`
        );
    });

    // ----------------------------------------------------------------
    // 4. Format document works through extension
    // ----------------------------------------------------------------
    test('format document works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

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
        const edits = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.TextEdit[]>(
                'vscode.executeFormatDocumentProvider', uri,
                { tabSize: FORMAT_TAB_SIZE, insertSpaces: true }
            ).then((r) => r, () => [] as vscode.TextEdit[]),
            predicate: (r) => r !== null && r !== undefined && r.length > 0,
        });

        assert.ok(edits !== undefined, 'Expected format edits to be defined');
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
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

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
        const defPosition = new vscode.Position(0, DEF_NAME_COLUMN);
        const highlights = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.DocumentHighlight[]>(
                'vscode.executeDocumentHighlights', uri, defPosition
            ).then((r) => r, () => [] as vscode.DocumentHighlight[]),
            predicate: (r) => r !== null && r !== undefined && r.length >= MIN_HIGHLIGHT_COUNT,
        });

        assert.ok(highlights !== undefined, 'Expected document highlights to be defined');
        assert.ok(
            Array.isArray(highlights),
            'Expected document highlights to be an array'
        );
        assert.ok(
            highlights.length >= MIN_HIGHLIGHT_COUNT,
            `Expected at least ${MIN_HIGHLIGHT_COUNT} highlights (1 definition + 2 call sites), ` +
            `but got ${highlights.length}: ${
            highlights.map((h) => `L${h.range.start.line}:${h.range.start.character}`).join(', ')}`
        );

        // Verify highlights span multiple lines.
        const highlightLines = new Set(highlights.map((h) => h.range.start.line));
        assert.ok(
            highlightLines.size >= MIN_HIGHLIGHT_LINE_COUNT,
            `Expected highlights on at least ${MIN_HIGHLIGHT_LINE_COUNT} different lines, ` +
            `but all highlights were on lines: ${[...highlightLines].join(', ')}`
        );
    });
});
