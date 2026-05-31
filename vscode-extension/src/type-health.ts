// Implements [EXTACT-HEALTH]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-HEALTH
/**
 * Type Health — TreeDataProvider for the Basilisk sidebar.
 *
 * Shows per-module type coverage statistics, error/warning counts, and
 * adoption status. Data is fetched from the LSP server via the
 * `basilisk.typeHealth` command.
 */

import * as vscode from "vscode";
import { type Store } from "./store";
import { Logger } from "./logger";

// ── LSP response types ───────────────────────────────────────────────────

interface HealthStats {
  readonly totalSymbols: number;
  readonly annotatedSymbols: number;
  readonly coveragePercent: number;
  readonly errors: number;
  readonly warnings: number;
  readonly adoptedFiles: number;
  readonly totalFiles: number;
}

interface ModuleHealth {
  readonly name: string;
  readonly path: string;
  readonly coveragePercent: number;
  readonly errors: number;
  readonly warnings: number;
  readonly adopted: boolean;
  readonly unannotated: readonly string[];
}

interface TypeHealthResponse {
  readonly workspace: HealthStats;
  readonly modules: readonly ModuleHealth[];
}

/** Maximum unannotated symbols to show in tooltip before truncating. */
const MAX_UNANNOTATED_TOOLTIP = 10;

// ── Sort modes ───────────────────────────────────────────────────────────

type SortMode = "worst" | "best" | "alpha";

const SORT_CYCLE: readonly SortMode[] = ["worst", "best", "alpha"];

// ── Tree items ───────────────────────────────────────────────────────────

type TreeItem = SummaryItem | ModuleHealthItem;

export class SummaryItem extends vscode.TreeItem {
  constructor(stats: HealthStats) {
    super("Workspace Coverage", vscode.TreeItemCollapsibleState.None);
    if (stats.totalFiles === 0) {
      // [EXTACT-HEALTH] Empty workspace (no Python files): render an explicit
      // empty state instead of a misleading 100% coverage bar / green "pass"
      // icon for 0/0 symbols (issue #57).
      this.description = "No Python files found";
      this.tooltip = "Basilisk found no Python files to measure in this workspace";
      this.iconPath = new vscode.ThemeIcon("info");
      this.contextValue = "summaryEmpty";
      return;
    }
    this.description = `${coverageBar(stats.coveragePercent)} ${stats.coveragePercent}% — ${stats.annotatedSymbols}/${stats.totalSymbols} symbols`;
    this.tooltip = [
      `Coverage: ${stats.coveragePercent}%`,
      `Symbols: ${stats.annotatedSymbols}/${stats.totalSymbols} annotated`,
      `Errors: ${stats.errors}`,
      `Warnings: ${stats.warnings}`,
      `Files: ${stats.totalFiles} (${stats.adoptedFiles} adopted)`,
    ].join("\n");
    this.iconPath = coverageIcon(stats.coveragePercent);
    this.contextValue = "summary";
  }
}

class ModuleHealthItem extends vscode.TreeItem {
  constructor(public readonly health: ModuleHealth) {
    super(health.name, vscode.TreeItemCollapsibleState.None);
    const badge = health.adopted ? " [adopted]" : "";
    const issues = [];
    if (health.errors > 0) { issues.push(`${health.errors}E`); }
    if (health.warnings > 0) { issues.push(`${health.warnings}W`); }
    const issueStr = issues.length > 0 ? ` — ${issues.join(" ")}` : "";
    this.description = `${coverageBar(health.coveragePercent)} ${health.coveragePercent}%${issueStr}${badge}`;

    this.tooltip = [
      `Module: ${health.name}`,
      `Coverage: ${health.coveragePercent}%`,
      `Errors: ${health.errors}`,
      `Warnings: ${health.warnings}`,
      health.adopted ? "Status: Adopted (errors demoted to warnings)" : "",
      health.unannotated.length > 0
        ? `Unannotated: ${health.unannotated.slice(0, MAX_UNANNOTATED_TOOLTIP).join(", ")}${health.unannotated.length > MAX_UNANNOTATED_TOOLTIP ? "..." : ""}`
        : "",
    ].filter(Boolean).join("\n");

    this.iconPath = coverageIcon(health.coveragePercent);
    this.contextValue = "moduleHealth";

    this.command = {
      command: "vscode.open",
      title: "Open Module",
      arguments: [vscode.Uri.file(health.path)],
    };
  }
}

