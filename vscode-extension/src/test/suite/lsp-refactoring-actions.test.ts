// Implements [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
/**
 * LSP Refactoring Code Action Tests for the Basilisk VS Code Extension.
 *
 * Tests that all refactoring code actions (extract, inline, convert,
 * move, change signature) are offered through the real LSP server.
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import {
    setupLspTestSuite,
    teardownLspTestSuite,
    openPythonFile,
    closeAllEditors,
    pollUntilResult,
} from "./test-helpers";

// eslint-disable-next-line max-lines-per-function
suite('LSP Refactoring Code Action Tests', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        const setup = await setupLspTestSuite('basilisk-refactor-actions-');
        tmpDir = setup.tmpDir;
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    // ── Extract Variable ────────────────────────────────────────────────

    test('extract variable code action is offered for expression selection', async function () {

        const source = 'result: int = some_func(42) + other_func(7)\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_extract_var.py', source);

        const range = new vscode.Range(
            new vscode.Position(0, 14),
            new vscode.Position(0, 27)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('Extract variable')),
            `Expected action containing 'Extract variable', got: ${titles.join(', ')}`
        );
    });

    // ── Extract Constant ────────────────────────────────────────────────

    test('extract constant code action is offered inside function', async function () {

        const source = 'import os\n\ndef f() -> int:\n    return 42\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_extract_const.py', source);

        const range = new vscode.Range(
            new vscode.Position(3, 11),
            new vscode.Position(3, 13)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('Extract constant')),
            `Expected action containing 'Extract constant', got: ${titles.join(', ')}`
        );
    });

    // ── Extract Function ────────────────────────────────────────────────

    test('extract function code action is offered for statement selection', async function () {

        const source = 'def main() -> None:\n    x: int = 1\n    y: int = x + 1\n    print(y)\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_extract_func.py', source);

        const range = new vscode.Range(
            new vscode.Position(1, 0),
            new vscode.Position(3, 0)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('Extract function')),
            `Expected action containing 'Extract function', got: ${titles.join(', ')}`
        );
    });

    // ── Inline Variable ─────────────────────────────────────────────────

    test('inline variable code action is offered', async function () {

        const source = 'def f() -> None:\n    temp = calculate()\n    result = temp + 1\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_inline_var.py', source);

        const range = new vscode.Range(
            new vscode.Position(1, 4),
            new vscode.Position(1, 4)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('Inline variable')),
            `Expected action containing 'Inline variable', got: ${titles.join(', ')}`
        );
    });

    // ── Inline Function ─────────────────────────────────────────────────

    test('inline function code action is offered', async function () {

        const source = 'def double(x: int) -> int:\n    return x * 2\n\nresult: int = double(5)\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_inline_func.py', source);

        const range = new vscode.Range(
            new vscode.Position(3, 14),
            new vscode.Position(3, 14)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('Inline function')),
            `Expected action containing 'Inline function', got: ${titles.join(', ')}`
        );
    });

    // ── Union Conversion ────────────────────────────────────────────────

    test('Union conversion code action is offered', async function () {

        const source = 'from typing import Union\nx: Union[int, str] = 1\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_union.py', source);

        const range = new vscode.Range(
            new vscode.Position(1, 3),
            new vscode.Position(1, 3)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('Union')),
            `Expected action containing 'Union', got: ${titles.join(', ')}`
        );
    });

    // ── Optional Conversion ─────────────────────────────────────────────

    test('Optional conversion code action is offered', async function () {

        const source = 'from typing import Optional\nx: Optional[int] = None\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_optional.py', source);

        const range = new vscode.Range(
            new vscode.Position(1, 3),
            new vscode.Position(1, 3)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('Optional')),
            `Expected action containing 'Optional', got: ${titles.join(', ')}`
        );
    });

    // ── f-string Conversion ─────────────────────────────────────────────

    test('f-string conversion code action is offered', async function () {

        const source = 'name: str = "world"\nx: str = f"hello {name}"\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_fstring.py', source);

        const range = new vscode.Range(
            new vscode.Position(1, 9),
            new vscode.Position(1, 9)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('.format()')),
            `Expected action containing '.format()', got: ${titles.join(', ')}`
        );
    });

    // ── dict Literal Conversion ─────────────────────────────────────────

    test('dict literal conversion code action is offered', async function () {

        const source = 'x: dict[str, int] = dict(a=1, b=2)\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_dict.py', source);

        const range = new vscode.Range(
            new vscode.Position(0, 20),
            new vscode.Position(0, 20)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('dict')),
            `Expected action containing 'dict', got: ${titles.join(', ')}`
        );
    });

    // ── list Literal Conversion ─────────────────────────────────────────

    test('list literal conversion code action is offered', async function () {

        const source = 'x: list[int] = list()\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_list.py', source);

        const range = new vscode.Range(
            new vscode.Position(0, 15),
            new vscode.Position(0, 15)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('list')),
            `Expected action containing 'list', got: ${titles.join(', ')}`
        );
    });

    // ── Ternary Conversion ──────────────────────────────────────────────

    test('ternary conversion code action is offered', async function () {

        const source = 'def f(cond: bool) -> int:\n    x: int = 1 if cond else 0\n    return x\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_ternary.py', source);

        const range = new vscode.Range(
            new vscode.Position(1, 4),
            new vscode.Position(1, 4)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('if/else')),
            `Expected action containing 'if/else', got: ${titles.join(', ')}`
        );
    });

    // ── Move Symbol ─────────────────────────────────────────────────────

    test('move symbol code action is offered for class', async function () {

        const source = 'import os\n\nclass MyWidget:\n    pass\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_move.py', source);

        const range = new vscode.Range(
            new vscode.Position(2, 0),
            new vscode.Position(2, 0)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('Move') && t.includes('new file')),
            `Expected action containing 'Move' and 'new file', got: ${titles.join(', ')}`
        );
    });

    // ── Change Signature ────────────────────────────────────────────────

    test('change signature remove parameter is offered', async function () {

        const source = 'def greet(name: str, greeting: str) -> str:\n    return f"{greeting}, {name}"\n\nresult: str = greet("world", "Hello")\n';
        const { uri } = await openPythonFile(tmpDir, 'refactor_change_sig.py', source);

        const range = new vscode.Range(
            new vscode.Position(0, 21),
            new vscode.Position(0, 21)
        );

        const actions = await pollUntilResult({
            fn: () => vscode.commands.executeCommand<vscode.CodeAction[]>(
                'vscode.executeCodeActionProvider', uri, range
            ).then(r => r ?? [], () => []),
            predicate: (r) => r.length > 0,
        });

        const titles = actions.map(a => a.title);
        assert.ok(
            titles.some(t => t.includes('Remove parameter')),
            `Expected action containing 'Remove parameter', got: ${titles.join(', ')}`
        );
    });
});
