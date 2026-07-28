// Implements [CONFIGEDITOR-VSIX-EXPERIENCE] navigation out of the editor.
/**
 * Opening a document, a rule guide, or an occurrence from the configuration
 * editor. Every target is checked against the state the server sent — the
 * webview is untrusted, so a URI it did not receive is never opened.
 */

import * as vscode from "vscode";
import type { ConfigurationEditorState } from "./configuration-editor-state";
import { configurationRepairUri, fileIsWithinRoot } from "./configuration-editor-errors";

/** Open one of the nested folder configs the snapshot actually listed. */
export async function openConfigFile(
  state: ConfigurationEditorState,
  uri: string,
): Promise<void> {
  const overrides = state.snapshot?.pathOverrides ?? [];
  if (!overrides.some((entry) => entry.configUri === uri)) { return; }
  const target = vscode.Uri.parse(uri);
  if (target.scheme !== "file" || !fileIsWithinRoot(target, state.rootUri)) { return; }
  await vscode.window.showTextDocument(target, { preview: false });
}

/**
 * Open the raw active configuration — the repair target when the document is
 * malformed, otherwise the source the snapshot names. A source that does not
 * exist yet is not an error: the file is created on the first applied change.
 */
export async function openRawConfiguration(state: ConfigurationEditorState): Promise<void> {
  const repairUri = state.repairUri;
  if (repairUri !== undefined) {
    await vscode.window.showTextDocument(vscode.Uri.parse(repairUri, true), { preview: false });
    return;
  }
  const sourceUri = configurationRepairUri(state.snapshot?.configUri, state.snapshot?.rootUri);
  if (sourceUri === undefined) { return; }
  try {
    await vscode.window.showTextDocument(vscode.Uri.parse(sourceUri, true), { preview: false });
  } catch {
    void vscode.window.showInformationMessage("Basilisk will create pyproject.toml when you apply a configuration change.");
  }
}

/** Open a rule guide, but only a URL the catalog itself advertised. */
export async function openRuleDocs(state: ConfigurationEditorState, uri: string): Promise<void> {
  const rules = state.snapshot?.rules ?? [];
  if (!rules.some((rule) => rule.descriptor.docsUrl === uri)) { return; }
  const target = vscode.Uri.parse(uri);
  if (target.scheme === "https") { await vscode.env.openExternal(target); }
}

/** Reveal one occurrence the server actually returned, at its own position. */
export async function openOccurrence(
  state: ConfigurationEditorState,
  occurrence: { readonly uri: string; readonly line: number; readonly character: number },
): Promise<void> {
  const items = state.occurrences?.items ?? [];
  const allowed = items.some((item) => item.uri === occurrence.uri
    && item.range.start.line === occurrence.line
    && item.range.start.character === occurrence.character);
  if (!allowed) { return; }
  const target = vscode.Uri.parse(occurrence.uri);
  if (target.scheme !== "file" || !fileIsWithinRoot(target, state.rootUri)) { return; }
  const position = new vscode.Position(occurrence.line, occurrence.character);
  await vscode.window.showTextDocument(target, { preview: false, selection: new vscode.Range(position, position) });
}
