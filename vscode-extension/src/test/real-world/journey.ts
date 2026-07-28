// Implements [VSIX-REALWORLD-JOURNEY]. See docs/specs/VSIX-REAL-WORLD-SPEC.md#VSIX-REALWORLD-JOURNEY
/**
 * The interaction engine for the real-world e2e suites: every phase a user
 * would drive by hand (open, hover, jump, complete, find references, edit,
 * search) executed against a pinned real-world repository, with a counted
 * assertion on every observable outcome. `check()` both asserts and counts,
 * so the suite can enforce a minimum assertion density at the end
 * ([VSIX-REALWORLD-JOURNEY] density floor).
 */

import { delay } from '../../timeouts';
import * as assert from 'assert';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { getStore } from '../../extension';
import {
    DIAGNOSTIC_TIMEOUT_MS,
    filterBasiliskDiagnostics,
    flattenSymbolNames,
    getDocumentSymbols,
    getHoverText,
    getNavLocations,
    pollUntilResult,
    replaceDocumentContent,
} from '../suite/test-helpers';
import {
    type CompletionProbe,
    type FileJourney,
    type RepoSpec,
    FETCH_MARKER,
    completionPosition,
    probePosition,
} from './corpus';
import { type ResourceMonitor } from './metrics';

/** How long the workspace-wide diagnostic set must hold still to be "settled". */
const DIAGNOSTIC_SETTLE_MS = 6_000;
/** Poll cadence while waiting for the diagnostic set to settle. */
const SETTLE_POLL_MS = 500;
/** Skip blitz files smaller than this — no symbols to assert on. */
const BLITZ_MIN_FILE_BYTES = 300;
/** Sample resources every N files during the open blitz. */
const BLITZ_SAMPLE_EVERY = 4;

let assertionCount = 0;

/** Counted assertion — the unit of the suite's assertion-density floor. */
export function check(condition: boolean, message: string): void {
    assertionCount += 1;
    assert.ok(condition, message);
}

/** Counted strict-equality assertion. */
export function checkEq<T>(actual: T, expected: T, message: string): void {
    assertionCount += 1;
    assert.strictEqual(actual, expected, message);
}

/** Total counted assertions executed so far in this test process. */
export function assertionTotal(): number {
    return assertionCount;
}

/** PID of the running basilisk LSP server (via the language client's child process). */
export function findServerPid(): number {
    const store = getStore();
    assert.ok(store !== undefined, 'extension store unavailable — extension not activated');
    const client = store.client.value;
    assert.ok(client !== undefined, 'LSP client not started');
    const internals = client as unknown as { _serverProcess?: { pid?: number } };
    const pid = internals._serverProcess?.pid;
    assert.ok(typeof pid === 'number' && pid > 0, 'basilisk server PID unavailable on the language client');
    return pid;
}

/** Recursively count `.py` files under `dir` (skipping dot-directories). */
function countPythonFiles(dir: string): number {
    let count = 0;
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        if (entry.name.startsWith('.')) { continue; }
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            count += countPythonFiles(full);
        } else if (entry.name.endsWith('.py')) {
            count += 1;
        }
    }
    return count;
}

/**
 * Assert the opened workspace IS the pinned corpus tree: right folder name,
 * fetch marker stamped with the pinned commit, sentinel present, and the
 * full tree on disk. Returns the workspace root path.
 */
export function verifyPinnedWorkspace(spec: RepoSpec): string {
    const folders = vscode.workspace.workspaceFolders ?? [];
    checkEq(folders.length, 1, 'real-world config must open exactly one workspace folder');
    const root = folders[0].uri.fsPath;
    checkEq(path.basename(root), spec.name, `workspace folder must be the ${spec.name} corpus checkout`);
    const marker = path.join(root, FETCH_MARKER);
    check(fs.existsSync(marker), `fetch marker missing — run scripts/fetch-real-world-repos.mjs (${marker})`);
    checkEq(
        fs.readFileSync(marker, 'utf8').trim(), spec.commit,
        `workspace tree is not pinned at ${spec.tag} (${spec.commit}) — re-run the fetch script`,
    );
    check(fs.existsSync(path.join(root, spec.sentinel)), `sentinel ${spec.sentinel} missing from workspace`);
    const pyFiles = countPythonFiles(root);
    check(
        pyFiles >= spec.minPythonFiles,
        `workspace holds ${pyFiles} .py files — expected at least ${spec.minPythonFiles}; tree is truncated`,
    );
    return root;
}

