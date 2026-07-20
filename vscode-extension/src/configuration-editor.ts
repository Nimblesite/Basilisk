// Implements [VSIX-CONFIGURATION-EDITOR] / [VSIX-CONFIGURATION-EDITOR-THIN-SHELL].
/** VS Code host for the LSP-owned Basilisk configuration editor. */

import { effect } from "@preact/signals-core";
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
import { buildConfigurationEditorDocument } from "./configuration-editor-document";
import {
  configurationError,
  configurationRepairUri,
  fileIsWithinRoot,
} from "./configuration-editor-errors";
import {
  decodeConfigurationEditorIntent,
  type ConfigurationEditorIntent,
} from "./configuration-editor-intents";
import type {
  ApplyConfigurationRequest,
  ConfigurationPreview,
  ConfigurationSnapshot,
  PreviewConfigurationRequest,
  RuleOccurrencesRequest,
  RuleOccurrencesResponse,
  TypeshedActionRequest,
  TypeshedActionResult,
} from "./configuration-editor-model";
import {
  confirmVerificationOff,
  disablesTypeshedVerification,
  isTypeshedOnly,
  pickTypeshedFolder,
  TypeshedEditorUi,
} from "./configuration-editor-typeshed";
import { Logger } from "./logger";
import { SingletonWebviewPanel, type WebviewMessage } from "./profiler-webview";
import type { Store } from "./store";

export const CONFIGURATION_EDITOR_COMMAND = "basilisk.openConfigurationEditor";
/** Explorer context-menu entry on pyproject.toml ("Edit Config"). */
export const EDIT_CONFIG_COMMAND = "basilisk.editConfig";
export const CONFIGURATION_EDITOR_CONTEXT = "basilisk.configurationEditorSupported";
const SNAPSHOT_METHOD = "basilisk/configurationSnapshot";
const PREVIEW_METHOD = "basilisk/previewConfigurationChange";
const APPLY_METHOD = "basilisk/applyConfigurationChange";
const OCCURRENCES_METHOD = "basilisk/ruleOccurrences";
const TYPESHED_ACTION_METHOD = "basilisk/typeshedAction";
const EXECUTE_COMMAND_METHOD = "workspace/executeCommand";
const ADOPT_WORKSPACE_COMMAND = "basilisk.adoptWorkspace";
const FIX_WORKSPACE_COMMAND = "basilisk.fixWorkspace";
const CONFIGURATION_VIEW_TYPE = "basilisk.configurationEditor";
export { configurationRepairUri } from "./configuration-editor-errors";

/** Typed transport seam: production uses LanguageClient; tests can inject a fake. */
export interface ConfigurationEditorTransport {
  snapshot(rootUri: string): Promise<ConfigurationSnapshot>;
  preview(request: PreviewConfigurationRequest): Promise<ConfigurationPreview>;
  apply(request: ApplyConfigurationRequest): Promise<ConfigurationSnapshot>;
  occurrences(request: RuleOccurrencesRequest): Promise<RuleOccurrencesResponse>;
  typeshedAction(request: TypeshedActionRequest): Promise<TypeshedActionResult>;
  executeCommand(command: string, args: readonly unknown[]): Promise<void>;
}

interface ExperimentalCapabilities {
  readonly basilisk?: {
    readonly configurationEditor?: unknown;
  };
}

/**
 * [LSPARCH-CONFIG-EDITOR-PROTOCOL]: the editor ships with the server, so the
 * capability is pure presence — `configurationEditor` advertised truthy.
 */
export function supportsConfigurationEditor(client: LanguageClient | undefined): boolean {
  const experimental = client?.initializeResult?.capabilities.experimental as unknown;
  if (typeof experimental !== "object" || experimental === null) { return false; }
  const capability = (experimental as ExperimentalCapabilities).basilisk?.configurationEditor;
  return capability !== undefined && capability !== null && capability !== false;
}

