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
import { execFileSync } from 'child_process';

export const EXTENSION_ID = 'basilisk-lang.basilisk';

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

/** Timeout (ms) for basilisk binary version check via CLI. */
const BINARY_VERSION_CHECK_TIMEOUT_MS = 5_000;

/** Default interval (ms) for polling loops. */
export const DEFAULT_POLL_INTERVAL_MS = 100;

/** Interval (ms) between server readiness polls during setup. */
const SERVER_READINESS_POLL_INTERVAL_MS = 200;

/**
 * Module-level flag: once any poll (prewarmLsp or setupLspTestSuite)
 * confirms the LSP server is responsive, subsequent calls skip the
 * expensive 60-second poll entirely.
 */
let lspReadyConfirmed = false;

/** Mark the LSP server as confirmed ready (called from prewarmLsp). */
export function markLspReady(): void {
    lspReadyConfirmed = true;
}

/** Returns true if the LSP server has been confirmed ready in this test run. */
export function isLspReady(): boolean {
    return lspReadyConfirmed;
}


/**
 * Resolves the absolute path to the basilisk binary built from Cargo.
 * Returns undefined if the binary does not exist.
 */
export function findBasiliskBinary(): string | undefined {
    const envPath = process.env.BASILISK_EXECUTABLE_PATH;
    if (envPath !== undefined && envPath !== '' && fs.existsSync(envPath)) {
        return envPath;
    }

    // __dirname at runtime is vscode-extension/out/test/suite/ — 4 levels to repo root.
    const workspaceRoot = path.resolve(__dirname, '../../../..');
    const releaseBinary = path.join(workspaceRoot, 'target', 'release', 'basilisk');
    if (fs.existsSync(releaseBinary)) {
        return releaseBinary;
    }

    const debugBinary = path.join(workspaceRoot, 'target', 'debug', 'basilisk');
    if (fs.existsSync(debugBinary)) {
        return debugBinary;
    }

    try {
        execFileSync('basilisk', ['--version'], { timeout: BINARY_VERSION_CHECK_TIMEOUT_MS });
        return 'basilisk';
    } catch {
        return undefined;
    }
}

/**
 * Wait until at least one diagnostic appears for the given URI,
 * or until the timeout elapses — whichever comes first.
 */
export async function waitForDiagnostics(
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
 * Wait for diagnostics to clear (reach zero) for the given URI,
 * or until the timeout elapses.
 */
export async function waitForDiagnosticsCleared(
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

/** Options for polling an async function until a predicate is satisfied. */
export interface PollOptions<T> {
    fn: () => PromiseLike<T>;
    predicate: (result: T) => boolean;
    timeoutMs?: number;
    intervalMs?: number;
}

/**
 * Poll an async function until it returns a truthy, non-empty result.
 * Avoids fixed sleeps by retrying at short intervals.
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
    return fn();
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
 * Standard suiteSetup body: find binary, create tmpDir, activate extension,
 * poll until the LSP server responds to documentSymbol requests.
 * Returns the tmpDir path.
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

    const ext = vscode.extensions.getExtension(EXTENSION_ID);
    if (ext && !ext.isActive) {
        await ext.activate();
    }

    // After a simulated deactivate() cycle (e.g. from a prior test suite),
    // the extension is still marked as active by VS Code, but the store is
    // undefined and the LSP is stopped. Calling getStore() triggers the
    // lazy re-init path that creates a new store and starts the LSP client.
    const { getStore: getStoreFromExtension } = await import('../../extension');
    // First call after deactivate() returns undefined (proves cleanup).
    // Second call triggers lazy re-init.
    if (getStoreFromExtension() === undefined) {
        getStoreFromExtension();
    }

    // Poll until the LSP server is responsive.
    // Skip if a prior call (prewarmLsp or earlier suite) already confirmed readiness.
    if (!lspReadyConfirmed) {
        const dummyPath = path.join(tmpDir, '__init__.py');
        fs.writeFileSync(dummyPath, '', 'utf8');
        const dummyUri = vscode.Uri.file(dummyPath);
        const dummyDoc = await vscode.workspace.openTextDocument(dummyUri);
        await vscode.window.showTextDocument(dummyDoc);
        const deadline = Date.now() + SERVER_START_WAIT_MS;
        while (Date.now() < deadline) {
            try {
                const syms = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                    'vscode.executeDocumentSymbolProvider', dummyUri
                );
                if (syms !== null && syms !== undefined) {
                    lspReadyConfirmed = true;
                    break;
                }
            } catch { /* server not ready yet */ }
            await new Promise<void>((r) => setTimeout(r, SERVER_READINESS_POLL_INTERVAL_MS));
        }
        await vscode.commands.executeCommand('workbench.action.closeAllEditors');
    }

    return { tmpDir, basiliskBinary: binary };
}

/** Clean up a tmpDir created by setupLspTestSuite. */
export function teardownLspTestSuite(tmpDir: string): void {
    if (tmpDir !== '' && fs.existsSync(tmpDir)) {
        fs.rmSync(tmpDir, { recursive: true, force: true });
    }
}
