// Implements [LSPCFGED-TYPESHED] native controls and read-only license view.

import * as vscode from "vscode";
import type { ConfigurationSnapshot, TypeshedLicenseDocument } from "./configuration-editor-model";
import type { ConfigurationEditorIntent } from "./configuration-editor-intents";

class LicenseProvider implements vscode.TextDocumentContentProvider, vscode.Disposable {
  private readonly changed = new vscode.EventEmitter<vscode.Uri>();
  private content = "";
  public readonly onDidChange = this.changed.event;
  public readonly uri = vscode.Uri.parse("basilisk-typeshed-license:/LICENSE");

  public provideTextDocumentContent(): string { return this.content; }
  public set(content: string): void { this.content = content; this.changed.fire(this.uri); }
  public dispose(): void { this.changed.dispose(); }
}

export class TypeshedEditorUi implements vscode.Disposable {
  private readonly provider = new LicenseProvider();
  private readonly registration = vscode.workspace.registerTextDocumentContentProvider(
    "basilisk-typeshed-license",
    this.provider,
  );

  public async showLicense(license: TypeshedLicenseDocument): Promise<void> {
    this.provider.set(`${license.title}\n\n${license.content}`);
    const document = await vscode.workspace.openTextDocument(this.provider.uri);
    await vscode.window.showTextDocument(document, { preview: true });
  }

  public dispose(): void { this.registration.dispose(); this.provider.dispose(); }
}

export async function confirmVerificationOff(): Promise<boolean> {
  const accepted = await vscode.window.showWarningMessage(
    "Disable Typeshed content verification? Safety, shape, and license gates will still run, but source status will report UNVERIFIED.",
    { modal: true },
    "Disable verification",
  );
  return accepted === "Disable verification";
}

export async function pickTypeshedFolder(
  snapshot: ConfigurationSnapshot,
  key: "TypeshedPath" | "TypeshedCachePath",
): Promise<Extract<ConfigurationEditorIntent, { type: "preview" }> | undefined> {
  const selected = await vscode.window.showOpenDialog({
    canSelectFiles: false, canSelectFolders: true, canSelectMany: false,
    defaultUri: vscode.Uri.parse(snapshot.rootUri, true),
    openLabel: key === "TypeshedPath" ? "Use Typeshed folder" : "Use cache folder",
    title: key === "TypeshedPath" ? "Choose a Typeshed tree containing stdlib/" : "Choose the Typeshed cache folder",
  });
  const folder = selected?.[0];
  if (folder === undefined) { return undefined; }
  const mutations: Extract<ConfigurationEditorIntent, { type: "preview" }>["mutations"] = [{
    kind: "SetTypeshedSetting", key: { kind: key }, value: { kind: "Text", value: folder.fsPath },
  }];
  if (key === "TypeshedPath") {
    mutations.push({ kind: "RemoveTypeshedSetting", key: { kind: "TypeshedCommit" } });
  }
  return { type: "preview", mutations };
}

/**
 * A Typeshed edit is a direct source switch, not a rule-severity trade-off:
 * there is no impact to weigh, so it applies as soon as it is made
 * ([LSPCFGED-TYPESHED]). A control that needed a second confirmation could
 * show a value the configuration does not hold.
 */
export function isTypeshedOnly(
  intent: Extract<ConfigurationEditorIntent, { type: "preview" }>,
): boolean {
  return intent.mutations.every((mutation) =>
    mutation.kind === "SetTypeshedSetting" || mutation.kind === "RemoveTypeshedSetting");
}

export function disablesTypeshedVerification(
  intent: Extract<ConfigurationEditorIntent, { type: "preview" }>,
): boolean {
  return intent.mutations.some((mutation) =>
    mutation.kind === "SetTypeshedSetting"
    && mutation.key.kind === "TypeshedVerify"
    && mutation.value.kind === "Boolean"
    && !mutation.value.value);
}
