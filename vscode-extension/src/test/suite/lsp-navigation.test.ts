// Implements [LSPARCH-FEATURES-DEFINITION]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-DEFINITION
/**
 * LSP Navigation & Code Action Tests for the Basilisk VS Code Extension.
 *
 * Tests go-to-definition, go-to-declaration, go-to-type-definition,
 * signature help, code actions, and hover-docstring through VS Code's
 * command APIs against the real LSP server.
 *
 * Extracted from lsp-integration.test.ts to keep files under the 500-line limit.
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 *   - The binary must be on PATH or the test will fail hard
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import {
    DIAGNOSTIC_TIMEOUT_MS,
    SUITE_SETUP_TIMEOUT_MS,
    closeAllEditors,
    openPythonFile,
    pollUntilResult,
    setupLspTestSuite,
    teardownLspTestSuite,
    waitForDiagnostics,
} from './test-helpers';

/** Extra buffer (ms) added to test-level timeouts beyond the core wait. */
const TIMEOUT_BUFFER_MS = 5_000;

/** Large buffer (ms) for tests that involve multiple operations. */
const LARGE_TIMEOUT_BUFFER_MS = 10_000;

// ── Test-specific line/column positions ──────────────────────────────

/** Line containing the function call for go-to-definition / declaration. */
const GOTO_CALL_LINE = 3;

/** Column of the function name in the call expression. */
const GOTO_CALL_COLUMN = 18;

/** Line of the variable for go-to-type-definition. */
const TYPE_DEF_VAR_LINE = 3;

/** Column of the variable for go-to-type-definition. */
const TYPE_DEF_VAR_COLUMN = 2;

/** Line for signature help trigger ("greet()"). */
const SIG_HELP_LINE = 3;

/** Column inside the parentheses for signature help. */
const SIG_HELP_COLUMN = 6;

/** Line of the function definition for hover-docstring test. */
const HOVER_DOCSTRING_LINE = 0;

/** Column of the function name in the def line. */
const HOVER_DOCSTRING_COLUMN = 5;

/**
 * Extract hover text content from hover results.
 */
function extractHoverText(hovers: vscode.Hover[]): string {
    return hovers
        .flatMap((h) =>
            h.contents.map((c) => {
                if (typeof c === 'string') {return c;}
                if (c instanceof vscode.MarkdownString) {return c.value;}
                if ('value' in c) {return (c as { value: string }).value;}
                return '';
            })
        )
        .join('\n');
}

// eslint-disable-next-line max-lines-per-function
suite('LSP Navigation & Code Action Tests', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const setup = await setupLspTestSuite('basilisk-nav-test-');
        tmpDir = setup.tmpDir;
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        teardownLspTestSuite(tmpDir);
    });

    teardown(async () => {
        await closeAllEditors();
    });

    // ----------------------------------------------------------------
    // Go-to-definition works through extension
    // ----------------------------------------------------------------
    test('go-to-definition works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_BUFFER_MS);

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
        const callPosition = new vscode.Position(GOTO_CALL_LINE, GOTO_CALL_COLUMN);
        const locations = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.Location[]>(
                'vscode.executeDefinitionProvider', uri, callPosition
            ).then((r) => r, () => [] as vscode.Location[]),
            predicate: (r) => r !== null && r !== undefined && r.length > 0,
        });

        assert.ok(locations !== undefined, 'Expected definition locations to be defined');
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
    // Signature help works through extension
    // ----------------------------------------------------------------
    test('signature help works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_BUFFER_MS);

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
        const position = new vscode.Position(SIG_HELP_LINE, SIG_HELP_COLUMN);
        const signatureHelp = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.SignatureHelp>(
                'vscode.executeSignatureHelpProvider', uri, position, '('
            ).then((r) => r, () => null),
            predicate: (r) => r !== null && r !== undefined && r.signatures.length > 0,
        });

        assert.ok(signatureHelp !== null && signatureHelp !== undefined, 'Expected signature help result to be defined');
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
    // Code actions provided for diagnostics
    // ----------------------------------------------------------------
    test('code actions provided for diagnostics', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + LARGE_TIMEOUT_BUFFER_MS);

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

        assert.ok(codeActions !== undefined, 'Expected code actions result to be defined');
        assert.ok(
            codeActions.length > 0,
            `Expected at least one code action for the diagnostic. ` +
            `Diagnostic: ${diagnostics[0].message}`
        );

        // Verify the code action has a title (i.e. is well-formed).
        const firstAction = codeActions[0];
        assert.ok(
            firstAction.title.length > 0,
            `Expected code action to have a non-empty title, got: "${firstAction.title}"`
        );
    });

    // ----------------------------------------------------------------
    // Go-to-declaration works through extension
    // ----------------------------------------------------------------
    test('go-to-declaration works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_BUFFER_MS);

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
        const callPosition = new vscode.Position(GOTO_CALL_LINE, GOTO_CALL_COLUMN);
        const locations = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.Location[]>(
                'vscode.executeDeclarationProvider', uri, callPosition
            ).then((r) => r, () => [] as vscode.Location[]),
            predicate: (r) => r !== null && r !== undefined && r.length > 0,
        });

        assert.ok(locations !== undefined, 'Expected declaration locations to be defined');
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
    // Go-to-type-definition works through extension
    // ----------------------------------------------------------------
    test('go-to-type-definition works through extension', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_BUFFER_MS);

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
        const varPosition = new vscode.Position(TYPE_DEF_VAR_LINE, TYPE_DEF_VAR_COLUMN);
        const locations = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.Location[]>(
                'vscode.executeTypeDefinitionProvider', uri, varPosition
            ).then((r) => r, () => [] as vscode.Location[]),
            predicate: (r) => r !== null && r !== undefined && r.length > 0,
        });

        assert.ok(locations !== undefined, 'Expected type definition locations to be defined');
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
    // Hover shows docstrings
    // ----------------------------------------------------------------
    test('hover shows docstring for function', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TIMEOUT_BUFFER_MS);

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
        const position = new vscode.Position(HOVER_DOCSTRING_LINE, HOVER_DOCSTRING_COLUMN);
        const hovers = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.Hover[]>(
                'vscode.executeHoverProvider', uri, position
            ).then((r) => r, () => [] as vscode.Hover[]),
            predicate: (r) => r !== null && r !== undefined && r.length > 0,
        });

        assert.ok(hovers !== undefined, 'Expected hover result to be defined');
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