/** Width of the Unicode progress bar in characters. */
const COVERAGE_BAR_WIDTH = 10;

/** Coverage threshold for "good" (green icon). */
const COVERAGE_GOOD_THRESHOLD = 90;

/** Coverage threshold for "warning" (yellow icon). */
const COVERAGE_WARN_THRESHOLD = 50;

/** Render a progress bar using Unicode block characters. */
function coverageBar(percent: number): string {
  const filled = Math.round(percent / COVERAGE_BAR_WIDTH);
  return "\u2588".repeat(filled) + "\u2591".repeat(COVERAGE_BAR_WIDTH - filled);
}

function coverageIcon(percent: number): vscode.ThemeIcon {
  if (percent >= COVERAGE_GOOD_THRESHOLD) { return new vscode.ThemeIcon("pass", new vscode.ThemeColor("testing.iconPassed")); }
  if (percent >= COVERAGE_WARN_THRESHOLD) { return new vscode.ThemeIcon("warning", new vscode.ThemeColor("list.warningForeground")); }
  return new vscode.ThemeIcon("error", new vscode.ThemeColor("list.errorForeground"));
}

// ── Provider ─────────────────────────────────────────────────────────────

export class TypeHealthProvider implements vscode.TreeDataProvider<TreeItem>, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<TreeItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private data: TypeHealthResponse | undefined;
  private sortMode: SortMode = "worst";
  private readonly disposables: vscode.Disposable[] = [];

  constructor(private readonly store: Store) {}

  public refresh(): void {
    this.data = undefined;
    this.emitter.fire(undefined);
  }

  public cycleSortMode(): void {
    const idx = SORT_CYCLE.indexOf(this.sortMode);
    this.sortMode = SORT_CYCLE[(idx + 1) % SORT_CYCLE.length];
    this.emitter.fire(undefined);
  }

  public dispose(): void {
    for (const d of this.disposables) { d.dispose(); }
    this.emitter.dispose();
  }

  public getTreeItem(element: TreeItem): vscode.TreeItem {
    return element;
  }

  public async getChildren(element?: TreeItem): Promise<TreeItem[]> {
    if (element !== undefined) { return []; }

    if (this.data === undefined) {
      await this.fetchHealth();
    }
    if (this.data === undefined) { return []; }

    const items: TreeItem[] = [new SummaryItem(this.data.workspace)];
    const sorted = this.sortModules([...this.data.modules]);
    for (const mod of sorted) {
      items.push(new ModuleHealthItem(mod));
    }
    return items;
  }

  private sortModules(modules: ModuleHealth[]): ModuleHealth[] {
    switch (this.sortMode) {
      case "worst":
        return modules.sort((a, b) => a.coveragePercent - b.coveragePercent);
      case "best":
        return modules.sort((a, b) => b.coveragePercent - a.coveragePercent);
      case "alpha":
        return modules.sort((a, b) => a.name.localeCompare(b.name));
    }
  }

  private async fetchHealth(): Promise<void> {
    const client = this.store.client.value;
    if (!client?.isRunning()) {
      return;
    }
    try {
      const result = await client.sendRequest<TypeHealthResponse>(
        "workspace/executeCommand",
        { command: "basilisk.typeHealth", arguments: [{}] },
      );
      this.data = result ?? undefined;
    } catch (err: unknown) {
      Logger.error(`Type Health fetch failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
}

// ── Registration ─────────────────────────────────────────────────────────

/**
 * Register the type health panel.
 *
 * Returns an array of Disposables for command registrations that must be
 * tracked in `singletonDisposables` so `deactivate()` can dispose them
 * before re-init. Tree views and provider go to `context.subscriptions`.
 */
export function registerTypeHealth(
  context: vscode.ExtensionContext,
  store: Store,
): { provider: TypeHealthProvider; disposables: vscode.Disposable[] } {
  const provider = new TypeHealthProvider(store);

  const treeView = vscode.window.createTreeView("basilisk.typeHealth", {
    treeDataProvider: provider,
  });

  context.subscriptions.push(treeView);
  context.subscriptions.push(provider);

  const disposables = [
    vscode.commands.registerCommand("basilisk.refreshTypeHealth", () => {
      provider.refresh();
    }),
    vscode.commands.registerCommand("basilisk.sortTypeHealth", () => {
      provider.cycleSortMode();
    }),
  ];

  return { provider, disposables };
}