function clientTransport(store: Store): ConfigurationEditorTransport {
  function runningClient(): LanguageClient {
    const client = store.client.value;
    if (client?.isRunning() !== true) { throw new Error("The Basilisk language server is not running."); }
    return client;
  }
  return {
    async snapshot(rootUri: string): Promise<ConfigurationSnapshot> {
      return runningClient().sendRequest<ConfigurationSnapshot>(SNAPSHOT_METHOD, { rootUri });
    },
    async preview(request: PreviewConfigurationRequest): Promise<ConfigurationPreview> {
      return runningClient().sendRequest<ConfigurationPreview>(PREVIEW_METHOD, request);
    },
    async apply(request: ApplyConfigurationRequest): Promise<ConfigurationSnapshot> {
      return runningClient().sendRequest<ConfigurationSnapshot>(APPLY_METHOD, request);
    },
    async occurrences(request: RuleOccurrencesRequest): Promise<RuleOccurrencesResponse> {
      return runningClient().sendRequest<RuleOccurrencesResponse>(OCCURRENCES_METHOD, request);
    },
    async typeshedAction(request: TypeshedActionRequest): Promise<TypeshedActionResult> {
      return runningClient().sendRequest<TypeshedActionResult>(TYPESHED_ACTION_METHOD, request);
    },
    async executeCommand(command: string, args: readonly unknown[]): Promise<void> {
      await runningClient().sendRequest(EXECUTE_COMMAND_METHOD, { command, arguments: args });
    },
  };
}

function fileWorkspaceRoot(): vscode.WorkspaceFolder | undefined {
  const uri = vscode.window.activeTextEditor?.document.uri;
  return uri === undefined ? undefined : vscode.workspace.getWorkspaceFolder(uri);
}

/** Choose an explicit root; active-editor ownership wins in a multi-root workspace. */
export async function selectConfigurationRoot(): Promise<string | undefined> {
  const activeRoot = fileWorkspaceRoot();
  if (activeRoot !== undefined) { return activeRoot.uri.toString(); }
  const roots = vscode.workspace.workspaceFolders ?? [];
  if (roots.length === 1) { return roots[0]?.uri.toString(); }
  if (roots.length === 0) { return undefined; }
  const choice = await vscode.window.showQuickPick(
    roots.map((root) => ({ label: root.name, detail: root.uri.fsPath, rootUri: root.uri.toString() })),
    { title: "Basilisk Configuration", placeHolder: "Choose the workspace configuration to edit" },
  );
  return choice?.rootUri;
}

/** Singleton editor tab and intent router; all configuration state remains in Store. */
export class ConfigurationEditorController implements vscode.Disposable {
  private readonly panel: SingletonWebviewPanel;
  private readonly transport: ConfigurationEditorTransport;
  private readonly disposeStateEffect: () => void;
  private readonly typeshedUi = new TypeshedEditorUi();
  private webviewReady = false;
  private readyMessages = 0;
  private loadingRoot: string | undefined;
  private pendingRefreshRoot: string | undefined;
  private loadGeneration = 0;
  private previewGeneration = 0;
  private occurrenceGeneration = 0;
  private disposed = false;

  constructor(private readonly store: Store, transport?: ConfigurationEditorTransport) {
    this.transport = transport ?? clientTransport(store);
    this.panel = new SingletonWebviewPanel(
      CONFIGURATION_VIEW_TYPE,
      (message: WebviewMessage) => { void this.receive(message); },
      {
        viewColumn: vscode.ViewColumn.Active,
        retainContextWhenHidden: false,
        enableFindWidget: true,
        onDidReveal: () => { void this.refresh(); },
        onDidDispose: () => { this.handlePanelDisposed(); },
      },
    );
    this.disposeStateEffect = effect(() => {
      const state = this.store.configurationEditor.value;
      if (this.webviewReady) {
        void this.panel.postMessage({ type: "state", state });
      }
      if (state.refreshRequested && state.rootUri !== undefined) {
        if (this.loadingRoot === undefined) {
          void this.load(state.rootUri);
        } else {
          this.pendingRefreshRoot = state.rootUri;
        }
      }
    });
  }

