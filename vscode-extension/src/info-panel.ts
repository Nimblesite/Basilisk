// Implements [EXTACT-INFO]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO
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

/**
 * Feature toggle — clicking toggles the corresponding setting.
 *
 * Actionable affordance per [EXTACT-INFO-AFFORDANCE]: carries a `command`, an
 * imperative tooltip, and a toggle-state icon. Paired with the inline action
 * button contributed for `viewItem == feature`.
 */
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
    this.tooltip = `Click to ${enabled ? "disable" : "enable"} ${label}`;
    this.contextValue = "feature";
    this.command = {
      command: "basilisk.toggleFeature",
      title: "Toggle Feature",
      arguments: [settingKey, !enabled],
    };
  }
}

/** Definition of a quick action — the single source of truth for a row. */
interface ActionDef {
  readonly label: string;
  readonly commandId: string;
  /** Codicon id connoting the action (verb/tool). */
  readonly icon: string;
  /** Imperative tooltip describing the effect. */
  readonly tooltip: string;
}

/**
 * Quick action — clicking executes a command.
 *
 * Actionable affordance per [EXTACT-INFO-AFFORDANCE]: carries a `command`, an
 * imperative tooltip describing the effect, and a verb/tool icon. Paired with
 * the inline action button contributed for `viewItem == action`.
 */
class ActionItem extends vscode.TreeItem {
  constructor(def: ActionDef) {
    super(def.label, vscode.TreeItemCollapsibleState.None);
    this.iconPath = new vscode.ThemeIcon(def.icon);
    this.tooltip = def.tooltip;
    this.contextValue = "action";
    this.command = {
      command: def.commandId,
      title: def.label,
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

// Only features whose toggle has a real, observable effect belong here.
// A toggle that writes a setting the server (or extension) never reads is
// a lie to the user and must not exist. Audited 2026-05-30:
//   - Type Checking (basilisk.enabled): gates diagnostic publication in
//     extension.ts `checkDocument` — disabling clears diagnostics.
//   - uv Integration (basilisk.uv.enabled): gates the uv Quick Actions and
//     uv Server Info rows below (buildQuickActionsSection / buildUvInfoItems).
// Removed because the setting was silently dropped by the LSP server (it only
// parses analysisMode + testExplorer in did_change_configuration) and nothing
// else read it: Inlay Hints (Params/Types), Ruff Integration, Test Explorer,
// Debugger (never even declared), AI Typing. See EXTACT-INFO-FEATURE-STATUS.
const FEATURES: readonly FeatureDef[] = [
  { label: "Type Checking", settingKey: "basilisk.enabled" },
  { label: "uv Integration", settingKey: "basilisk.uv.enabled" },
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

// Quick Actions — single source of truth, partitioned by how each handler is
// wired so a row can be gated on liveness per [EXTACT-INFO-ACTION-WIRING] (a row
// may only appear if its command resolves to a live handler).

// Client-core actions — registered client-side and always offered so the user
// can recover a stopped server (restart) and read its log.
const CLIENT_ACTIONS: readonly ActionDef[] = [
  { label: "Restart Server", commandId: "basilisk.restartServer", icon: "debug-restart", tooltip: "Restart the Basilisk language server" },
  { label: "Show Output", commandId: "basilisk.showOutput", icon: "output", tooltip: "Show the Basilisk output log" },
];

// Server-advertised actions — their handlers exist only while the LSP is Running
// (store.syncServerCommands). Shown only when the server actually advertises
// them; otherwise hidden (not shown-but-dead) so clicking can never raise
// "command not found", per [EXTACT-INFO-ACTION-WIRING].
const SERVER_ACTIONS: readonly ActionDef[] = [
  { label: "Fix All in Workspace", commandId: "basilisk.fixWorkspace", icon: "wand", tooltip: "Apply all safe autofixes across the workspace" },
  { label: "Organize Imports (Workspace)", commandId: "basilisk.organizeImports", icon: "list-ordered", tooltip: "Organize imports across the workspace" },
];

// uv actions — server-advertised AND additionally gated on uv Integration being
// enabled. Hidden when uv is off or the server is down, per
// [EXTACT-INFO-ACTION-WIRING].
const UV_ACTIONS: readonly ActionDef[] = [
  { label: "uv Sync", commandId: "basilisk.uv.sync", icon: "sync", tooltip: "Run uv sync to update the environment" },
  { label: "uv Add Package", commandId: "basilisk.uv.add", icon: "add", tooltip: "Run uv add to add a dependency" },
  { label: "uv Lock", commandId: "basilisk.uv.lock", icon: "lock", tooltip: "Run uv lock to update the lock file" },
  { label: "uv Create Env", commandId: "basilisk.uv.createEnv", icon: "terminal", tooltip: "Run uv venv to create a virtual environment" },
];

/** Build the quick actions section, hiding any action whose handler is not live. */
function buildQuickActionsSection(store: Store): SectionItem {
  const uvEnabled = vscode.workspace.getConfiguration("basilisk").get<boolean>("uv.enabled") ?? true;
  function advertised(def: ActionDef): boolean {
    return store.isServerCommandAdvertised(def.commandId);
  }
  const serverActions = SERVER_ACTIONS.filter(advertised);
  const uvActions = uvEnabled ? UV_ACTIONS.filter(advertised) : [];
  const defs = [...CLIENT_ACTIONS, ...serverActions, ...uvActions];
  return new SectionItem("Quick Actions", defs.map((def) => new ActionItem(def)));
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
      buildQuickActionsSection(this.store),
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
    // Inline action-button dispatcher [EXTACT-INFO-AFFORDANCE]. The literal
    // button contributed for actionable rows (viewItem == action|feature)
    // invokes this with the clicked row; it forwards to the row's own command
    // so the button and the whole-row click share a single source of truth.
    vscode.commands.registerCommand(
      "basilisk.info.runAction",
      async (item: vscode.TreeItem | undefined) => {
        const command = item?.command;
        if (command === undefined) { return; }
        const commandArgs: unknown[] = command.arguments ?? [];
        await vscode.commands.executeCommand(command.command, ...commandArgs);
      },
    ),
  ];

  return { provider, disposables };
}
