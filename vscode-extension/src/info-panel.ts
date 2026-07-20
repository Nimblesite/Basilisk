// Implements [EXTACT-INFO]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO
/**
 * Basilisk Info Panel — TreeDataProvider for the Basilisk sidebar.
 *
 * Slimmed per issue #103: one analyzer toggle followed by flat, read-only
 * server details. Read-only data does not need collapsible tree structure.
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
import type { LanguageClient } from "vscode-languageclient/node";
import { type Store } from "./store";
import type { TypeshedStatusState } from "./configuration-editor-model";

// ── Tree node types ──────────────────────────────────────────────────────

type InfoItem = FeatureItem | InfoTextItem;

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
//   - Analyzer (basilisk.enabled): the LSP is authoritative for diagnostics
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
  { label: "Analyzer", settingKey: "basilisk.enabled" },
];

// ── Resolved environment ([LSPARCH-RESOLVED-ENV]) ────────────────────────

/** One resolved tool from the server's initialize payload. */
interface ResolvedTool {
  readonly path: string;
  readonly version?: string | null;
}

/** The server-resolved python/uv/binary environment (`null` = none found). */
interface ResolvedEnvironment {
  readonly python?: ResolvedTool | null;
  readonly uv?: ResolvedTool | null;
  readonly binary?: ResolvedTool | null;
}

/**
 * Read the resolved environment the server reported in its initialize
 * response ([LSPARCH-RESOLVED-ENV]) — the LSP is authoritative for what
 * auto-detection found (issue #153); the extension never re-derives it.
 * `undefined` while no live server data exists (starting/stopped).
 */
function resolvedEnvironment(client: LanguageClient | undefined): ResolvedEnvironment | undefined {
  const experimental = client?.initializeResult?.capabilities.experimental as
    | { basilisk?: { resolvedEnvironment?: ResolvedEnvironment } }
    | undefined;
  return experimental?.basilisk?.resolvedEnvironment;
}

/** Render a tool as `version (path)` — bare path when the probe failed. */
function formatResolvedTool(tool: ResolvedTool): string {
  const { path, version } = tool;
  return version !== undefined && version !== null && version !== "" ? `${version} (${path})` : path;
}

/**
 * Row value for a setting whose `""` default means auto-detect (issue #153):
 * an explicit setting shows what the server resolved it to (the raw setting
 * before the server is up, an explicit `none found (…)` when the server
 * couldn't use it), and auto-detect always shows its outcome —
 * `auto-detect → <version> (<path>)`, `→ none found` on failure, or
 * `→ awaiting server…` before any authoritative data exists. The bare
 * placeholder never renders.
 */
