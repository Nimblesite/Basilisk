// Implements [CONFIGEDITOR-ACCESSIBILITY-SECURITY] structured error routing.

import * as path from "path";
import * as vscode from "vscode";

const CONFLICT_WORDS = ["stale", "conflict", "revision", "changed since preview"] as const;

export interface ConfigurationError {
  readonly message: string;
  readonly conflict: boolean;
  readonly repairUri: string | undefined;
}

/** Accept only the root-level `pyproject.toml` as a repair/open target. */
export function configurationRepairUri(value: unknown, rootUri: string | undefined): string | undefined {
  if (typeof value !== "string" || rootUri === undefined) { return undefined; }
  try {
    const source = vscode.Uri.parse(value, true);
    const root = vscode.Uri.parse(rootUri, true);
    if (source.scheme !== "file" || root.scheme !== "file") { return undefined; }
    const sourcePath = path.resolve(source.fsPath);
    const rootPath = path.resolve(root.fsPath);
    if (path.dirname(sourcePath) !== rootPath || path.basename(sourcePath) !== "pyproject.toml") {
      return undefined;
    }
    return source.toString();
  } catch {
    return undefined;
  }
}

export function configurationError(error: unknown, rootUri?: string): ConfigurationError {
  const record = typeof error === "object" && error !== null
    ? error as { readonly data?: unknown; readonly message?: unknown }
    : undefined;
  const message = error instanceof Error
    ? error.message
    : typeof record?.message === "string" ? record.message : String(error);
  const data = record?.data;
  const conflict = typeof data === "object" && data !== null
    && (data as { readonly kind?: unknown }).kind === "revisionConflict";
  const context = typeof data === "object" && data !== null
    ? (data as { readonly context?: unknown }).context
    : undefined;
  const sourceUri = typeof context === "object" && context !== null
    ? (context as { readonly sourceUri?: unknown }).sourceUri
    : undefined;
  return {
    message,
    conflict: conflict || CONFLICT_WORDS.some((word) => message.toLowerCase().includes(word)),
    repairUri: configurationRepairUri(sourceUri, rootUri),
  };
}

export function fileIsWithinRoot(target: vscode.Uri, rootUri: string | undefined): boolean {
  if (rootUri === undefined) { return false; }
  try {
    const root = vscode.Uri.parse(rootUri, true);
    if (root.scheme !== "file") { return false; }
    const relative = path.relative(path.resolve(root.fsPath), path.resolve(target.fsPath));
    return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
  } catch {
    return false;
  }
}