  /**
   * Open/re-render the editor for one explicit workspace root, optionally
   * focused on one rule (the diagnostic hover's Configure Severity link,
   * [CONFIGEDITOR-VSIX-EXPERIENCE]).
   */
  public open(rootUri: string, focusRule?: string): void {
    if (this.disposed) { return; }
    const wasOpen = this.panel.isOpen();
    const wasVisible = this.panel.isVisible();
    this.webviewReady = false;
    if (this.store.configurationEditor.value.rootUri !== rootUri) {
      this.pendingRefreshRoot = undefined;
    }
    // A plain open must clear any stale focus target — `null`, not undefined.
    this.store.beginConfigurationLoad(rootUri, focusRule ?? null);
    this.panel.show("Basilisk Configuration", buildConfigurationEditorDocument());
    // A hidden live panel refreshes from the real hidden→visible callback.
    // New/already-visible panels do not produce that transition, so load here.
    if (!wasOpen || wasVisible) { void this.load(rootUri); }
  }

  public isOpen(): boolean { return this.panel.isOpen(); }

  /** Number of real ready handshakes received (e2e lifecycle seam). */
  public readyMessageCount(): number { return this.readyMessages; }

  /** Test seam for exercising the same runtime decoder/router as the webview. */
  public async receive(message: unknown): Promise<void> {
    const intent = decodeConfigurationEditorIntent(message);
    if (intent === undefined) {
      Logger.warn("Ignored invalid configuration editor webview message");
      return;
    }
    await this.route(intent);
  }

  private async route(intent: ConfigurationEditorIntent): Promise<void> {
    switch (intent.type) {
      case "ready": this.handleReady(); return;
      case "refresh": await this.refresh(); return;
      case "preview": await this.preview(intent); return;
      case "apply": await this.apply(); return;
      case "cancelPreview": this.cancelPreview(); return;
      case "adopt": await this.runWorkspaceCommand(ADOPT_WORKSPACE_COMMAND, false); return;
      case "fixSafe": await this.runWorkspaceCommand(FIX_WORKSPACE_COMMAND, true); return;
      case "openConfigFile": await this.openConfigFile(intent.uri); return;
      case "occurrences": await this.loadOccurrences(intent.request); return;
      case "openRaw": await this.openRawConfiguration(); return;
      case "openDocs": await this.openRuleDocs(intent.uri); return;
      case "openOccurrence": await this.openOccurrence(intent); return;
      case "pickTypeshedFolder": await this.pickTypeshedFolder(intent.key); return;
      case "typeshedAction": await this.runTypeshedAction(intent.action); return;
    }
  }

  private handleReady(): void {
    this.readyMessages += 1;
    this.webviewReady = true;
    void this.panel.postMessage({ type: "state", state: this.store.configurationEditor.value });
  }

  private handlePanelDisposed(): void {
    this.webviewReady = false;
    this.loadGeneration += 1;
    this.previewGeneration += 1;
    this.occurrenceGeneration += 1;
    this.loadingRoot = undefined;
    this.pendingRefreshRoot = undefined;
    this.store.resetConfigurationEditor();
  }

  private async refresh(): Promise<void> {
    const rootUri = this.store.configurationEditor.value.rootUri;
    if (rootUri !== undefined) { await this.load(rootUri); }
  }

  private requestIsStale(generation: number, rootUri?: string): boolean {
    return generation !== this.loadGeneration || this.disposed || !this.panel.isOpen()
      || (rootUri !== undefined && this.store.configurationEditor.value.rootUri !== rootUri);
  }

