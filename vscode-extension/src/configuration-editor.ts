// Implements [VSIX-CONFIGURATION-EDITOR] / [VSIX-CONFIGURATION-EDITOR-THIN-SHELL].
/** VS Code host for the LSP-owned Basilisk configuration editor. */

import { effect } from "@preact/signals-core";
import * as path from "path";
import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
import { buildConfigurationEditorDocument } from "./configuration-editor-document";
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
} from "./configuration-editor-model";
import { Logger } from "./logger";
import { SingletonWebviewPanel, type WebviewMessage } from "./profiler-webview";
import type { Store } from "./store";

export const CONFIGURATION_EDITOR_COMMAND = "basilisk.openConfigurationEditor";
export const CONFIGURATION_EDITOR_CONTEXT = "basilisk.configurationEditorSupported";
/** [LSPARCH-CONFIG-EDITOR-PROTOCOL]: exact match with the server's advertised version. */
export const CONFIGURATION_EDITOR_VERSION = 2;
const SNAPSHOT_METHOD = "basilisk/configurationSnapshot";
const PREVIEW_METHOD = "basilisk/previewConfigurationChange";
const APPLY_METHOD = "basilisk/applyConfigurationChange";
const OCCURRENCES_METHOD = "basilisk/ruleOccurrences";
const CONFIGURATION_VIEW_TYPE = "basilisk.configurationEditor";
const CONFLICT_WORDS = ["stale", "conflict", "revision", "changed since preview"] as const;

/** Typed transport seam: production uses LanguageClient; tests can inject a fake. */
export interface ConfigurationEditorTransport {
  snapshot(rootUri: string): Promise<ConfigurationSnapshot>;
  preview(request: PreviewConfigurationRequest): Promise<ConfigurationPreview>;
  apply(request: ApplyConfigurationRequest): Promise<ConfigurationSnapshot>;
  occurrences(request: RuleOccurrencesRequest): Promise<RuleOccurrencesResponse>;
}

interface ExperimentalCapabilities {
  readonly basilisk?: {
    readonly configurationEditor?: { readonly version?: unknown };
  };
}

interface ConfigurationError {
  readonly message: string;
  readonly conflict: boolean;
  readonly repairUri: string | undefined;
}

/** Read the versioned experimental capability without trusting its runtime shape. */
export function configurationEditorCapabilityVersion(client: LanguageClient | undefined): number | undefined {
  const experimental = client?.initializeResult?.capabilities.experimental as unknown;
  if (typeof experimental !== "object" || experimental === null) { return undefined; }
  const version = (experimental as ExperimentalCapabilities).basilisk?.configurationEditor?.version;
  return typeof version === "number" && Number.isInteger(version) ? version : undefined;
}

export function supportsConfigurationEditor(client: LanguageClient | undefined): boolean {
  return configurationEditorCapabilityVersion(client) === CONFIGURATION_EDITOR_VERSION;
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
  };
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
    if (path.dirname(sourcePath) !== rootPath) { return undefined; }
    if (path.basename(sourcePath) !== "pyproject.toml") { return undefined; }
    return source.toString();
  } catch {
    return undefined;
  }
}