/** Open a repo-relative file in the editor and return its document. */
export async function openWorkspaceFile(root: string, relPath: string): Promise<vscode.TextDocument> {
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(root, relPath)));
    await vscode.window.showTextDocument(doc, { preview: false });
    checkEq(doc.languageId, 'python', `${relPath} must open as a Python document`);
    check(doc.lineCount > 1, `${relPath} must have content (${doc.lineCount} lines)`);
    return doc;
}

/** Workspace-wide basilisk diagnostics, keyed by file, inside `root` only. */
export function workspaceDiagnostics(root: string): [vscode.Uri, vscode.Diagnostic[]][] {
    return vscode.languages.getDiagnostics()
        .map(([uri, diags]): [vscode.Uri, vscode.Diagnostic[]] => [uri, filterBasiliskDiagnostics(diags)])
        .filter(([uri, diags]) => diags.length > 0 && uri.fsPath.startsWith(root));
}

/**
 * Wait for whole-workspace analysis to complete: the workspace symbol index
 * answers the first pinned query AND the diagnostic set holds still for
 * {@link DIAGNOSTIC_SETTLE_MS}. Returns the settled per-file snapshot.
 */
export async function waitForWorkspaceAnalysis(
    spec: RepoSpec,
    root: string,
): Promise<[vscode.Uri, vscode.Diagnostic[]][]> {
    const first = spec.workspaceSymbols[0];
    const symbols = await pollUntilResult({
        fn: async () => vscode.commands.executeCommand<vscode.SymbolInformation[]>(
            'vscode.executeWorkspaceSymbolProvider', first.query,
        ).then((r) => r ?? [], () => [] as vscode.SymbolInformation[]),
        predicate: (r) => r.some((s) => s.name === first.expectName),
        timeoutMs: spec.budgets.cpuSettleTimeoutMs,
        intervalMs: SETTLE_POLL_MS,
    }).catch(() => [] as vscode.SymbolInformation[]);
    check(
        symbols.some((s) => s.name === first.expectName),
        `workspace symbol index never answered "${first.query}" — analysis did not complete`,
    );

    // The server computes the whole scan BEFORE publishing anything, while
    // the symbol index answers incrementally mid-scan — so the diagnostic
    // set is deceptively "stable" at zero until the end-of-scan burst. An
    // empty snapshot therefore NEVER counts as settled: every corpus repo
    // is fetched without its dependencies, guaranteeing unresolved-import
    // diagnostics once the scan actually publishes.
    const deadline = Date.now() + spec.budgets.cpuSettleTimeoutMs;
    let lastShape = '';
    let stableSince = Date.now();
    while (Date.now() < deadline) {
        const snapshot = workspaceDiagnostics(root);
        const shape = `${snapshot.length}:${snapshot.reduce((n, [, d]) => n + d.length, 0)}`;
        if (shape !== lastShape) {
            lastShape = shape;
            stableSince = Date.now();
        } else if (snapshot.length > 0 && Date.now() - stableSince >= DIAGNOSTIC_SETTLE_MS) {
            return snapshot;
        }
        await delay(SETTLE_POLL_MS);
    }
    assert.fail(`workspace diagnostics never settled non-empty within ${spec.budgets.cpuSettleTimeoutMs}ms (last shape ${lastShape})`);
}

/** Structural invariants every published basilisk diagnostic must satisfy. */
export function assertDiagnosticInvariants(snapshot: readonly [vscode.Uri, vscode.Diagnostic[]][]): void {
    for (const [uri, diags] of snapshot) {
        const rel = vscode.workspace.asRelativePath(uri);
        check(
            uri.fsPath.endsWith('.py') || uri.fsPath.endsWith('.pyi'),
            `${rel}: basilisk diagnostics must only target Python files`,
        );
        for (const d of diags) {
            assertSingleDiagnosticInvariants(rel, d);
        }
    }
}

