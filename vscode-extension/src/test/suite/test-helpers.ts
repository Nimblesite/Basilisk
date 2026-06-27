// Implements [VSIX]. See docs/specs/VSIX-SPEC.md#VSIX
/**
 * Shared test helpers for Basilisk VS Code extension E2E tests.
 *
 * Centralises LSP test utilities that were previously duplicated across
 * every test file: binary discovery, diagnostic polling, file management.
 */

import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';


export { POLL_INTERVAL_MS, WAIT_MS } from '../../timeouts';

export const EXTENSION_ID = 'Nimblesite.basilisk';

/** Maximum time (ms) to wait for diagnostics from the LSP server. */
export const DIAGNOSTIC_TIMEOUT_MS = 15_000;

/** Time (ms) to wait for "no diagnostics" assertions. */
export const NO_DIAGNOSTIC_WAIT_MS = 5_000;

/** Time (ms) to wait for the LSP server to fully start.
 *  CI runners need up to 2 minutes for a cold start (cargo build + LSP init). */
export const SERVER_START_WAIT_MS = 60_000;

/** Mocha timeout (ms) for suiteSetup hooks that wait for the LSP.
 *  Must exceed SERVER_START_WAIT_MS to avoid Mocha killing the hook early. */
export const SUITE_SETUP_TIMEOUT_MS = 90_000;

/** Maximum time (ms) to wait for a server-advertised command to appear. */
export const COMMAND_WAIT_MS = 1_000;


/** Default interval (ms) for polling loops. */
export const DEFAULT_POLL_INTERVAL_MS = 100;

/** Interval (ms) between server readiness polls during setup. */
const SERVER_READINESS_POLL_INTERVAL_MS = 200;


function detectShipwrightPlatform(): string {
    if (process.platform === 'darwin' && process.arch === 'arm64') { return 'darwin-arm64'; }
    if (process.platform === 'linux' && process.arch === 'arm64') { return 'linux-arm64'; }
    if (process.platform === 'linux') { return 'linux-x64'; }
    if (process.platform === 'win32' && process.arch === 'arm64') { return 'win32-arm64'; }
    if (process.platform === 'win32') { return 'win32-x64'; }
    return 'linux-x64';
}

/** Resolve the basilisk binary for tests: bundled VSIX binary first, then workspace build. */
export function findBasiliskBinary(): string | undefined {
    const extensionRoot = path.resolve(__dirname, '../../..');
    const exe = process.platform === 'win32' ? '.exe' : '';
    const bundled = path.join(extensionRoot, 'bin', detectShipwrightPlatform(), `basilisk${exe}`);
    if (fs.existsSync(bundled)) {
        return bundled;
    }

    const workspaceRoot = path.resolve(__dirname, '../../../..');
    const releaseBinary = path.join(workspaceRoot, 'target', 'release', `basilisk${exe}`);
    if (fs.existsSync(releaseBinary)) {
        return releaseBinary;
    }

    const debugBinary = path.join(workspaceRoot, 'target', 'debug', `basilisk${exe}`);
    if (fs.existsSync(debugBinary)) {
        return debugBinary;
    }

    return undefined;
}

/**
 * Wait until at least one diagnostic appears for the given URI.
 * Throws if no diagnostics arrive before the timeout elapses.
 */
