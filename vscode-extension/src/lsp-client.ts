/**
 * LSP client setup and lifecycle for the Basilisk VS Code extension.
 */

import * as vscode from "vscode";
import {
  LanguageClient,
  type LanguageClientOptions,
  type ServerOptions,
  CloseAction,
  ErrorAction,
  RevealOutputChannelOn,
  State,
} from "vscode-languageclient/node";
import { Logger } from "./logger";

let client: LanguageClient | undefined;

export function getClient(): LanguageClient | undefined {
  return client;
}

/** Read all Basilisk settings from the VS Code configuration. */
export function readBasiliskSettings(): Record<string, unknown> {
  const cfg = vscode.workspace.getConfiguration("basilisk");
  return {
    analysisMode: cfg.get<string>("analysisMode") ?? "wholeModule",
    basilisk: {
      python: cfg.get<string>("python") ?? "",
      analysisMode: cfg.get<string>("analysisMode") ?? "wholeModule",
      inlayHints: {
        parameterNames: cfg.get<boolean>("inlayHints.parameterNames") ?? true,
        variableTypes: cfg.get<boolean>("inlayHints.variableTypes") ?? true,
      },
      ruff: {
        enabled: cfg.get<boolean>("ruff.enabled") ?? true,
        executablePath: cfg.get<string>("ruff.executablePath") ?? "ruff",
      },
    },
    ruff: {
      enabled: cfg.get<boolean>("ruff.enabled") ?? true,
      executablePath: cfg.get<string>("ruff.executablePath") ?? "ruff",
    },
    uv: {
      enabled: cfg.get<boolean>("uv.enabled") ?? true,
      executablePath: cfg.get<string>("uv.executablePath") ?? "",
      autoSync: cfg.get<boolean>("uv.autoSync") ?? false,
      stubSuggestions: cfg.get<boolean>("uv.stubSuggestions") ?? true,
      dependencyDiagnostics: cfg.get<boolean>("uv.dependencyDiagnostics") ?? true,
    },
  };
}

function buildServerSettings(): Record<string, unknown> {
  return { basilisk: readBasiliskSettings() };
}

export type StatusBarUpdater = (state: "starting" | "ready" | "error" | "stopped") => void;

interface LspClientOptions {
  context: vscode.ExtensionContext;
  executablePath: string;
  outputChannel: vscode.OutputChannel | undefined;
}

export function startLspClient(
  options: LspClientOptions,
  updateStatusBar: StatusBarUpdater
): void {
  const { context, executablePath, outputChannel } = options;
  const serverOptions: ServerOptions = {
    command: executablePath,
    args: ["lsp"],
  };

  const traceChannel = vscode.window.createOutputChannel("Basilisk LSP Trace");
  context.subscriptions.push(traceChannel);

  const clientOptions = buildClientOptions(outputChannel, traceChannel, updateStatusBar);

  client = new LanguageClient(
    "basilisk",
    "Basilisk Type Checker",
    serverOptions,
    clientOptions
  );

  updateStatusBar("starting");
  registerStateHandler(updateStatusBar);
  registerConfigForwarding(context);
  registerTabTracking(context);

  client.start().catch((error: unknown) => {
    const errorMessage = error instanceof Error ? error.message : String(error);
    const msg =
      `Basilisk: Failed to start language server. ` +
      `Is '${executablePath}' installed and on PATH? ${errorMessage}`;
    vscode.window.showErrorMessage(msg);
    Logger.error(msg);
    updateStatusBar("error");
  });

  context.subscriptions.push(client);
}

function buildClientOptions(
  outputCh: vscode.OutputChannel | undefined,
  traceCh: vscode.OutputChannel,
  updateStatusBar: StatusBarUpdater
): LanguageClientOptions {
  return {
    documentSelector: [{ scheme: "file", language: "python" }],
    synchronize: {
      configurationSection: "basilisk",
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.{py,pyi}"),
    },
    initializationOptions: readBasiliskSettings(),
    traceOutputChannel: traceCh,
    outputChannel: outputCh,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    errorHandler: {
      error: (error, _message, count) => {
        Logger.error(`LSP error: ${error.message ?? error}`);
        if (count !== undefined && count < 3) {
          return { action: ErrorAction.Continue };
        }
        updateStatusBar("error");
        return { action: ErrorAction.Shutdown };
      },
      closed: () => {
        Logger.warn("LSP connection closed. Restarting...");
        return { action: CloseAction.Restart };
      },
    },
    middleware: {
      executeCommand: async (command, args, next) => {
        return await next(command, args) as unknown;
      },
      workspace: {
        configuration: async (params, token, next) => {
          const results = await next(params, token);
          if (!Array.isArray(results)) {
            return results as unknown as Record<string, unknown>[];
          }
          return (results as unknown[]).map((item: unknown, idx: number) => {
            const section = params.items[idx]?.section;
            if (section === "basilisk" || section?.startsWith("basilisk.")) {
              return {
                ...(typeof item === "object" && item !== null ? item as Record<string, unknown> : {}),
                ...readBasiliskSettings(),
              } as Record<string, unknown>;
            }
            return item as Record<string, unknown>;
          });
        },
      },
    },
  };
}

function registerStateHandler(
  updateStatusBar: StatusBarUpdater
): void {
  if (!client) {return;}
  client.onDidChangeState((event) => {
    switch (event.newState) {
      case State.Running:
        Logger.info("Basilisk language server is running.");
        updateStatusBar("ready");
        break;
      case State.Stopped:
        Logger.info("Basilisk language server stopped.");
        updateStatusBar("stopped");
        break;
      case State.Starting:
        Logger.info("Basilisk language server is starting...");
        updateStatusBar("starting");
        break;
    }
  });
}

function registerConfigForwarding(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("basilisk") && client?.isRunning()) {
        void client.sendNotification("workspace/didChangeConfiguration", {
          settings: buildServerSettings(),
        });
      }
    })
  );
}

function registerTabTracking(context: vscode.ExtensionContext): void {
  let knownOpenUris = collectOpenPythonUris();

  context.subscriptions.push(
    vscode.window.tabGroups.onDidChangeTabs(() => {
      if (!client?.isRunning()) {return;}

      const currentUris = collectOpenPythonUris();

      const mode = vscode.workspace.getConfiguration("basilisk").get<string>("analysisMode") ?? "wholeModule";
      if (mode === "openFilesOnly") {
        for (const uriStr of knownOpenUris) {
          if (!currentUris.has(uriStr)) {
            void client.sendNotification("textDocument/didClose", {
              textDocument: { uri: vscode.Uri.parse(uriStr).toString() },
            });
          }
        }
      }

      knownOpenUris = currentUris;
    })
  );
}

function collectOpenPythonUris(): Set<string> {
  const uris = new Set<string>();
  for (const group of vscode.window.tabGroups.all) {
    for (const tab of group.tabs) {
      const input = tab.input;
      if (input instanceof vscode.TabInputText) {
        if (input.uri.scheme === "file" && input.uri.fsPath.endsWith(".py")) {
          uris.add(input.uri.toString());
        }
      }
    }
  }
  return uris;
}
