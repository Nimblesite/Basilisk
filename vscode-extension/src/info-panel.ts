/**
 * Basilisk Info Panel — TreeDataProvider for the Basilisk sidebar.
 *
 * Shows feature status (toggleable), quick actions, and server information
 * including uv environment status. This panel is always visible regardless
 * of workspace state.
 */

import * as vscode from "vscode";
import { type Store } from "./store";

// ── Tree node types ──────────────────────────────────────────────────────

type InfoItem = SectionItem | FeatureItem | ActionItem | InfoTextItem;

/** Section header — collapsible container for related items. */
class SectionItem extends vscode.TreeItem {
  constructor(
    public readonly section: string,
    public readonly items: InfoItem[],
  ) {
    super(section, vscode.TreeItemCollapsibleState.Expanded);
    this.contextValue = "section";
  }
}

/** Feature toggle — clicking toggles the corresponding setting. */
class FeatureItem extends vscode.TreeItem {
  constructor(
    label: string,
    public readonly settingKey: string,
    enabled: boolean,
  ) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.iconPath = enabled
      ? new vscode.ThemeIcon("check", new vscode.ThemeColor("testing.iconPassed"))
      : new vscode.ThemeIcon("circle-slash", new vscode.ThemeColor("disabledForeground"));
    this.description = enabled ? "Enabled" : "Disabled";
    this.contextValue = "feature";
    this.command = {
      command: "basilisk.toggleFeature",
      title: "Toggle Feature",
      arguments: [settingKey, !enabled],
    };
  }
}

/** Quick action — clicking executes a command. */
class ActionItem extends vscode.TreeItem {
  constructor(
    label: string,
    commandId: string,
    icon: string,
  ) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.iconPath = new vscode.ThemeIcon(icon);
    this.contextValue = "action";
    this.command = {
      command: commandId,
      title: label,
    };
  }
}

/** Read-only info text. */
class InfoTextItem extends vscode.TreeItem {
  constructor(label: string, value: string, icon?: string) {
    super(label, vscode.TreeItemCollapsibleState.None);
    this.description = value;
    if (icon !== undefined) {
      this.iconPath = new vscode.ThemeIcon(icon);
    }
    this.contextValue = "info";
  }
}

// ── Feature definitions ──────────────────────────────────────────────────

interface FeatureDef {
  readonly label: string;
  readonly settingKey: string;
}

const FEATURES: readonly FeatureDef[] = [
  { label: "Type Checking", settingKey: "basilisk.enabled" },
  { label: "Inlay Hints (Params)", settingKey: "basilisk.inlayHints.parameterNames" },
  { label: "Inlay Hints (Types)", settingKey: "basilisk.inlayHints.variableTypes" },
  { label: "Ruff Integration", settingKey: "basilisk.ruff.enabled" },
  { label: "Debugger", settingKey: "basilisk.debugger.enabled" },
  { label: "Test Explorer", settingKey: "basilisk.testExplorer.enabled" },
  { label: "uv Integration", settingKey: "basilisk.uv.enabled" },
  { label: "AI Typing", settingKey: "basilisk.aiTyping.enabled" },
];

// ── Provider ─────────────────────────────────────────────────────────────

/** Build the feature status section from current configuration. */
function buildFeatureStatusSection(): SectionItem {
  const cfg = vscode.workspace.getConfiguration();
  const items: FeatureItem[] = FEATURES.map((f) => {
    const enabled = cfg.get<boolean>(f.settingKey) ?? true;
    return new FeatureItem(f.label, f.settingKey, enabled);
  });
  return new SectionItem("Feature Status", items);
}

/** Build the quick actions section with uv actions when enabled. */
function buildQuickActionsSection(): SectionItem {
  const items: ActionItem[] = [
    new ActionItem("Restart Server", "basilisk.restartServer", "debug-restart"),
    new ActionItem("Show Output", "basilisk.showOutput", "output"),
    new ActionItem("Fix All in Workspace", "basilisk.fixWorkspace", "wand"),
    new ActionItem("Organize Imports (Workspace)", "basilisk.organizeImports", "list-ordered"),
  ];

  const uvEnabled = vscode.workspace.getConfiguration("basilisk").get<boolean>("uv.enabled") ?? true;
  if (uvEnabled) {
    items.push(
      new ActionItem("uv Sync", "basilisk.uv.sync", "sync"),
      new ActionItem("uv Add Package", "basilisk.uv.add", "add"),
      new ActionItem("uv Lock", "basilisk.uv.lock", "lock"),
      new ActionItem("uv Create Env", "basilisk.uv.createEnv", "terminal"),
    );
  }

  return new SectionItem("Quick Actions", items);
}