function resolutionDescription(
  configured: string,
  env: ResolvedEnvironment | undefined,
  key: "python" | "uv",
): string {
  const tool = env?.[key];
  if (configured !== "") {
    if (env === undefined) { return configured; }
    return tool === undefined || tool === null
      ? `none found (${configured})`
      : formatResolvedTool(tool);
  }
  if (env === undefined) {
    return "auto-detect → awaiting server…";
  }
  return tool === undefined || tool === null
    ? "auto-detect → none found"
    : `auto-detect → ${formatResolvedTool(tool)}`;
}

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
 * Build the single compact uv info row. The verbose sub-settings (auto-sync,
 * stub suggestions) live in the tooltip, not as their own rows (issue #103).
 * The row value is the RESOLVED uv binary from the server, never the bare
 * `auto-detect` placeholder (issue #153).
 *
 * Implements the one-uv-row rule of [EXTACT-INFO-SERVER-INFO].
 */
function buildUvInfoItem(
  cfg: vscode.WorkspaceConfiguration,
  env: ResolvedEnvironment | undefined,
): InfoTextItem {
  const uvEnabled = cfg.get<boolean>("uv.enabled") ?? true;
  const uvPath = cfg.get<string>("uv.executablePath") ?? "";
  const uvAutoSync = cfg.get<boolean>("uv.autoSync") ?? false;

  const item = new InfoTextItem(
    "uv",
    uvEnabled ? resolutionDescription(uvPath, env, "uv") : "disabled",
    "package",
  );
  item.tooltip = [
    `uv Integration: ${uvEnabled ? "enabled" : "disabled"}`,
    `Executable: ${uvPath === "" ? "auto-detect" : uvPath}`,
    `Resolved: ${resolutionDescription(uvPath, env, "uv")}`,
    `Auto-Sync: ${uvAutoSync ? "on" : "off"}`,
  ].join("\n");
  return item;
}

function statusKind(value: { readonly kind: string } | undefined): string {
  return value?.kind ?? "Pending";
}

function typeshedLifecycleIcon(status: TypeshedStatusState): string {
  if (status.lifecycle.kind === "Acquiring") { return "loading~spin"; }
  if (status.lifecycle.kind === "Blocked") { return "error"; }
  return "database";
}

function rootLabel(rootUri: string): string {
  try {
    const parts = new URL(rootUri).pathname.split("/").filter((part) => part !== "");
    return decodeURIComponent(parts[parts.length - 1] ?? rootUri);
  } catch {
    return rootUri;
  }
}

function typeshedTooltip(rootUri: string, status: TypeshedStatusState): string {
  return [
    `Root: ${rootUri}`,
    `State: ${statusKind(status.lifecycle)}`,
    `Source: ${statusKind(status.activeSource)}`,
    `Commit: ${status.commitIdentity ?? "not available"}`,
    `Transport: ${statusKind(status.transport)}`,
    `Provenance: ${statusKind(status.provenance)}`,
    `Signed release: ${status.signedRelease ? "yes" : "no"}`,
    `License: ${statusKind(status.licenseStatus)}`,
  ].join("\n");
}

function typeshedSourceItem(
  prefix: string,
  rootUri: string,
  status: TypeshedStatusState,
): InfoTextItem {
  const commit = status.commitIdentity === undefined ? "" : ` · ${status.commitIdentity}`;
  const item = new InfoTextItem(
    `${prefix} Source`,
    `${statusKind(status.activeSource)}${commit}`,
    "repo",
  );
  item.tooltip = typeshedTooltip(rootUri, status);
  return item;
}

function typeshedInfoItems(
  statuses: ReadonlyMap<string, TypeshedStatusState>,
): InfoTextItem[] {
  const entries = [...statuses.entries()].sort(([left], [right]) => left.localeCompare(right));
  return entries.flatMap(([rootUri, status]) => {
    const prefix = entries.length === 1 ? "Typeshed" : `Typeshed (${rootLabel(rootUri)})`;
    const rows = [
      new InfoTextItem(
        `${prefix} State`,
        statusKind(status.lifecycle),
        typeshedLifecycleIcon(status),
      ),
      typeshedSourceItem(prefix, rootUri, status),
      ...(status.blockedReason === undefined
        ? []
        : [new InfoTextItem(`${prefix} Blocked`, status.blockedReason, "error")]),
      ...status.warnings.map((warning) => new InfoTextItem(
        `${prefix} ${warning.code}`,
        warning.message,
        statusKind(warning.severity) === "High" ? "warning" : "info",
      )),
    ];
    return rows;
  });
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
      void this.store.typeshedStatuses.value;
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
    if (element !== undefined) { return []; }

    // Implements [EXTACT-INFO-STRUCTURE] / [EXTACT-INFO-QUICK-ACTIONS]: flat
    // layout — the analyzer toggle, then compact read-only server details. No
    // Quick Actions section: those are Modules-toolbar buttons, the status bar,
    // and the command palette.
    return [
      ...buildFeatureToggles(),
      ...this.buildServerInfoItems(),
    ];
  }

  // Implements [EXTACT-INFO-SERVER-INFO]: compact read-only Version / Analysis
  // Mode / Python / uv / Binary rows; no live server-state row. Python, uv,
  // and Binary render the server-RESOLVED values from [LSPARCH-RESOLVED-ENV]
  // (issue #153) — never a bare `auto-detect` placeholder or a blank row.
  private buildServerInfoItems(): InfoTextItem[] {
    // No live "Server" state row: the status bar already shows it (issue
    // #103). The lspState/client effect in the constructor still re-renders
    // this section so the Version row appears as soon as the server is up.
    const client = this.store.client.value;
    const serverInfo = client?.initializeResult?.serverInfo;
    const env = resolvedEnvironment(client);
    const cfg = vscode.workspace.getConfiguration("basilisk");

    const mode = cfg.get<string>("analysisMode") ?? "wholeModule";
    const pythonPath = cfg.get<string>("python") ?? "";
    const binary = env?.binary;

    const items: InfoTextItem[] = [
      ...(serverInfo !== undefined ? [new InfoTextItem("Version", serverInfo.version ?? "unknown", "versions")] : []),
      new InfoTextItem("Analysis Mode", mode, "symbol-keyword"),
      new InfoTextItem("Python", resolutionDescription(pythonPath, env, "python"), "symbol-namespace"),
      buildUvInfoItem(cfg, env),
      // The running server binary, from the server itself (current_exe) — the
      // one source that cannot lie about which basilisk is answering. Absent
      // (never blank) while no server is running (issue #153 defect 3).
      ...(binary !== undefined && binary !== null
        ? [new InfoTextItem("Binary", formatResolvedTool(binary), "file-binary")]
        : []),
      ...typeshedInfoItems(this.store.typeshedStatuses.value),
    ];

    return items;
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
