// Implements [VSIX-OUTPUT-CHANNELS]. See docs/specs/VSIX-SPEC.md#VSIX-OUTPUT-CHANNELS
/**
 * LSP trace channel tests (GitHub #201).
 *
 * The "Basilisk LSP Trace" output channel is the only field observability for
 * LSP request/response traffic. #201 reported it entirely blank: setting the
 * documented `basilisk.trace.server` switch produced zero output, leaving
 * failures undiagnosable. These tests drive the real LSP pipeline and assert
 * what users actually see through the `lspTraceLines()` seam
 * (src/lsp-trace.ts — VS Code offers no API to read a channel back).
 */

import { delay } from '../../timeouts';
import * as assert from 'assert';
import * as vscode from 'vscode';
import { lspTraceLines } from '../../lsp-trace';

import {
    DIAGNOSTIC_TIMEOUT_MS,
    SUITE_SETUP_TIMEOUT_MS,
    openPythonFile,
    closeAllEditors,
    setupLspTestSuite,
    teardownLspTestSuite,
} from './test-helpers';

/** How long trace lines get to land after tracing is switched on. */
const TRACE_WAIT_MS = 15_000;

/** Poll interval while waiting for trace lines. */
const TRACE_POLL_MS = 100;

/** A trace line naming an LSP document method — proof of per-request tracing. */
const LSP_METHOD_RE = /textDocument\//;

/** Wait until a recorded trace line matches `pattern`, or time out. */
async function waitForTraceLine(
    pattern: RegExp,
    timeoutMs: number
): Promise<string | undefined> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        const line = lspTraceLines().find((candidate) => pattern.test(candidate));
        if (line !== undefined) {
            return line;
        }
        await delay(TRACE_POLL_MS);
    }
    return undefined;
}

suite('LSP Trace Channel Tests', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        const result = await setupLspTestSuite('basilisk-trace-test-');
        tmpDir = result.tmpDir;
    });

    suiteTeardown(async () => {
        await vscode.workspace
            .getConfiguration('basilisk')
            .update('trace.server', undefined, vscode.ConfigurationTarget.Global);
        await closeAllEditors();
        teardownLspTestSuite(tmpDir);
    });

    // ----------------------------------------------------------------
    // GitHub #201: with basilisk.trace.server enabled, real LSP traffic
    // (didOpen, hover) must surface in the trace channel. [VSIX-OUTPUT-CHANNELS]
    // ----------------------------------------------------------------
    test('enabling basilisk.trace.server surfaces LSP requests in the trace channel', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + TRACE_WAIT_MS * 2);

        await vscode.workspace
            .getConfiguration('basilisk')
            .update('trace.server', 'verbose', vscode.ConfigurationTarget.Global);

        // Drive genuine LSP traffic through the live server: didOpen from the
        // editor, then an explicit hover request.
        const { doc, uri } = await openPythonFile(
            tmpDir,
            'trace_probe.py',
            'def greet(name: str) -> str:\n' +
            '    return name\n' +
            '\n' +
            '\n' +
            'result = greet("world")\n'
        );
        assert.strictEqual(doc.languageId, 'python', 'probe file must open as python');
        await vscode.commands.executeCommand<vscode.Hover[]>(
            'vscode.executeHoverProvider',
            uri,
            new vscode.Position(4, 10)
        );

        const line = await waitForTraceLine(LSP_METHOD_RE, TRACE_WAIT_MS);
        assert.notStrictEqual(
            line,
            undefined,
            'basilisk.trace.server=verbose must surface textDocument/* traffic in the ' +
            `"Basilisk LSP Trace" channel; the channel stayed blank (#201) — ` +
            `${lspTraceLines().length} line(s) recorded`
        );
    });
});