  private async load(rootUri: string): Promise<void> {
    if (this.loadingRoot === rootUri || this.disposed) { return; }
    const generation = ++this.loadGeneration;
    this.occurrenceGeneration += 1;
    this.loadingRoot = rootUri;
    this.store.beginConfigurationLoad(rootUri);
    try {
      const snapshot = await this.transport.snapshot(rootUri);
      if (this.requestIsStale(generation)) { return; }
      if (snapshot.rootUri !== rootUri) { throw new Error("The server returned configuration for a different workspace root."); }
      this.store.acceptConfigurationSnapshot(snapshot);
    } catch (error: unknown) {
      if (this.requestIsStale(generation)) { return; }
      const details = configurationError(error, rootUri);
      this.store.failConfigurationEditor(details.message, details.conflict, details.repairUri);
    } finally {
      if (generation === this.loadGeneration) {
        this.loadingRoot = undefined;
        const pendingRoot = this.pendingRefreshRoot;
        this.pendingRefreshRoot = undefined;
        if (pendingRoot !== undefined && !this.requestIsStale(generation, pendingRoot)) {
          void this.load(pendingRoot);
        }
      }
    }
  }

  private async preview(intent: Extract<ConfigurationEditorIntent, { type: "preview" }>): Promise<void> {
    const state = this.store.configurationEditor.value;
    const snapshot = state.snapshot;
    if (snapshot === undefined || state.phase === "applying") { return; }
    if (disablesTypeshedVerification(intent)) {
      if (!await confirmVerificationOff()) {
        void this.panel.postMessage({ type: "state", state });
        return;
      }
    }
    const generation = this.loadGeneration;
    const previewGeneration = ++this.previewGeneration;
    this.store.beginConfigurationPreview();
    try {
      const preview = await this.transport.preview({
        rootUri: snapshot.rootUri,
        baseRevision: snapshot.revision,
        mutations: intent.mutations,
      });
      if (generation !== this.loadGeneration || previewGeneration !== this.previewGeneration
        || this.disposed || !this.panel.isOpen()) { return; }
      // A Typeshed edit has no severity impact to weigh, so it lands at once.
      if (isTypeshedOnly(intent)) { await this.applyPreview(preview); return; }
      this.store.acceptConfigurationPreview(preview);
    } catch (error: unknown) {
      if (generation !== this.loadGeneration || previewGeneration !== this.previewGeneration
        || this.disposed || !this.panel.isOpen()) { return; }
      const details = configurationError(error, snapshot.rootUri);
      this.store.failConfigurationEditor(details.message, details.conflict, details.repairUri);
    }
  }

  private async pickTypeshedFolder(key: "TypeshedPath" | "TypeshedCachePath"): Promise<void> {
    const state = this.store.configurationEditor.value;
    if (state.snapshot === undefined) { return; }
    const intent = await pickTypeshedFolder(state.snapshot, key);
    // A cancelled picker writes nothing, so the controls must snap back to the
    // configuration that still holds ([CONFIGEDITOR-VSIX-EXPERIENCE]).
    if (intent === undefined) { void this.panel.postMessage({ type: "state", state }); return; }
    await this.preview(intent);
  }

  private async runTypeshedAction(action: TypeshedActionRequest["action"]): Promise<void> {
    const snapshot = this.store.configurationEditor.value.snapshot;
    if (snapshot === undefined) { return; }
    const generation = this.loadGeneration;
    try {
      const result = await this.transport.typeshedAction({
        rootUri: snapshot.rootUri,
        baseRevision: snapshot.revision,
        action,
      });
      if (this.requestIsStale(generation, snapshot.rootUri)) { return; }
      if (result.kind === "Preview") {
        // PinCurrent writes the active commit: a source choice, applied now.
        await this.applyPreview(result.preview);
      } else if (result.kind === "Snapshot") {
        this.store.acceptConfigurationSnapshot(result.snapshot);
      } else {
        await this.typeshedUi.showLicense(result.license);
      }
    } catch (error: unknown) {
      if (this.requestIsStale(generation, snapshot.rootUri)) { return; }
      const details = configurationError(error, snapshot.rootUri);
      this.store.failConfigurationEditor(details.message, details.conflict, details.repairUri);
    }
  }

