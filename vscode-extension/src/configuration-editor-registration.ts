// Implements [VSIX-CONFIGURATION-EDITOR] capability-gated command registration.

import { effect } from "@preact/signals-core";
import * as vscode from "vscode";
import {
  ConfigurationEditorController,
  CONFIGURATION_EDITOR_COMMAND,
  CONFIGURATION_EDITOR_CONTEXT,
  EDIT_CONFIG_COMMAND,
  selectConfigurationRoot,
  supportsConfigurationEditor,
  type ConfigurationEditorTransport,
} from "./configuration-editor";
import type { Store } from "./store";

const MAX_RULE_CODE_LENGTH = 64;

export function configurationEditorFocusRule(value: unknown): string | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) { return undefined; }
  const rule = (value as { readonly rule?: unknown }).rule;
  return typeof rule === "string" && rule.length > 0 && rule.length <= MAX_RULE_CODE_LENGTH
    ? rule
    : undefined;
}

async function openConfigurationFor(
  controller: ConfigurationEditorController,
  resource?: vscode.Uri,
  focusRule?: string,
): Promise<void> {
  const folder = resource === undefined ? undefined : vscode.workspace.getWorkspaceFolder(resource);
  const rootUri = folder?.uri.toString() ?? await selectConfigurationRoot();
  if (rootUri === undefined) {
    void vscode.window.showInformationMessage("Open a workspace folder to configure Basilisk.");
    return;
  }
  controller.open(rootUri, focusRule);
}

/** Register the capability-gated editor commands and context. */
export function registerConfigurationEditor(
  store: Store,
  transport?: ConfigurationEditorTransport,
): { readonly controller: ConfigurationEditorController; readonly disposables: vscode.Disposable[] } {
  const controller = new ConfigurationEditorController(store, transport);
  let commands: vscode.Disposable[] | undefined;
  function disposeCommands(): void { commands?.forEach((command) => { command.dispose(); }); commands = undefined; }
  let previouslySupported = false;
  const disposeCapabilityEffect = effect(() => {
    const supported = transport !== undefined
      || (store.lspState.value === "running" && supportsConfigurationEditor(store.client.value));
    void vscode.commands.executeCommand("setContext", CONFIGURATION_EDITOR_CONTEXT, supported);
    if (supported && commands === undefined) {
      commands = [
        vscode.commands.registerCommand(CONFIGURATION_EDITOR_COMMAND, async (argument?: unknown) =>
          openConfigurationFor(controller, undefined, configurationEditorFocusRule(argument))),
        vscode.commands.registerCommand(EDIT_CONFIG_COMMAND, async (resource?: vscode.Uri) =>
          openConfigurationFor(controller, resource instanceof vscode.Uri ? resource : undefined)),
      ];
    } else if (!supported) {
      disposeCommands();
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
      disposeCommands();
      void vscode.commands.executeCommand("setContext", CONFIGURATION_EDITOR_CONTEXT, false);
    },
  };
  return { controller, disposables: [controller, capabilityLifecycle] };
}