/** Build uv-related info items from configuration. */
function buildUvInfoItems(cfg: vscode.WorkspaceConfiguration): InfoTextItem[] {
  const uvEnabled = cfg.get<boolean>("uv.enabled") ?? true;
  const uvPath = cfg.get<string>("uv.executablePath") ?? "";
  const uvAutoSync = cfg.get<boolean>("uv.autoSync") ?? false;
  const uvStubs = cfg.get<boolean>("uv.stubSuggestions") ?? true;

  return [
    new InfoTextItem("uv", uvEnabled ? (uvPath === "" ? "auto-detect" : uvPath) : "disabled", "package"),
    new InfoTextItem("uv Auto-Sync", uvAutoSync ? "on" : "off", "sync"),
    new InfoTextItem("Stub Suggestions", uvStubs ? "on" : "off", "library"),
  ];
}

export class InfoPanelProvider implements vscode.TreeDataProvider<InfoItem>, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<InfoItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private readonly disposables: vscode.Disposable[] = [];

  constructor(private readonly store: Store) {
    this.disposables.push(
      vscode.workspace.onDidChangeConfiguration((e) => {
        if (e.affectsConfiguration("basilisk")) {
          this.emitter.fire(undefined);
        }
      }),
    );
  }

  public refresh(): void {
    this.emitter.fire(undefined);
  }

  public dispose(): void {
    for (const d of this.disposables) { d.dispose(); }
    this.emitter.dispose();
  }

  public getTreeItem(element: InfoItem): vscode.TreeItem {
    return element;
  }

  public getChildren(element?: InfoItem): InfoItem[] {
    if (element instanceof SectionItem) {
      return element.items;
    }
    if (element !== undefined) { return []; }

    return [
      buildFeatureStatusSection(),
      buildQuickActionsSection(),
      this.buildServerInfoSection(),
    ];
  }

  private buildServerInfoSection(): SectionItem {
    const lspState = this.store.lspState.value;
    const stateIcon = lspState === "running" ? "circle-filled" : "circle-slash";

    const client = this.store.client.value;
    const serverInfo = client?.initializeResult?.serverInfo;
    const cfg = vscode.workspace.getConfiguration("basilisk");

    const mode = cfg.get<string>("analysisMode") ?? "wholeModule";
    const pythonPath = cfg.get<string>("python") ?? "auto";
    const execPath = cfg.get<string>("executablePath") ?? "basilisk";

    const items: InfoTextItem[] = [
      new InfoTextItem("Server", lspState, stateIcon),
      ...(serverInfo !== undefined ? [new InfoTextItem("Version", serverInfo.version ?? "unknown", "versions")] : []),
      new InfoTextItem("Analysis Mode", mode, "symbol-keyword"),
      new InfoTextItem("Python", pythonPath === "" ? "auto-detect" : pythonPath, "symbol-namespace"),
      ...buildUvInfoItems(cfg),
      new InfoTextItem("Binary", execPath, "file-binary"),
    ];

    return new SectionItem("Server Info", items);
  }
}

// ── Registration ─────────────────────────────────────────────────────────

/**
 * Register the info panel.
 *
 * Returns an array of Disposables for command registrations that must be
 * tracked in `singletonDisposables` so `deactivate()` can dispose them
 * before re-init. Tree views and provider go to `context.subscriptions`.
 */
export function registerInfoPanel(
  context: vscode.ExtensionContext,
  store: Store,
): { provider: InfoPanelProvider; disposables: vscode.Disposable[] } {
  const provider = new InfoPanelProvider(store);

  const treeView = vscode.window.createTreeView("basilisk.info", {
    treeDataProvider: provider,
  });

  context.subscriptions.push(treeView);
  context.subscriptions.push(provider);

  // Feature toggle command.
  const disposables = [
    vscode.commands.registerCommand(
      "basilisk.toggleFeature",
      async (settingKey: string, newValue: boolean) => {
        const cfg = vscode.workspace.getConfiguration();
        await cfg.update(settingKey, newValue, vscode.ConfigurationTarget.Workspace);
      },
    ),
  ];

  return { provider, disposables };
}