  private async apply(): Promise<void> {
    const { preview, phase } = this.store.configurationEditor.value;
    if (preview === undefined || phase !== "preview") { return; }
    await this.applyPreview(preview);
  }

  /**
   * Discard an unapplied preview and re-render from the snapshot, so a
   * dismissed dialog can never leave a control showing a value the
   * configuration does not hold ([CONFIGEDITOR-VSIX-EXPERIENCE]).
   */
  private cancelPreview(): void {
    this.previewGeneration += 1;
    this.store.cancelConfigurationPreview();
  }

  private async applyPreview(preview: ConfigurationPreview): Promise<void> {
    const snapshot = this.store.configurationEditor.value.snapshot;
    if (snapshot === undefined) { return; }
    const generation = this.loadGeneration;
    this.previewGeneration += 1;
    this.store.beginConfigurationApply();
    const sourceWasDirty = findConfigurationDocument(snapshot.configUri)?.isDirty === true;
    try {
      // [CONFIGEDITOR-OPERATIONS]: rootUri + previewId fully identify the
      // cached preview; the preview itself pins the base revision.
      const fresh = await this.transport.apply({
        rootUri: snapshot.rootUri,
        previewId: preview.previewId,
      });
      // Save before the staleness check: the server's configurationChanged
      // notification precedes the apply response, so a racing refresh
      // routinely bumps the generation — the disk write still has to land.
      if (!sourceWasDirty) { await saveConfigurationDocument(fresh.configUri); }
      if (this.disposed || !this.panel.isOpen()
        || this.store.configurationEditor.value.rootUri !== snapshot.rootUri) { return; }
      if (generation !== this.loadGeneration) {
        this.loadGeneration += 1;
        this.loadingRoot = undefined;
      }
      this.pendingRefreshRoot = undefined;
      this.store.acceptConfigurationSnapshot(fresh);
    } catch (error: unknown) {
      if (generation !== this.loadGeneration || this.disposed || !this.panel.isOpen()) { return; }
      const details = configurationError(error, snapshot.rootUri);
      this.store.failConfigurationEditor(details.message, details.conflict, details.repairUri);
    }
  }

  /**
   * Run a workspace-scoped server command (adopt current debt, apply safe
   * fixes) and reload. Both are the real, already-registered commands that
   * rewrite configuration via `workspace/applyEdit`; the editor only forwards
   * and re-snapshots — it never computes debt or edits config text itself.
   */
  private async runWorkspaceCommand(command: string, includeRoot: boolean): Promise<void> {
    const rootUri = this.store.configurationEditor.value.snapshot?.rootUri;
    if (rootUri === undefined || this.store.configurationEditor.value.phase === "applying") { return; }
    const generation = this.loadGeneration;
    this.previewGeneration += 1;
    this.store.beginConfigurationApply();
    try {
      await this.transport.executeCommand(command, includeRoot ? [{ rootUri }] : []);
      if (this.requestIsStale(generation, rootUri)) { return; }
      await this.load(rootUri);
    } catch (error: unknown) {
      if (this.requestIsStale(generation, rootUri)) { return; }
      const details = configurationError(error, rootUri);
      this.store.failConfigurationEditor(details.message, details.conflict, details.repairUri);
    }
  }

  /**
   * Open a nested path-override configuration file. Untrusted input: only a URI
   * the current snapshot listed as a path override, and only inside the root.
   */
  private async openConfigFile(uri: string): Promise<void> {
    const overrides = this.store.configurationEditor.value.snapshot?.pathOverrides ?? [];
    if (!overrides.some((entry) => entry.configUri === uri)) { return; }
    const target = vscode.Uri.parse(uri);
    const rootUri = this.store.configurationEditor.value.rootUri;
    if (target.scheme !== "file" || !fileIsWithinRoot(target, rootUri)) { return; }
    await vscode.window.showTextDocument(target, { preview: false });
  }