function assertSingleDiagnosticInvariants(rel: string, d: vscode.Diagnostic): void {
    check(d.message.trim().length > 0, `${rel}: diagnostic has an empty message`);
    check(d.range.start.line >= 0, `${rel}: diagnostic range starts before line 0`);
    check(
        d.range.end.isAfterOrEqual(d.range.start),
        `${rel}: diagnostic range ends (${d.range.end.line}:${d.range.end.character}) before it starts`,
    );
    check(
        d.severity >= vscode.DiagnosticSeverity.Error && d.severity <= vscode.DiagnosticSeverity.Hint,
        `${rel}: diagnostic severity ${d.severity} is not a valid DiagnosticSeverity`,
    );
    // PEP-rule codes are snake_case rule names; opt-in house rules are
    // BSK-XXXX. Both carry a docs link to their /errors/<code> page.
    if (typeof d.code === 'object') {
        check(String(d.code.value).trim().length > 0, `${rel}: diagnostic has an empty code value`);
        check(
            d.code.target.toString().includes('/errors/'),
            `${rel}: diagnostic docs link ${d.code.target.toString()} does not point at an /errors/ page`,
        );
    }
}

async function runHoverProbes(file: FileJourney, doc: vscode.TextDocument): Promise<void> {
    for (const probe of file.hovers) {
        const position = probePosition(doc.getText(), probe.token, probe.at);
        const hover = await getHoverText(doc.uri, position);
        check(hover.trim().length > 0, `${file.path}: no hover content at ${JSON.stringify(probe.at ?? probe.token)}`);
        for (const expected of probe.expect) {
            check(
                hover.includes(expected),
                `${file.path}: hover for ${JSON.stringify(probe.at ?? probe.token)} lacks ${JSON.stringify(expected)} — got: ${hover.slice(0, 200)}`,
            );
        }
    }
}

/** Platform-independent (forward-slash) form of a filesystem path. */
function slashed(fsPath: string): string {
    return fsPath.split('\\').join('/');
}

async function runDefinitionProbes(file: FileJourney, doc: vscode.TextDocument, root: string): Promise<void> {
    for (const probe of file.definitions) {
        const position = probePosition(doc.getText(), probe.token, probe.at);
        const locations = await getNavLocations('vscode.executeDefinitionProvider', doc.uri, position);
        check(locations.length > 0, `${file.path}: no definition for ${JSON.stringify(probe.at ?? probe.token)}`);
        const expectedPath = slashed(path.join(root, probe.expectFile));
        const hit = locations.find((l) => slashed(l.uri.fsPath) === expectedPath);
        check(
            hit !== undefined,
            `${file.path}: definition of ${JSON.stringify(probe.at ?? probe.token)} should land in ${probe.expectFile} — ` +
            `got ${locations.map((l) => vscode.workspace.asRelativePath(l.uri)).join(', ')}`,
        );
        if (hit !== undefined) {
            check(hit.range.start.line >= 0, `${file.path}: definition target range is invalid`);
            check(fs.existsSync(hit.uri.fsPath), `${file.path}: definition target ${hit.uri.fsPath} does not exist on disk`);
        }
    }
}

/** Normalise a completion item's label to its plain text. */
function completionLabel(item: vscode.CompletionItem): string {
    return typeof item.label === 'string' ? item.label : item.label.label;
}

async function runCompletionProbe(file: FileJourney, doc: vscode.TextDocument, probe: CompletionProbe): Promise<void> {
    const position = completionPosition(doc.getText(), probe);
    const list = await pollUntilResult({
        fn: async () => vscode.commands.executeCommand<vscode.CompletionList>(
            'vscode.executeCompletionItemProvider', doc.uri, position,
        ).then((r) => r ?? new vscode.CompletionList([]), () => new vscode.CompletionList([])),
        predicate: (r) => r.items.length > 0,
        timeoutMs: DIAGNOSTIC_TIMEOUT_MS,
    }).catch(() => new vscode.CompletionList([]));
    const labels = list.items.map(completionLabel);
    check(
        labels.length >= probe.expect.length,
        `${file.path}: completion after ${JSON.stringify(probe.afterDot)} returned ${labels.length} items — ` +
        `expected at least ${probe.expect.length}`,
    );
    for (const expected of probe.expect) {
        check(
            labels.includes(expected),
            `${file.path}: completion after ${JSON.stringify(probe.afterDot)} lacks ${JSON.stringify(expected)} — ` +
            `got: ${labels.slice(0, 15).join(', ')}…`,
        );
    }
    for (const item of list.items) {
        check(completionLabel(item).length > 0, `${file.path}: completion list contains an empty label`);
    }
}

