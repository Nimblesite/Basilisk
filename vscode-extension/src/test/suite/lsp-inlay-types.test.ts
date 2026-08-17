// Tests for [LSPARCH-FEATURES-INLAYHINTS].
// See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-INLAYHINTS and the
// implementation in crates/basilisk-lsp/src/inlay_hints.rs.
/**
 * INLINE TYPE VISIBILITY — end-to-end guardrail suite.
 *
 * Basilisk surfaces inferred types INLINE (the `int` / `bool` / `str`
 * "bubbles" a user sees next to a name) via `textDocument/inlayHint`. The whole
 * point is that a developer reads the type of a symbol WITHOUT hovering the
 * mouse over it. This suite pins that behaviour down hard: it drives the real
 * VS Code inlay-hint provider (which round-trips through the running LSP) across
 * many variables and every inferable builtin type, and asserts the exact inline
 * type label appears on the exact line of each symbol.
 *
 * A regression that stops inline types from rendering — or renders the wrong
 * type, or double-renders over an already-annotated symbol — fails here.
 *
 * Prerequisites:
 *   - The `basilisk` binary must be built: `cargo build -p basilisk-cli`
 */

import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import {
    closeAllEditors,
    DIAGNOSTIC_TIMEOUT_MS,
    getInlayHints,
    inlayLabelsOnLine,
    locate,
    normalizedInlayLabel,
    openPythonFile,
    removeTestDir,
    replaceDocumentContent,
    SUITE_SETUP_TIMEOUT_MS,
    waitForInlayLabel,
    waitForLspReady,
} from './test-helpers';

/** Additional time (ms) added to DIAGNOSTIC_TIMEOUT_MS for individual test timeouts. */
const EXTRA_TEST_TIMEOUT_MS = 10_000;

/** A `[variable-name, expected normalised inline type]` expectation. */
type InlineTypeCase = [string, string];

/** Assert an inline `:type` hint (normalised) sits on the variable's own line. */
function assertInlineTypeOnVar(
    hints: readonly vscode.InlayHint[],
    source: string,
    testCase: InlineTypeCase,
): void {
    const [varName, expected] = testCase;
    const line = locate(source, varName).line;
    const labels = inlayLabelsOnLine(hints, line);
    assert.ok(
        labels.includes(expected),
        `Expected inline type "${expected}" on "${varName}" (line ${line}) ` +
        `without hovering — got ${JSON.stringify(labels)}`,
    );
}

/** Assert NO inline type hint is rendered on `varName`'s line (e.g. already annotated). */
function assertNoInlineTypeOnVar(
    hints: readonly vscode.InlayHint[],
    source: string,
    varName: string,
): void {
    const line = locate(source, varName).line;
    const typeLabels = inlayLabelsOnLine(hints, line).filter((l) => l.startsWith(':'));
    assert.deepStrictEqual(
        typeLabels,
        [],
        `Expected NO inline type hint on already-annotated "${varName}" (line ${line}) ` +
        `— got ${JSON.stringify(typeLabels)}`,
    );
}