  private async loadOccurrences(request: Omit<RuleOccurrencesRequest, "rootUri">): Promise<void> {
    const rootUri = this.store.configurationEditor.value.snapshot?.rootUri;
    if (rootUri === undefined) { return; }
    const generation = this.loadGeneration;
    const occurrenceGeneration = ++this.occurrenceGeneration;
    const append = request.cursor !== undefined;
    this.store.beginRuleOccurrences(!append);
    try {
      const response = await this.transport.occurrences({ rootUri, ...request });
      if (generation !== this.loadGeneration || occurrenceGeneration !== this.occurrenceGeneration
        || this.disposed || !this.panel.isOpen()) { return; }
      this.store.acceptRuleOccurrences(response, append);
    } catch (error: unknown) {
      if (generation !== this.loadGeneration || occurrenceGeneration !== this.occurrenceGeneration
        || this.disposed || !this.panel.isOpen()) { return; }
      const details = configurationError(error, rootUri);
      this.store.failRuleOccurrences(details.message);
    }
  }

  private async openRawConfiguration(): Promise<void> {
    const state = this.store.configurationEditor.value;
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

  private async openRuleDocs(uri: string): Promise<void> {
    const rules = this.store.configurationEditor.value.snapshot?.rules ?? [];
    if (!rules.some((rule) => rule.descriptor.docsUrl === uri)) { return; }
    const target = vscode.Uri.parse(uri);
    if (target.scheme === "https") { await vscode.env.openExternal(target); }
  }

  private async openOccurrence(intent: Extract<ConfigurationEditorIntent, { type: "openOccurrence" }>): Promise<void> {
    const items = this.store.configurationEditor.value.occurrences?.items ?? [];
    const allowed = items.some((item) => item.uri === intent.uri
      && item.range.start.line === intent.line && item.range.start.character === intent.character);
    if (!allowed) { return; }
    const target = vscode.Uri.parse(intent.uri);
    const rootUri = this.store.configurationEditor.value.rootUri;
    if (target.scheme !== "file" || !fileIsWithinRoot(target, rootUri)) { return; }
    const position = new vscode.Position(intent.line, intent.character);
    await vscode.window.showTextDocument(target, { preview: false, selection: new vscode.Range(position, position) });
  }

  public dispose(): void {
    this.disposed = true;
    this.disposeStateEffect();
    this.typeshedUi.dispose();
    this.panel.dispose();
    this.store.resetConfigurationEditor();
  }

  /** Re-read an already-open editor after the capability returns. */
  public refreshOpen(): void {
    const rootUri = this.store.configurationEditor.value.rootUri;
    if (this.isOpen() && rootUri !== undefined) { void this.load(rootUri); }
  }

  /** Invalidate in-flight work and clear configuration data when support disappears. */
  public capabilityLost(message: string): void {
    this.loadGeneration += 1;
    this.previewGeneration += 1;
    this.occurrenceGeneration += 1;
    this.loadingRoot = undefined;
    this.pendingRefreshRoot = undefined;
    this.store.markConfigurationUnsupported(message);
  }
}

/** Locate the open text document backing the active configuration source. */
function findConfigurationDocument(sourceUri: string): vscode.TextDocument | undefined {
  try {
    const target = vscode.Uri.parse(sourceUri, true).toString();
    return vscode.workspace.textDocuments.find((document) => document.uri.toString() === target);
  } catch {
    return undefined;
  }
}

/**
 * Implements [CONFIGEDITOR-SOURCES]: a successful apply must reach disk.
 * `workspace.applyEdit` only rewrites the in-memory buffer, and the server
 * overlay merely bridges "until the client write is visible on disk" — so
 * persist the document the apply edit dirtied.
 */
async function saveConfigurationDocument(sourceUri: string): Promise<void> {
  const document = findConfigurationDocument(sourceUri);
  if (document?.isDirty !== true) { return; }
  const saved = await document.save();
  if (!saved) {
    Logger.warn("Configuration apply could not save pyproject.toml; the change is still unsaved in the editor");
  }
}