async function runReferenceProbes(file: FileJourney, doc: vscode.TextDocument): Promise<void> {
    for (const probe of file.references) {
        const position = probePosition(doc.getText(), probe.token, probe.at);
        const locations = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.Location[]>(
                'vscode.executeReferenceProvider', doc.uri, position,
            ).then((r) => r ?? [], () => [] as vscode.Location[]),
            predicate: (r) => r.length >= probe.minLocations,
            timeoutMs: DIAGNOSTIC_TIMEOUT_MS,
        }).catch(() => [] as vscode.Location[]);
        check(
            locations.length >= probe.minLocations,
            `${file.path}: ${JSON.stringify(probe.at ?? probe.token)} has ${locations.length} references — ` +
            `expected at least ${probe.minLocations}`,
        );
        for (const loc of locations) {
            check(loc.range.start.line >= 0, `${file.path}: reference location has an invalid range`);
            check(fs.existsSync(loc.uri.fsPath), `${file.path}: reference target ${loc.uri.fsPath} does not exist`);
        }
    }
}

/** The full per-file interaction journey: symbols → hovers → defs → completions → refs. */
export async function runFileJourney(file: FileJourney, root: string, monitor: ResourceMonitor): Promise<void> {
    const doc = await openWorkspaceFile(root, file.path);
    const symbols = await getDocumentSymbols(doc.uri, (s) => s.length >= file.minDocumentSymbols);
    check(
        symbols.length >= file.minDocumentSymbols,
        `${file.path}: expected at least ${file.minDocumentSymbols} top-level symbols, got ${symbols.length}`,
    );
    const names = flattenSymbolNames(symbols);
    for (const expected of file.expectSymbols) {
        check(names.includes(expected), `${file.path}: document symbols lack ${JSON.stringify(expected)}`);
    }
    for (const name of names) {
        check(name.trim().length > 0, `${file.path}: document symbol with empty name`);
    }
    await runHoverProbes(file, doc);
    await runDefinitionProbes(file, doc, root);
    for (const probe of file.completions) {
        await runCompletionProbe(file, doc, probe);
    }
    await runReferenceProbes(file, doc);
    monitor.assertMemoryWithinBudget(`after journey ${file.path}`);
}

/** Workspace-symbol search must resolve every pinned query to its file. */
export async function runWorkspaceSymbolProbes(spec: RepoSpec, root: string): Promise<void> {
    for (const probe of spec.workspaceSymbols) {
        const results = await pollUntilResult({
            fn: async () => vscode.commands.executeCommand<vscode.SymbolInformation[]>(
                'vscode.executeWorkspaceSymbolProvider', probe.query,
            ).then((r) => r ?? [], () => [] as vscode.SymbolInformation[]),
            predicate: (r) => r.length > 0,
            timeoutMs: DIAGNOSTIC_TIMEOUT_MS,
        }).catch(() => [] as vscode.SymbolInformation[]);
        check(results.length > 0, `workspace symbols: no results for "${probe.query}"`);
        const expectedPath = slashed(path.join(root, probe.expectFile));
        check(
            results.some((s) => s.name === probe.expectName && slashed(s.location.uri.fsPath) === expectedPath),
            `workspace symbols: "${probe.query}" should surface ${probe.expectName} in ${probe.expectFile} — ` +
            `got ${results.slice(0, 10).map((s) => `${s.name}@${vscode.workspace.asRelativePath(s.location.uri)}`).join(', ')}`,
        );
        for (const s of results) {
            check(s.name.length > 0, `workspace symbols: empty symbol name for query "${probe.query}"`);
        }
    }
}

/**
 * Open-many soak: open `count` real files back to back the way a user
 * riffles through a codebase, asserting symbols on each and sampling
 * resources as it goes. Feeds the leak assertion that follows it.
 */
export async function runOpenBlitz(spec: RepoSpec, root: string, monitor: ResourceMonitor): Promise<void> {
    const dir = path.join(root, spec.openBlitz.dir);
    const files = fs.readdirSync(dir)
        .filter((f) => f.endsWith('.py') && fs.statSync(path.join(dir, f)).size > BLITZ_MIN_FILE_BYTES)
        .sort()
        .slice(0, spec.openBlitz.count);
    check(
        files.length >= Math.min(spec.openBlitz.count, 8),
        `open blitz found only ${files.length} candidate files in ${spec.openBlitz.dir}`,
    );
    for (const [index, name] of files.entries()) {
        const doc = await openWorkspaceFile(root, `${spec.openBlitz.dir}/${name}`);
        const symbols = await getDocumentSymbols(doc.uri);
        check(symbols.length > 0, `open blitz: ${name} produced no document symbols`);
        if ((index + 1) % BLITZ_SAMPLE_EVERY === 0) {
            monitor.assertMemoryWithinBudget(`open blitz after ${index + 1} files`);
        }
    }
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
}

