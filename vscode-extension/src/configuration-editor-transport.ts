// Implements [VSIX-CONFIGURATION-EDITOR] / [VSIX-CONFIGURATION-EDITOR-THIN-SHELL].
/** LSP seam for the configuration editor: capability probe, transport, root choice.
 *
 * Split out of `configuration-editor.ts` so every file behind
 * [VSIX-CONFIGURATION-EDITOR-FILES] stays under the repository's 500-LOC ceiling.
 * Everything here is about talking to the server or picking which workspace root
 * to talk about — none of it touches the panel, so the host file keeps only
 * lifecycle and intent routing.
 */

import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";
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
import type { Store } from "./store";

const SNAPSHOT_METHOD = "basilisk/configurationSnapshot";
const PREVIEW_METHOD = "basilisk/previewConfigurationChange";
const APPLY_METHOD = "basilisk/applyConfigurationChange";
const OCCURRENCES_METHOD = "basilisk/ruleOccurrences";
const TYPESHED_ACTION_METHOD = "basilisk/typeshedAction";
const EXECUTE_COMMAND_METHOD = "workspace/executeCommand";

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

export function clientTransport(store: Store): ConfigurationEditorTransport {
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
