// Implements [EXTACT-INFO]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO
/**
 * Basilisk Info Panel — TreeDataProvider for the Basilisk sidebar.
 *
 * Slimmed per issue #103: feature toggles at the top level (two toggles do
 * not justify a section header) plus a compact read-only Server Info section.
 * Quick actions live elsewhere — Fix All / Organize Imports / Restart are
 * toolbar buttons on the Modules panel (when-gated on the server running),
 * Show Output is the status-bar click action, and everything remains in the
 * command palette. The live server state row was dropped: the status bar
 * already shows it.
 *
 * This panel is always visible regardless of workspace state.
 */

import * as vscode from "vscode";
import { effect } from "@preact/signals-core";
import { type Store } from "./store";

// ── Tree node types ──────────────────────────────────────────────────────

type InfoItem = SectionItem | FeatureItem | InfoTextItem;

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

// Implements the read-only affordance of [EXTACT-INFO-AFFORDANCE] /
// [EXTACT-INFO-SERVER-INFO]: contextValue `info`, no command, no inline button,
// value shown in the row description.
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

// Implements [EXTACT-INFO-FEATURE-STATUS]: only features whose toggle has a
// real, observable effect belong here. A toggle that writes a setting the server
// (or extension) never reads is a lie to the user and must not exist.
//   - Type Checking (basilisk.enabled): the LSP is authoritative for diagnostics
//     and honours this setting — disabling clears published diagnostics and
//     suppresses new ones; re-enabling re-scans. See [ANALYSIS-ENABLED]
//     (crates/basilisk-lsp/src/server/init.rs) and GitHub #65 / #119.
// Removed because the setting is silently dropped by the LSP server (it parses
// analysisMode + testExplorer + enabled in did_change_configuration) and nothing
// else acts on it:
//   - uv Integration (basilisk.uv.enabled): NO server code reads it, so the
//     toggle never disabled uv integration — a no-op affordance. Removed per
//     GitHub #190; the uv commands remain in the palette / code actions and the
//     read-only "uv" Server Info row still reports uv status.
//   - Inlay Hints (Params/Types), Ruff Integration, Test Explorer, Debugger
//     (never even declared), AI Typing — all dropped server-side likewise.
// See EXTACT-INFO-FEATURE-STATUS.
const FEATURES: readonly FeatureDef[] = [
  { label: "Type Checking", settingKey: "basilisk.enabled" },
];

// ── Provider ─────────────────────────────────────────────────────────────

/**
 * Build the top-level feature toggles. Rendered directly at the root — two
 * toggles do not justify a "Feature Status" section header (issue #103).
 *
 * Implements [EXTACT-INFO-STRUCTURE] (toggles at root, no section header) and
 * the actionable half of [EXTACT-INFO-FEATURE-STATUS].
 */
function buildFeatureToggles(): FeatureItem[] {
  const cfg = vscode.workspace.getConfiguration();
  return FEATURES.map((f) => {
    const enabled = cfg.get<boolean>(f.settingKey) ?? true;
    return new FeatureItem(f.label, f.settingKey, enabled);
  });
}

/**
 * Build the single compact uv info row. Auto-sync lives in the tooltip rather
 * than as its own row (issue #103).
 *
 * Implements the one-uv-row rule of [EXTACT-INFO-SERVER-INFO].
 */
function buildUvInfoItem(cfg: vscode.WorkspaceConfiguration): InfoTextItem {
  const uvEnabled = cfg.get<boolean>("uv.enabled") ?? true;
  const uvPath = cfg.get<string>("uv.executablePath") ?? "";
  const uvAutoSync = cfg.get<boolean>("uv.autoSync") ?? false;

  const item = new InfoTextItem(
    "uv",
    uvEnabled ? (uvPath === "" ? "auto-detect" : uvPath) : "disabled",
    "package",
  );
  item.tooltip = [
    `uv Integration: ${uvEnabled ? "enabled" : "disabled"}`,
    `Executable: ${uvPath === "" ? "auto-detect" : uvPath}`,
    `Auto-Sync: ${uvAutoSync ? "on" : "off"}`,
  ].join("\n");
  return item;
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
    // Implements the freshness rule of [EXTACT-INFO-STRUCTURE] /
    // [EXTACT-INFO-SERVER-INFO] — defect 3 (issue #103): Server Info must not go
    // stale. Re-render whenever the LSP lifecycle signals change (e.g. the
    // Version row appears once the client's initializeResult arrives) — same
    // signals pattern as bindLspStateEffects in lsp-client.ts.
    const disposeEffect = effect(() => {
      // Subscribe to both signals; the values themselves are read in
      // buildServerInfoSection on the re-render this triggers.
      void this.store.lspState.value;
      void this.store.client.value;
      this.emitter.fire(undefined);
    });
    this.disposables.push({ dispose: disposeEffect });
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

    // Implements [EXTACT-INFO-STRUCTURE] / [EXTACT-INFO-QUICK-ACTIONS]: slimmed
    // layout (issue #103) — toggles at the root, then compact Server Info. No
    // Quick Actions section: those are Modules-toolbar buttons, the status bar,
    // and the command palette.
    return [
      ...buildFeatureToggles(),
      this.buildServerInfoSection(),
    ];
  }

  // Implements [EXTACT-INFO-SERVER-INFO]: compact read-only Version / Analysis
  // Mode / Python / uv / Binary rows; no live server-state row.
  private buildServerInfoSection(): SectionItem {
    // No live "Server" state row: the status bar already shows it (issue
    // #103). The lspState/client effect in the constructor still re-renders
    // this section so the Version row appears as soon as the server is up.
    const client = this.store.client.value;
    const serverInfo = client?.initializeResult?.serverInfo;
    const cfg = vscode.workspace.getConfiguration("basilisk");

    const mode = cfg.get<string>("analysisMode") ?? "wholeModule";
    const pythonPath = cfg.get<string>("python") ?? "auto";
    const execPath = cfg.get<string>("executablePath") ?? "basilisk";

    const items: InfoTextItem[] = [
      ...(serverInfo !== undefined ? [new InfoTextItem("Version", serverInfo.version ?? "unknown", "versions")] : []),
      new InfoTextItem("Analysis Mode", mode, "symbol-keyword"),
      new InfoTextItem("Python", pythonPath === "" ? "auto-detect" : pythonPath, "symbol-namespace"),
      buildUvInfoItem(cfg),
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
        const target = featureToggleTarget(vscode.workspace.workspaceFolders?.length ?? 0);
        await cfg.update(settingKey, newValue, target);
      },
    ),
    // Inline action-button dispatcher [EXTACT-INFO-AFFORDANCE]. The literal
    // button contributed for feature-toggle rows (viewItem == feature)
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

/**
 * Configuration target for feature toggles — defect 2 of issue #103.
 *
 * Implements the write-target rule of [EXTACT-INFO-STRUCTURE] /
 * [EXTACT-INFO-FEATURE-STATUS]: Workspace when a folder is open, else Global.
 *
 * The info panel is always visible (`visibility: "visible"`, no `when`), so
 * toggles can be clicked with no folder open. Writing to
 * `ConfigurationTarget.Workspace` is invalid without a workspace folder and
 * rejects the update, so fall back to `Global` in that case. Pure in the
 * folder count so both branches are testable in the e2e host (which always
 * launches with a folder).
 */
export function featureToggleTarget(workspaceFolderCount: number): vscode.ConfigurationTarget {
  return workspaceFolderCount > 0
    ? vscode.ConfigurationTarget.Workspace
    : vscode.ConfigurationTarget.Global;
}