/**
 * The erroneous probe function appended during edit churn (cycle-unique).
 * Returns an int literal against a declared `-> str` — flagged by
 * `returns_compatibility` (def line) and `returns_compatibility_2`
 * (return line). A literal is used deliberately: returning a mistyped
 * *parameter* is not currently flagged by the checker.
 */
function churnProbeText(cycle: number): string {
    return `\n\ndef _bsk_realworld_probe_${cycle}() -> str:\n    return ${cycle}\n`;
}

/**
 * An unsaved edit re-triggers analysis of the whole workspace in
 * wholeModule mode, so a churn diagnostic can take a full re-analysis pass
 * (~15s on the flask corpus) to arrive — give it analysis-scale time.
 * Exported so the churn test's mocha timeout covers every sanctioned poll.
 */
export const CHURN_DIAGNOSTIC_TIMEOUT_MS = 45_000;

async function appendAndExpectError(doc: vscode.TextDocument, cycle: number, relPath: string): Promise<void> {
    const probeText = churnProbeText(cycle);
    const newText = doc.getText() + probeText;
    check(await replaceDocumentContent(doc, newText), `${relPath}: churn edit ${cycle} failed to apply`);
    // The bad-return error may anchor on the `return` line or the signature
    // line depending on the rule — any line inside the appended probe counts.
    const probeStartLine = newText.slice(0, newText.indexOf(probeText)).split('\n').length;
    function inProbe(x: vscode.Diagnostic): boolean {
        return x.range.start.line >= probeStartLine;
    }
    const diags = await pollUntilResult({
        fn: async () => filterBasiliskDiagnostics(vscode.languages.getDiagnostics(doc.uri)),
        predicate: (d) => d.some(inProbe),
        timeoutMs: CHURN_DIAGNOSTIC_TIMEOUT_MS,
    }).catch(() => [] as vscode.Diagnostic[]);
    const hit = diags.find(inProbe);
    check(hit !== undefined, `${relPath}: churn cycle ${cycle} — no diagnostic on the appended bad return (lines >= ${probeStartLine})`);
    if (hit !== undefined) {
        checkEq(hit.severity, vscode.DiagnosticSeverity.Error, `${relPath}: bad return must be an Error`);
        check(hit.message.length > 10, `${relPath}: churn diagnostic message is too thin: "${hit.message}"`);
        assertSingleDiagnosticInvariants(relPath, hit);
    }
}

/**
 * Edit churn: repeatedly introduce a guaranteed type error, assert the
 * diagnostic arrives on the exact line, revert, and assert the diagnostic
 * set returns to its baseline — live analysis, no stale leftovers.
 */
export async function runEditChurn(spec: RepoSpec, root: string): Promise<void> {
    const relPath = spec.editChurn.path;
    const doc = await openWorkspaceFile(root, relPath);
    const original = doc.getText();
    const baseline = filterBasiliskDiagnostics(vscode.languages.getDiagnostics(doc.uri)).length;
    for (let cycle = 0; cycle < spec.editChurn.cycles; cycle++) {
        await appendAndExpectError(doc, cycle, relPath);
        check(await replaceDocumentContent(doc, original), `${relPath}: churn revert ${cycle} failed to apply`);
        const after = await pollUntilResult({
            fn: async () => filterBasiliskDiagnostics(vscode.languages.getDiagnostics(doc.uri)),
            predicate: (d) => d.length === baseline,
            timeoutMs: CHURN_DIAGNOSTIC_TIMEOUT_MS,
        }).catch(() => filterBasiliskDiagnostics(vscode.languages.getDiagnostics(doc.uri)));
        checkEq(
            after.length, baseline,
            `${relPath}: cycle ${cycle} — diagnostics did not return to baseline after revert`,
        );
        checkEq(doc.getText(), original, `${relPath}: cycle ${cycle} — document text not restored`);
    }
    await vscode.commands.executeCommand('workbench.action.files.revert');
    checkEq(doc.isDirty, false, `${relPath}: document left dirty after churn`);
}
