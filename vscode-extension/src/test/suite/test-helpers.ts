/**
 * Shared test helpers for Basilisk VS Code extension E2E tests.
 *
 * Centralises LSP test utilities that were previously duplicated across
 * every test file: binary discovery, diagnostic polling, file management.
 */

import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { execFileSync } from 'child_process';

export const EXTENSION_ID = 'basilisk-lang.basilisk';

/** Maximum time (ms) to wait for diagnostics from the LSP server. */
export const DIAGNOSTIC_TIMEOUT_MS = 15_000;

/** Time (ms) to wait for "no diagnostics" assertions. */
export const NO_DIAGNOSTIC_WAIT_MS = 5_000;

/** Time (ms) to wait for the LSP server to fully start. */
export const SERVER_START_WAIT_MS = 10_000;

/**
 * Resolves the absolute path to the basilisk binary built from Cargo.
 * Returns undefined if the binary does not exist.
 */
export function findBasiliskBinary(): string | undefined {
    const envPath = process.env.BASILISK_EXECUTABLE_PATH;
    if (envPath && fs.existsSync(envPath)) {
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
        execFileSync('basilisk', ['--version'], { timeout: 5000 });
        return 'basilisk';
    } catch {
        return undefined;
    }
}

/**
 * Wait until at least one diagnostic appears for the given URI,
 * or until the timeout elapses — whichever comes first.
 */
export function waitForDiagnostics(
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
export function waitForDiagnosticsCleared(
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

/**
 * Poll an async function until it returns a truthy, non-empty result.
 * Avoids fixed sleeps by retrying at short intervals.
 */
export async function pollUntilResult<T>(
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
    if (!binary) {
        throw new Error(
            'Basilisk binary not found. Build with: cargo build -p basilisk-cli'
        );
    }

    const tmpDir = fs.mkdtempSync(path.join(require('os').tmpdir(), tmpDirPrefix));

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
    const deadline = Date.now() + SERVER_START_WAIT_MS;
    while (Date.now() < deadline) {
        try {
            const syms = await vscode.commands.executeCommand<vscode.DocumentSymbol[]>(
                'vscode.executeDocumentSymbolProvider', dummyUri
            );
            if (syms !== null && syms !== undefined) break;
        } catch { /* server not ready yet */ }
        await new Promise<void>((r) => setTimeout(r, 200));
    }
    await vscode.commands.executeCommand('workbench.action.closeAllEditors');

    return { tmpDir, basiliskBinary: binary };
}

/** Clean up a tmpDir created by setupLspTestSuite. */
export function teardownLspTestSuite(tmpDir: string): void {
    if (tmpDir && fs.existsSync(tmpDir)) {
        fs.rmSync(tmpDir, { recursive: true, force: true });
    }
}