export async function waitForDiagnostics(
    uri: vscode.Uri,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS
): Promise<vscode.Diagnostic[]> {
    return new Promise((resolve, reject) => {
        const existing = vscode.languages.getDiagnostics(uri);
        if (existing.length > 0) {
            resolve(existing);
            return;
        }

        const timeout = setTimeout(() => {
            disposable.dispose();
            const stale = vscode.languages.getDiagnostics(uri);
            if (stale.length > 0) {
                resolve(stale);
            } else {
                reject(new Error(
                    `waitForDiagnostics timed out after ${timeoutMs}ms — ` +
                    `no diagnostics appeared for ${uri.fsPath}`
                ));
            }
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
 * Wait for diagnostics to clear (reach zero) for the given URI.
 * Throws if diagnostics remain when the timeout elapses.
 */
export async function waitForDiagnosticsCleared(
    uri: vscode.Uri,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS
): Promise<vscode.Diagnostic[]> {
    return new Promise((resolve, reject) => {
        const existing = vscode.languages.getDiagnostics(uri);
        if (existing.length === 0) {
            resolve([]);
            return;
        }

        const timeout = setTimeout(() => {
            disposable.dispose();
            const remaining = vscode.languages.getDiagnostics(uri);
            if (remaining.length === 0) {
                resolve([]);
            } else {
                reject(new Error(
                    `waitForDiagnosticsCleared timed out after ${timeoutMs}ms — ${remaining.length} diagnostic(s) still present for ${uri.fsPath}: ${remaining.map((d) => d.message).join('; ')}`
                ));
            }
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

/** Options for polling an async function until a predicate is satisfied. */
export interface PollOptions<T> {
    fn: () => PromiseLike<T>;
    predicate: (result: T) => boolean;
    timeoutMs?: number;
    intervalMs?: number;
}

/**
 * Poll an async function until it returns a result satisfying the predicate.
 * Throws if the predicate is never satisfied before the timeout elapses.
 *
 * Supports two calling conventions:
 * - `pollUntilResult({ fn, predicate, timeoutMs?, intervalMs? })`
 * - `pollUntilResult(fn, predicate)`
 */
export async function pollUntilResult<T>(
    optionsOrFn: PollOptions<T> | (() => PromiseLike<T>),
    predicateArg?: (result: T) => boolean,
): Promise<T> {
    const options: PollOptions<T> = typeof optionsOrFn === 'function'
        ? { fn: optionsOrFn, predicate: predicateArg ?? (() => true) }
        : optionsOrFn;
    const { fn, predicate, timeoutMs = NO_DIAGNOSTIC_WAIT_MS, intervalMs = DEFAULT_POLL_INTERVAL_MS } = options;
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
        const result = await fn();
        if (predicate(result)) {return result;}
        await new Promise<void>((r) => setTimeout(r, intervalMs));
    }
    // One final attempt after deadline.
    const last = await fn();
    if (predicate(last)) {return last;}
    throw new Error(
        `pollUntilResult timed out after ${timeoutMs}ms — ` +
        `predicate never satisfied (last result: ${JSON.stringify(last)})`
    );
}

/**
 * Create a temporary Python file, open it in the editor, and return
 * the document + URI. Caller is responsible for cleanup via tmpDir.
 */
export async function openPythonFile(
    tmpDir: string,
    filename: string,
    content: string
): Promise<{ doc: vscode.TextDocument; uri: vscode.Uri }> {
    const filePath = path.join(tmpDir, filename);
    fs.writeFileSync(filePath, content, 'utf8');
    const uri = vscode.Uri.file(filePath);
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc, { preview: false });
    return { doc, uri };
}

/** Close all open editors to avoid cross-test pollution. */
export async function closeAllEditors(): Promise<void> {
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');
}

/**
 * Replace the entire contents of a document with new text.
 * Uses WorkspaceEdit for reliability — editor.edit() can fail when
 * the editor state is transitioning (e.g. after a server restart).
 */
export async function replaceDocumentContent(
    doc: vscode.TextDocument,
    newContent: string
): Promise<boolean> {
    const edit = new vscode.WorkspaceEdit();
    const fullRange = new vscode.Range(
        new vscode.Position(0, 0),
        new vscode.Position(doc.lineCount, 0)
    );
    edit.replace(doc.uri, fullRange, newContent);
    return vscode.workspace.applyEdit(edit);
}

/**
 * Wait until the Basilisk LSP has fully initialized and advertised its commands.
 *
 * Use this in any suiteSetup that needs the LSP to be running before tests
 * execute. It polls store.serverCommands rather than documentSymbol so
 * Basilisk's own initialization — not VS Code's built-in Python extension
 * — determines readiness.
 *
 * Handles the lazy re-init path after deactivate() cycles from earlier test
 * suites: first calls getStore() to clear pendingReactivation (returns
 * undefined), then calls again to trigger initExtension.
 */
export async function waitForLspReady(): Promise<void> {
    const ext = vscode.extensions.getExtension(EXTENSION_ID);
    if (ext && !ext.isActive) {
        await ext.activate();
    }

    const { getStore: getStoreFromExtension } = await import('../../extension');
    if (getStoreFromExtension() === undefined) {
        getStoreFromExtension();
    }

    const deadline = Date.now() + SERVER_START_WAIT_MS;
    while (Date.now() < deadline) {
        const store = getStoreFromExtension();
        if (store !== undefined && store.serverCommands.value.size > 0) {
            return;
        }
        await new Promise<void>((r) => setTimeout(r, SERVER_READINESS_POLL_INTERVAL_MS));
    }
    throw new Error(
        `LSP server failed to become responsive within ${SERVER_START_WAIT_MS}ms. ` +
        'Ensure the basilisk binary is built: cargo build -p basilisk-cli'
    );
}

/**
 * Standard suiteSetup body: find binary, create tmpDir, wait for the LSP
 * to be ready, then close all editors. Returns tmpDir and binary path.
 */
export async function setupLspTestSuite(
    tmpDirPrefix: string
): Promise<{ tmpDir: string; basiliskBinary: string }> {
    const binary = findBasiliskBinary();
    if (binary === undefined) {
        throw new Error(
            'Basilisk binary not found. Build with: cargo build -p basilisk-cli'
        );
    }

    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), tmpDirPrefix));

    await waitForLspReady();
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');

    return { tmpDir, basiliskBinary: binary };
}

/** Clean up a tmpDir created by setupLspTestSuite. */
export function teardownLspTestSuite(tmpDir: string): void {
    if (tmpDir !== '' && fs.existsSync(tmpDir)) {
        fs.rmSync(tmpDir, { recursive: true, force: true });
    }
}

// ── Hover & navigation helpers ───────────────────────────────────────
// Shared so the hover (lsp-hover) and goto (lsp-goto) suites can HAMMER
// every symbol kind without re-implementing the poll/extract plumbing.

/** Flatten VS Code hover results into a single searchable string. */
export function extractHoverText(hovers: readonly vscode.Hover[]): string {
    return hovers
        .flatMap((h) =>
            h.contents.map((c) => {
                if (typeof c === 'string') { return c; }
                if (c instanceof vscode.MarkdownString) { return c.value; }
                if ('value' in c) { return (c as { value: string }).value; }
                return '';
            })
        )
        .join('\n');
}

/**
 * Locate a token within multi-line source and return a Position pointing at
 * the MIDDLE of that token (so the cursor sits firmly inside the identifier,
 * the way a user hovering/clicking would land). `occurrence` selects which
 * match (0-based) when the token appears more than once — essential for
 * distinguishing a definition site from its later reference sites.
 *
 * Throws if the token (at the requested occurrence) is absent: a missing
 * token means the test fixture drifted, which must fail loudly, not silently
 * hover at (0,0).
 */
export function locate(content: string, token: string, occurrence = 0): vscode.Position {
    const lines = content.split('\n');
    let seen = 0;
    for (let line = 0; line < lines.length; line++) {
        let from = 0;
        for (; ;) {
            const col = lines[line].indexOf(token, from);
            if (col === -1) { break; }
            if (seen === occurrence) {
                return new vscode.Position(line, col + Math.floor(token.length / 2));
            }
            seen += 1;
            from = col + token.length;
        }
    }
    throw new Error(
        `locate: token "${token}" (occurrence ${occurrence}) not found in source`
    );
}

/**
 * Poll the hover provider until it returns content, then return the flattened
 * hover text. Returns '' (never throws) when no hover ever materialises, so a
 * test can assert presence with a clear message instead of an opaque timeout.
 */
export async function getHoverText(
    uri: vscode.Uri,
    position: vscode.Position,
    timeoutMs: number = DIAGNOSTIC_TIMEOUT_MS,
): Promise<string> {
    // Poll until the hover has non-empty CONTENT, not merely a non-empty array.
    // During the analysis window the provider can transiently return a Hover with
    // empty contents (the intermittent "no content" regression, #200); resolving
    // on array length alone yields '' and a spurious failure. Gating on extracted
    // text makes the wait deterministic — it resolves only once real content
    // materialises, bounded by timeoutMs.
    const hovers = await pollUntilResult({
        fn: async () => vscode.commands.executeCommand<vscode.Hover[]>(
            'vscode.executeHoverProvider', uri, position
        ).then((r) => r ?? [], () => [] as vscode.Hover[]),
        predicate: (r) => Array.isArray(r) && extractHoverText(r).trim().length > 0,
        timeoutMs,
    }).catch(() => [] as vscode.Hover[]);
    return extractHoverText(hovers);
}

/** Definition-family providers usable with {@link getNavLocations}. */
export type NavProvider =
    | 'vscode.executeDefinitionProvider'
    | 'vscode.executeDeclarationProvider'
    | 'vscode.executeTypeDefinitionProvider';

/** Normalise the `(Location | LocationLink)[]` a provider may return to `Location[]`. */
export function normalizeLocations(
    raw: readonly (vscode.Location | vscode.LocationLink)[]
): vscode.Location[] {
    return raw.map((l) =>
        'targetUri' in l ? new vscode.Location(l.targetUri, l.targetRange) : l
    );
}

/**
 * Poll a definition-family provider until it returns at least one location,
 * normalising LocationLink results. Returns [] (never throws) on timeout so
 * the caller can assert with a descriptive message.
 */
export async function getNavLocations(
    command: NavProvider,
    uri: vscode.Uri,
    position: vscode.Position,
): Promise<vscode.Location[]> {
    const raw = await pollUntilResult({
        fn: async () => vscode.commands.executeCommand<(vscode.Location | vscode.LocationLink)[]>(
            command, uri, position
        ).then((r) => r ?? [], () => [] as vscode.Location[]),
        predicate: (r) => Array.isArray(r) && r.length > 0,
        timeoutMs: DIAGNOSTIC_TIMEOUT_MS,
    }).catch(() => [] as (vscode.Location | vscode.LocationLink)[]);
    return normalizeLocations(raw);
}