// eslint-disable-next-line max-lines-per-function -- suite callback contains all tests
suite('Inline Type Visibility (inlay hints)', () => {
    let tmpDir: string;

    suiteSetup(async function () {
        this.timeout(SUITE_SETUP_TIMEOUT_MS);
        tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'basilisk-inlay-types-'));
        await waitForLspReady();
        await closeAllEditors();
    });

    suiteTeardown(async () => {
        await closeAllEditors();
        if (tmpDir !== undefined && tmpDir !== '' && fs.existsSync(tmpDir)) {
            removeTestDir(tmpDir);
        }
    });

    teardown(async () => {
        await closeAllEditors();
    });

    // ----------------------------------------------------------------
    // 1. Every inferable builtin type is shown inline on its variable.
    // ----------------------------------------------------------------
    test('module-level variables show an inline type for every builtin kind', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

        const source = [
            'an_int = 42',
            'a_float = 3.14',
            'a_str = "hello"',
            'yes_flag = True',
            'no_flag = False',
            'some_bytes = b"data"',
            'nothing = None',
            'numbers = [1, 2, 3]',
            'mapping = {"k": "v"}',
            'uniques = {1, 2, 3}',
            'pair = (1, 2)',
            '',
        ].join('\n');

        const { doc } = await openPythonFile(tmpDir, 'inlay_all_types.py', source);
        const cases: InlineTypeCase[] = [
            ['an_int', ':int'],
            ['a_float', ':float'],
            ['a_str', ':str'],
            ['yes_flag', ':bool'],
            ['no_flag', ':bool'],
            ['some_bytes', ':bytes'],
            ['nothing', ':None'],
            // Container literals surface their inferred generic args (#290).
            ['numbers', ':list[int]'],
            ['mapping', ':dict[str,str]'],
            ['uniques', ':set[int]'],
            ['pair', ':tuple[int,int]'],
        ];

        const hints = await getInlayHints(doc, cases.length);
        assert.ok(
            hints.length >= cases.length,
            `Expected at least ${cases.length} inline type hints (one per variable), ` +
            `got ${hints.length}`,
        );
        cases.forEach((testCase) => assertInlineTypeOnVar(hints, source, testCase));
    });

    // ----------------------------------------------------------------
    // 2. Inline type hints are rendered as TYPE hints (the "bubbles"),
    //    never as parameter hints.
    // ----------------------------------------------------------------
    test('inline type hints carry InlayHintKind.Type', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

        const source = [
            'width = 1920',
            'height = 1080',
            'title = "screen"',
            'visible = True',
            '',
        ].join('\n');

        const { doc } = await openPythonFile(tmpDir, 'inlay_kind.py', source);
        const hints = await getInlayHints(doc, 4);

        const typeHints = hints.filter((h) => normalizedInlayLabel(h).startsWith(':'));
        assert.ok(
            typeHints.length >= 4,
            `Expected at least 4 inline type hints, got ${typeHints.length}`,
        );
        typeHints.forEach((h) =>
            assert.strictEqual(
                h.kind,
                vscode.InlayHintKind.Type,
                `Inline type hint ${JSON.stringify(normalizedInlayLabel(h))} must be ` +
                `InlayHintKind.Type, was ${String(h.kind)}`,
            ),
        );
    });

    // ----------------------------------------------------------------
    // 3. Function-local unannotated variables also show inline types.
    // ----------------------------------------------------------------
    test('function-local variables show inline types', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

        const source = [
            'def compute():',
            '    total = 0',
            '    label = "sum"',
            '    ratio = 2.5',
            '    active = True',
            '    blob = b"x"',
            '',
        ].join('\n');

        const { doc } = await openPythonFile(tmpDir, 'inlay_locals.py', source);
        const hints = await getInlayHints(doc, 5);

        assertInlineTypeOnVar(hints, source, ['total', ':int']);
        assertInlineTypeOnVar(hints, source, ['label', ':str']);
        assertInlineTypeOnVar(hints, source, ['ratio', ':float']);
        assertInlineTypeOnVar(hints, source, ['active', ':bool']);
        assertInlineTypeOnVar(hints, source, ['blob', ':bytes']);
    });

    // ----------------------------------------------------------------
    // 4. The real screenshot scenario: annotated symbols keep their
    //    source type (no duplicate hint); the gaps get filled inline.
    // ----------------------------------------------------------------
    test('annotated symbols are not double-typed; unannotated neighbours are filled', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

        const source = [
            'timeout: int = 100',
            'one_flag: bool = True',
            'other_flag: bool = False',
            'retries = 5',
            'label = "ready"',
            '',
        ].join('\n');

        const { doc } = await openPythonFile(tmpDir, 'inlay_mixed.py', source);
        const hints = await getInlayHints(doc, 2);

        // Explicitly-annotated symbols already show their type in source — no hint.
        assertNoInlineTypeOnVar(hints, source, 'timeout');
        assertNoInlineTypeOnVar(hints, source, 'one_flag');
        assertNoInlineTypeOnVar(hints, source, 'other_flag');

        // The unannotated neighbours DO get an inline type.
        assertInlineTypeOnVar(hints, source, ['retries', ':int']);
        assertInlineTypeOnVar(hints, source, ['label', ':str']);
    });

    // ----------------------------------------------------------------
    // 5. Functions without a return annotation show an inline "-> type".
    // ----------------------------------------------------------------
    test('functions show an inline return type without hovering', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

        const source = [
            'def get_count():',
            '    return 42',
            '',
            'def get_name():',
            '    return "hello"',
            '',
            'def do_nothing():',
            '    pass',
            '',
        ].join('\n');

        const { doc } = await openPythonFile(tmpDir, 'inlay_returns.py', source);
        const hints = await getInlayHints(doc, 3);

        function returnLabelsOn(needle: string): string[] {
            return inlayLabelsOnLine(hints, locate(source, needle).line);
        }

        assert.ok(
            returnLabelsOn('def get_count').includes('->int'),
            `Expected inline "-> int" on get_count — got ${JSON.stringify(returnLabelsOn('def get_count'))}`,
        );
        assert.ok(
            returnLabelsOn('def get_name').includes('->str'),
            `Expected inline "-> str" on get_name — got ${JSON.stringify(returnLabelsOn('def get_name'))}`,
        );
        assert.ok(
            returnLabelsOn('def do_nothing').includes('->None'),
            `Expected inline "-> None" on do_nothing — got ${JSON.stringify(returnLabelsOn('def do_nothing'))}`,
        );
    });

    // ----------------------------------------------------------------
    // 6. Call sites show which parameter each argument binds to, inline.
    // ----------------------------------------------------------------
    test('call sites show inline parameter-name hints', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

        const source = [
            'def greet(name, count):',
            '    return count',
            '',
            'greet("bob", 3)',
            '',
        ].join('\n');

        const { doc } = await openPythonFile(tmpDir, 'inlay_params.py', source);
        const hints = await getInlayHints(doc, 2);

        const callLine = locate(source, 'greet("bob"').line;
        const callLabels = inlayLabelsOnLine(hints, callLine);
        assert.ok(
            callLabels.includes('name='),
            `Expected inline "name=" hint at the call site — got ${JSON.stringify(callLabels)}`,
        );
        assert.ok(
            callLabels.includes('count='),
            `Expected inline "count=" hint at the call site — got ${JSON.stringify(callLabels)}`,
        );

        const paramHints = hints.filter((h) => normalizedInlayLabel(h).endsWith('='));
        assert.ok(paramHints.length >= 2, `Expected at least 2 parameter-name hints, got ${paramHints.length}`);
        paramHints.forEach((h) =>
            assert.strictEqual(
                h.kind,
                vscode.InlayHintKind.Parameter,
                `Parameter-name hint ${JSON.stringify(normalizedInlayLabel(h))} must be ` +
                `InlayHintKind.Parameter, was ${String(h.kind)}`,
            ),
        );
    });

    // ----------------------------------------------------------------
    // 7. Inline types are LIVE — they follow the value as it changes,
    //    never showing a stale type.
    // ----------------------------------------------------------------
    test('inline type stays correct as the value changes', async function () {
        this.timeout((DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS) * 2);

        const { doc } = await openPythonFile(tmpDir, 'inlay_live.py', 'value = 1\n');

        const initial = await waitForInlayLabel({ doc, line: 0, label: ':int' });
        assert.ok(
            inlayLabelsOnLine(initial, 0).includes(':int'),
            `Expected inline ":int" for an int literal — got ${JSON.stringify(inlayLabelsOnLine(initial, 0))}`,
        );

        assert.ok(await replaceDocumentContent(doc, 'value = "text"\n'), 'edit to str should apply');
        const asStr = await waitForInlayLabel({ doc, line: 0, label: ':str' });
        assert.ok(
            inlayLabelsOnLine(asStr, 0).includes(':str'),
            `Expected inline ":str" after reassigning to a string — got ${JSON.stringify(inlayLabelsOnLine(asStr, 0))}`,
        );

        assert.ok(await replaceDocumentContent(doc, 'value = [1, 2, 3]\n'), 'edit to list should apply');
        const asList = await waitForInlayLabel({ doc, line: 0, label: ':list[int]' });
        assert.ok(
            inlayLabelsOnLine(asList, 0).includes(':list[int]'),
            `Expected inline ":list[int]" after reassigning to a list — got ${JSON.stringify(inlayLabelsOnLine(asList, 0))}`,
        );
    });

    // ----------------------------------------------------------------
    // 8. A dense module surfaces an inline type for a LOT of variables.
    // ----------------------------------------------------------------
    test('a dense module surfaces an inline type for many variables at once', async function () {
        this.timeout(DIAGNOSTIC_TIMEOUT_MS + EXTRA_TEST_TIMEOUT_MS);

        const source = [
            'port = 8080',
            'host = "localhost"',
            'debug = True',
            'quiet = False',
            'threshold = 0.75',
            'payload = b"\\x00"',
            'tags = ["a", "b"]',
            'headers = {"accept": "json"}',
            'seen = {1, 2}',
            'coords = (0, 0)',
            'placeholder = None',
            'retries = 3',
            '',
        ].join('\n');

        const { doc } = await openPythonFile(tmpDir, 'inlay_dense.py', source);
        const expected: InlineTypeCase[] = [
            ['port', ':int'],
            ['host', ':str'],
            ['debug', ':bool'],
            ['quiet', ':bool'],
            ['threshold', ':float'],
            ['payload', ':bytes'],
            // Container literals surface their inferred generic args (#290).
            ['tags', ':list[str]'],
            ['headers', ':dict[str,str]'],
            ['seen', ':set[int]'],
            ['coords', ':tuple[int,int]'],
            ['placeholder', ':None'],
            ['retries', ':int'],
        ];

        const hints = await getInlayHints(doc, expected.length);
        const typeHintCount = hints.filter((h) => normalizedInlayLabel(h).startsWith(':')).length;
        assert.ok(
            typeHintCount >= expected.length,
            `Expected inline types for all ${expected.length} variables, got ${typeHintCount}`,
        );
        expected.forEach((testCase) => assertInlineTypeOnVar(hints, source, testCase));
    });
});
