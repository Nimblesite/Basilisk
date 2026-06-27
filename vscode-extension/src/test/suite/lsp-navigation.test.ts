// Implements [LSPARCH-FEATURES-SIGNATURE-HELP]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-SIGNATURE-HELP
/**
 * LSP Signature-Help & Code-Action Tests for the Basilisk VS Code Extension.
 *
 * Tests signature help and code actions through VS Code's command APIs against
 * the real LSP server.
 *
 * Hover and go-to-definition/-declaration/-type-definition are hammered
 * exhaustively in their own dedicated suites (lsp-hover.test.ts,
 * lsp-goto.test.ts) — they are NOT duplicated here.
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

/** Line for signature help trigger ("greet()"). */
const SIG_HELP_LINE = 3;

/** Column inside the parentheses for signature help. */
const SIG_HELP_COLUMN = 6;

suite('LSP Signature Help & Code Action Tests', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        // This suite's code-action test relies on an annotation diagnostic
        // (untyped param), which is an opt-in house rule — enable it here.
        const setup = await setupLspTestSuite('basilisk-nav-test-', { strictAnnotations: true });
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
});