function errorDetails(error: unknown, rootUri?: string): ConfigurationError {
  const record = typeof error === "object" && error !== null
    ? error as { readonly data?: unknown; readonly message?: unknown }
    : undefined;
  const message = error instanceof Error
    ? error.message
    : typeof record?.message === "string" ? record.message : String(error);
  const lower = message.toLowerCase();
  const data = record?.data;
  const structuredConflict = typeof data === "object" && data !== null
    && (data as { readonly kind?: unknown }).kind === "revisionConflict";
  const context = typeof data === "object" && data !== null
    ? (data as { readonly context?: unknown }).context
    : undefined;
  const sourceUri = typeof context === "object" && context !== null
    ? (context as { readonly sourceUri?: unknown }).sourceUri
    : undefined;
  return {
    message,
    conflict: structuredConflict || CONFLICT_WORDS.some((word) => lower.includes(word)),
    repairUri: configurationRepairUri(sourceUri, rootUri),
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

  /** Open/re-render the editor for one explicit workspace root. */
  public open(rootUri: string): void {
    if (this.disposed) { return; }
    const wasOpen = this.panel.isOpen();
    const wasVisible = this.panel.isVisible();
    this.webviewReady = false;
    if (this.store.configurationEditor.value.rootUri !== rootUri) {
      this.pendingRefreshRoot = undefined;
    }
    this.store.beginConfigurationLoad(rootUri);
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
      case "occurrences": await this.loadOccurrences(intent.request); return;
      case "openRaw": await this.openRawConfiguration(); return;
      case "openDocs": await this.openRuleDocs(intent.uri); return;
      case "openOccurrence": await this.openOccurrence(intent); return;
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
      const details = errorDetails(error, rootUri);
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
      this.store.acceptConfigurationPreview(preview);
    } catch (error: unknown) {
      if (generation !== this.loadGeneration || previewGeneration !== this.previewGeneration
        || this.disposed || !this.panel.isOpen()) { return; }
      const details = errorDetails(error, snapshot.rootUri);
      this.store.failConfigurationEditor(details.message, details.conflict, details.repairUri);
    }
  }

  private async apply(): Promise<void> {
    const { snapshot, preview, phase } = this.store.configurationEditor.value;
    if (snapshot === undefined || preview === undefined || phase !== "preview") { return; }
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
      if (generation !== this.loadGeneration || this.disposed || !this.panel.isOpen()) { return; }
      this.store.acceptConfigurationSnapshot(fresh);
    } catch (error: unknown) {
      if (generation !== this.loadGeneration || this.disposed || !this.panel.isOpen()) { return; }
      const details = errorDetails(error, snapshot.rootUri);
      this.store.failConfigurationEditor(details.message, details.conflict, details.repairUri);
    }
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
      const details = errorDetails(error, rootUri);
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

function fileIsWithinRoot(target: vscode.Uri, rootUri: string | undefined): boolean {
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

/** Register the capability-gated command and context used by the view-title gear. */
export function registerConfigurationEditor(
  store: Store,
  transport?: ConfigurationEditorTransport,
): { readonly controller: ConfigurationEditorController; readonly disposables: vscode.Disposable[] } {
  const controller = new ConfigurationEditorController(store, transport);
  let command: vscode.Disposable | undefined;
  let previouslySupported = false;
  const disposeCapabilityEffect = effect(() => {
    const supported = transport !== undefined
      || (store.lspState.value === "running" && supportsConfigurationEditor(store.client.value));
    void vscode.commands.executeCommand("setContext", CONFIGURATION_EDITOR_CONTEXT, supported);
    if (supported && command === undefined) {
      command = vscode.commands.registerCommand(CONFIGURATION_EDITOR_COMMAND, async () => {
        const rootUri = await selectConfigurationRoot();
        if (rootUri === undefined) {
          void vscode.window.showInformationMessage("Open a workspace folder to configure Basilisk.");
          return;
        }
        controller.open(rootUri);
      });
    } else if (!supported) {
      command?.dispose();
      command = undefined;
      if (controller.isOpen() && previouslySupported) {
        controller.capabilityLost("The language server no longer advertises the configuration editor. Reconnect or update Basilisk.");
      }
    }
    if (supported && !previouslySupported) { controller.refreshOpen(); }
    previouslySupported = supported;
  });
  const capabilityLifecycle: vscode.Disposable = {
    dispose(): void {
      disposeCapabilityEffect();
      command?.dispose();
      command = undefined;
      void vscode.commands.executeCommand("setContext", CONFIGURATION_EDITOR_CONTEXT, false);
    },
  };
  const disposables: vscode.Disposable[] = [controller, capabilityLifecycle];
  return { controller, disposables };
}
