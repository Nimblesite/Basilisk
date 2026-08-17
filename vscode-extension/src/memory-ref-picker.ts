// Implements [PROFILE-MEMORY-REFGRAPH-PICKER].
// See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-REFGRAPH-PICKER
/**
 * The data-driven type picker for "Show Reference Graph".
 *
 * The reference-graph walk needs a Python type name. Asking the user to *type*
 * one into a blank box is a dead end — they have to already know the answer. This
 * module builds the candidate list from the real program instead: the active
 * file's own `class` definitions (via the LSP's `textDocument/documentSymbol`
 * provider) plus the common container builtins, with a free-text escape hatch.
 *
 * `gatherReferenceTypeCandidates` is a pure seam over the real symbol provider so
 * the e2e suite can assert the picker is populated from real document symbols (no
 * mocks), while the interactive `pickReferenceType` is the thin Quick Pick shell.
 */

import * as vscode from "vscode";
import { Logger } from "./logger";
import type { Store } from "./store";
import { LSP_MEM_CMD, runMemoryScript } from "./memory-capture";
import { decodeRefGraph, openRefGraphWebview } from "./memory-ref-graph";

/** Reference-graph traversal bounds. */
const REF_GRAPH_MAX_DEPTH = 5;
const REF_GRAPH_MAX_NODES = 200;

/** Container builtins that frequently retain leaked objects. */
const CONTAINER_BUILTINS = ["dict", "list", "set", "tuple", "frozenset"];

/** The Quick Pick item that drops to free-text entry. */
const OTHER_TYPE_LABEL = "$(edit) Other type…";

/**
 * Candidate types for the picker: the active file's classes first (the most
 * likely leak suspects), then container builtins, de-duplicated in order.
 * `uri === undefined` (no active editor) still yields the builtins, so the
 * picker is never empty.
 */
export async function gatherReferenceTypeCandidates(uri: vscode.Uri | undefined): Promise<string[]> {
  const classes = uri === undefined ? [] : await classSymbolNames(uri);
  return [...new Set([...classes, ...CONTAINER_BUILTINS])];
}

/** Class names from the active file via the real document-symbol provider. */
async function classSymbolNames(uri: vscode.Uri): Promise<string[]> {
  try {
    const symbols = await vscode.commands.executeCommand<
      (vscode.DocumentSymbol | vscode.SymbolInformation)[]
    >("vscode.executeDocumentSymbolProvider", uri);
    return collectClassNames(symbols ?? []);
  } catch (err: unknown) {
    Logger.warn(`[Memory] document symbols unavailable: ${err instanceof Error ? err.message : String(err)}`);
    return [];
  }
}

/** Class names from a (possibly nested) document-symbol tree. */
function collectClassNames(symbols: (vscode.DocumentSymbol | vscode.SymbolInformation)[]): string[] {
  const names: string[] = [];
  for (const sym of symbols) {
    if (sym.kind === vscode.SymbolKind.Class) { names.push(sym.name); }
    // `DocumentSymbol` nests; `SymbolInformation` does not. Test for the
    // field instead of asserting the union down to one arm.
    const children = "children" in sym ? sym.children : [];
    if (children.length > 0) {
      names.push(...collectClassNames(children));
    }
  }
  return names;
}

/**
 * Offer the data-driven Quick Pick, returning the chosen type (or undefined when
 * cancelled). "Other type…" falls back to a free-text input box.
 */
export async function pickReferenceType(): Promise<string | undefined> {
  const uri = vscode.window.activeTextEditor?.document.uri;
  const candidates = await gatherReferenceTypeCandidates(uri);
  const items: vscode.QuickPickItem[] = [
    ...candidates.map((type) => ({ label: type })),
    { label: OTHER_TYPE_LABEL },
  ];
  const pick = await vscode.window.showQuickPick(items, {
    placeHolder: "Inspect retainers of which type? (your classes + containers)",
  });
  if (pick === undefined) { return undefined; }
  if (pick.label !== OTHER_TYPE_LABEL) { return pick.label; }
  return vscode.window.showInputBox({
    prompt: "Object type to inspect (e.g. DataFrame, MyClass)",
    placeHolder: "DataFrame",
  });
}

/** Run the reference-graph walk for `typeName` and open the graph webview. */
export async function walkReferences(store: Store, typeName: string): Promise<void> {
  const result = await runMemoryScript(store, LSP_MEM_CMD.references, {
    targetType: typeName,
    maxDepth: REF_GRAPH_MAX_DEPTH,
    maxNodes: REF_GRAPH_MAX_NODES,
  });
  if (result?.kind === "refs") {
    openRefGraphWebview({
      targetType: typeName,
      maxDepth: REF_GRAPH_MAX_DEPTH,
      maxNodes: REF_GRAPH_MAX_NODES,
      script: "",
      graph: decodeRefGraph(result.graph),
    });
  }
}
