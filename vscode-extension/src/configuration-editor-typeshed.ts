// Implements [LSPCFGED-TYPESHED] read-only license view. The folder pickers
// and the direct-write rule it shares with the Caching panel live in
// `configuration-editor-settings.ts`.

import * as vscode from "vscode";
import type { TypeshedLicenseDocument } from "./configuration-editor-model";

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
