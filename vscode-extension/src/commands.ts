/**
 * Command registration for the Basilisk VS Code extension.
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

/** Send an LSP executeCommand if the client is running. */
async function lspExecute(command: string, args: unknown[] = []): Promise<void> {
  const lspClient = getClient();
  if (!lspClient) {
    vscode.window.showWarningMessage("Basilisk: LSP client is not running.");
    return;
  }
  await lspClient.sendRequest("workspace/executeCommand", {
    command,
    arguments: args,
  });
}

export function registerAllCommands(
  context: vscode.ExtensionContext,
  outputChannel: vscode.OutputChannel | undefined
): void {
  registerCoreCommands(context, outputChannel);
  registerFixCommands(context);
  registerAdoptCommands(context);
  registerUvCommands(context);
}

function registerCoreCommands(
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

function registerFixCommands(context: vscode.ExtensionContext): void {
  safeRegisterCommand(context, "basilisk.fixFile", async () => {
    const editor = vscode.window.activeTextEditor;
    if (editor?.document.uri.scheme !== "file") {return;}

    const lspClient = getClient();
    if (lspClient) {
      await lspClient.sendRequest("workspace/executeCommand", {
        command: "basilisk.fixFile",
        arguments: [editor.document.uri.toString()],
      });
    } else {
      await vscode.commands.executeCommand("editor.action.fixAll");
    }
  });

  safeRegisterCommand(context, "basilisk.fixWorkspace", async () => {
    await lspExecute("basilisk.fixWorkspace");
  });
}

function registerAdoptCommands(context: vscode.ExtensionContext): void {
  safeRegisterCommand(context, "basilisk.adoptFile", async () => {
    const editor = vscode.window.activeTextEditor;
    if (editor?.document.uri.scheme !== "file") {return;}
    await lspExecute("basilisk.adoptFile", [editor.document.uri.toString()]);
  });

  safeRegisterCommand(context, "basilisk.adoptWorkspace", async () => {
    await lspExecute("basilisk.adoptWorkspace");
  });

  safeRegisterCommand(context, "basilisk.unadoptFile", async () => {
    const editor = vscode.window.activeTextEditor;
    if (editor?.document.uri.scheme !== "file") {return;}
    await lspExecute("basilisk.unadoptFile", [editor.document.uri.toString()]);
  });
}

function registerUvCommands(context: vscode.ExtensionContext): void {
  safeRegisterCommand(context, "basilisk.uv.sync", async () => {
    await lspExecute("basilisk.uv.sync");
    vscode.window.showInformationMessage("Basilisk: uv sync complete.");
  });

  safeRegisterCommand(context, "basilisk.uv.add", async () => {
    const packageName = await vscode.window.showInputBox({
      prompt: "Package name to add",
      placeHolder: "e.g. requests",
    });
    if (packageName === undefined || packageName === "") {return;}
    await lspExecute("basilisk.uv.add", [{ package: packageName }]);
    vscode.window.showInformationMessage(`Basilisk: Added ${packageName}.`);
  });

  safeRegisterCommand(context, "basilisk.uv.addDev", async () => {
    const packageName = await vscode.window.showInputBox({
      prompt: "Dev package name to add",
      placeHolder: "e.g. pytest",
    });
    if (packageName === undefined || packageName === "") {return;}
    await lspExecute("basilisk.uv.addDev", [{ package: packageName }]);
    vscode.window.showInformationMessage(`Basilisk: Added dev dependency ${packageName}.`);
  });

  safeRegisterCommand(context, "basilisk.uv.remove", async () => {
    const packageName = await vscode.window.showInputBox({
      prompt: "Package name to remove",
      placeHolder: "e.g. requests",
    });
    if (packageName === undefined || packageName === "") {return;}
    await lspExecute("basilisk.uv.remove", [{ package: packageName }]);
    vscode.window.showInformationMessage(`Basilisk: Removed ${packageName}.`);
  });

  safeRegisterCommand(context, "basilisk.uv.lock", async () => {
    await lspExecute("basilisk.uv.lock");
    vscode.window.showInformationMessage("Basilisk: uv lock complete.");
  });

  safeRegisterCommand(context, "basilisk.uv.createEnv", async () => {
    await lspExecute("basilisk.uv.createEnv");
    vscode.window.showInformationMessage("Basilisk: Virtual environment created.");
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
