/**
 * Command registration for the Basilisk VS Code extension.
 *
 * Only CLIENT-ONLY commands are registered here (restartServer, showOutput).
 * Server-advertised commands (fix, adopt, uv, organizeImports) are discovered
 * from the server's executeCommandProvider capabilities — the LSP client
 * library registers them automatically. Client-side UI for those commands
 * (editor URI injection, input prompts, toasts) lives in lsp-client.ts
 * middleware.
 *
 * See LSP-ARCHITECTURE-SPEC.md § Command Registration Rule.
 */

import * as vscode from "vscode";
import { execFile } from "child_process";
import * as path from "path";
import { Logger } from "./logger";
import { getClient, readBasiliskSettings } from "./lsp-client";

/** Registered command IDs so we can avoid double-registering on re-activation. */
const registeredCommands = new Set<string>();

function safeRegisterCommand(
  context: vscode.ExtensionContext,
  commandId: string,
  handler: (...args: unknown[]) => unknown
): void {
  if (registeredCommands.has(commandId)) {
    return;
  }
  const disposable = vscode.commands.registerCommand(commandId, handler);
  context.subscriptions.push(disposable);
  registeredCommands.add(commandId);
}

/** Register client-only commands (not advertised by the LSP server). */
export function registerClientCommands(
  context: vscode.ExtensionContext,
  outputChannel: vscode.OutputChannel | undefined
): void {
  safeRegisterCommand(context, "basilisk.restartServer", async () => {
    const lspClient = getClient();
    if (!lspClient) {
      vscode.window.showWarningMessage("Basilisk: No language server to restart.");
      return;
    }
    try {
      Logger.info("Restarting Basilisk language server...");
      await lspClient.stop();
      await lspClient.start();
      Logger.info("Basilisk language server restarted.");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      Logger.error(`Restart failed: ${msg}`);
      vscode.window.showErrorMessage(`Basilisk: Failed to restart server: ${msg}`);
    }
  });

  safeRegisterCommand(context, "basilisk.showOutput", () => {
    outputChannel?.show();
  });
}

/** Register organizeImports for subprocess mode (ruff CLI). */
export function registerOrganizeImportsCommand(
  context: vscode.ExtensionContext,
  workspaceRoot: () => string | undefined
): void {
  safeRegisterCommand(context, "basilisk.organizeImports", () => {
    const editor = vscode.window.activeTextEditor;
    if (editor?.document.languageId !== "python") {
      vscode.window.showWarningMessage("Basilisk: Open a Python file to organize imports.");
      return;
    }

    const settings = readBasiliskSettings() as { ruff?: { enabled?: boolean; executablePath?: string } };
    if (!settings.ruff?.enabled) {
      vscode.window.showWarningMessage("Basilisk: Ruff integration is disabled.");
      return;
    }

    const ruffPath = settings.ruff?.executablePath ?? "ruff";
    const filePath = editor.document.uri.fsPath;

    execFile(
      ruffPath,
      ["check", "--select", "I", "--fix", filePath],
      { cwd: workspaceRoot() },
      (error, _stdout, stderr) => {
        if (error && typeof error.code === "number" && error.code > 1) {
          vscode.window.showWarningMessage(
            `Basilisk: Failed to run ruff. Is '${ruffPath}' on PATH? (${error.message})`
          );
          Logger.error(`organizeImports error: ${stderr}`);
          return;
        }
        Logger.info(`Imports organized in ${path.basename(filePath)}`);
      }
    );
  });
}
