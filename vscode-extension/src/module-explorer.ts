// Implements [EXTACT-MODULES]. See docs/specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-MODULES
/**
 * Module Explorer — TreeDataProvider for the Basilisk sidebar.
 *
 * Shows the workspace's Python modules and their top-level symbols in a
 * hierarchical tree. Data is fetched from the LSP server via the
 * `basilisk.workspaceModules` command and refreshed on `basilisk/moduleChanged`
 * notifications.
 */

import * as vscode from "vscode";
import { type Store } from "./store";
import { subscribeRevision } from "./reactive-refresh";
import { Logger } from "./logger";

// ── LSP response types ───────────────────────────────────────────────────

interface SymbolNode {
  readonly name: string;
  readonly kind: "class" | "function" | "variable" | "constant" | "typeAlias";
  readonly line: number;
  readonly annotated: boolean;
  readonly exported: boolean;
  readonly children?: readonly SymbolNode[];
}

interface ModuleNode {
  readonly name: string;
  readonly path: string;
  readonly kind: "package" | "module";
  readonly symbols: readonly SymbolNode[];
  // Health rollup folded into each module by basilisk.workspaceModules
  // [EXTACT-MODULES] — coverage %, diagnostic counts, and adoption state, so the
  // merged panel needs no separate basilisk.typeHealth round-trip.
  readonly coveragePercent: number;
  readonly errors: number;
  readonly warnings: number;
  readonly adopted: boolean;
}

/** Workspace-wide health rollup carried alongside the module list. */
interface HealthStats {
  readonly totalSymbols: number;
  readonly annotatedSymbols: number;
  readonly coveragePercent: number;
  readonly errors: number;
  readonly warnings: number;
  readonly adoptedFiles: number;
  readonly totalFiles: number;
}

interface WorkspaceModulesResponse {
  readonly modules: readonly ModuleNode[];
  readonly workspace: HealthStats;
}

// ── Tree items ───────────────────────────────────────────────────────────

type TreeItem = ModuleTreeItem | SymbolTreeItem;

export class ModuleTreeItem extends vscode.TreeItem {
  constructor(
    public readonly module: ModuleNode,
  ) {
    super(
      module.name,
      module.symbols.length > 0
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None,
    );
    // Tint the namespace/file icon by coverage so a module's type health is
    // visible at a glance [EXTACT-MODULES]; the per-symbol "untyped" decoration
    // is the drill-down.
    const codicon = module.kind === "package" ? "symbol-namespace" : "symbol-file";
    this.iconPath = new vscode.ThemeIcon(codicon, coverageColor(module.coveragePercent));
    this.contextValue = "module";
    this.description = moduleDescription(module);
    this.tooltip = moduleTooltip(module);
    this.resourceUri = vscode.Uri.file(module.path);
    this.command = {
      command: "vscode.open",
      title: "Open Module",
      arguments: [vscode.Uri.file(module.path)],
    };
  }
}

class SymbolTreeItem extends vscode.TreeItem {
  constructor(
    public readonly symbol: SymbolNode,
    public readonly modulePath: string,
    public readonly moduleName: string,
  ) {
    super(
      symbol.name,
      symbol.children !== undefined && symbol.children.length > 0
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None,
    );
    this.iconPath = symbolIcon(symbol);
    this.contextValue = `symbol.${symbol.kind}`;

    // Decorations: unannotated shows "untyped", private shows "(private)", exported shows "(exported)".
    const parts: string[] = [];
    if (!symbol.annotated) { parts.push("untyped"); }
    if (symbol.name.startsWith("_") && !symbol.name.startsWith("__")) { parts.push("private"); }
    if (symbol.exported) { parts.push("exported"); }
    this.description = parts.join(" · ");

    const uri = vscode.Uri.file(modulePath);
    this.command = {
      command: "vscode.open",
      title: "Go to Symbol",
      arguments: [uri, { selection: new vscode.Range(symbol.line, 0, symbol.line, 0) }],
    };
  }
}

function symbolIcon(symbol: SymbolNode): vscode.ThemeIcon {
  // Unannotated symbols get a warning-colored icon, private get dimmed.
  const color = !symbol.annotated
    ? new vscode.ThemeColor("list.warningForeground")
    : symbol.name.startsWith("_") && !symbol.name.startsWith("__")
      ? new vscode.ThemeColor("disabledForeground")
      : undefined;

  switch (symbol.kind) {
    case "class": return new vscode.ThemeIcon("symbol-class", color);
    case "function": return new vscode.ThemeIcon("symbol-method", color);
    case "variable": return new vscode.ThemeIcon("symbol-variable", color);
    case "constant": return new vscode.ThemeIcon("symbol-constant", color);
    case "typeAlias": return new vscode.ThemeIcon("symbol-type-parameter", color);
    default: return new vscode.ThemeIcon("symbol-misc", color);
  }
}

// ── Coverage rendering [EXTACT-MODULES] ──────────────────────────────────

/** Width of the Unicode coverage bar in characters. */
const COVERAGE_BAR_WIDTH = 10;
/** Coverage threshold for "good" (green). */
const COVERAGE_GOOD_THRESHOLD = 90;
/** Coverage threshold for "warning" (yellow); below it is red. */
const COVERAGE_WARN_THRESHOLD = 50;

/** Render a coverage progress bar using Unicode block characters. */
function coverageBar(percent: number): string {
  const filled = Math.round(percent / COVERAGE_BAR_WIDTH);
  return "█".repeat(filled) + "░".repeat(COVERAGE_BAR_WIDTH - filled);
}

/** Theme color for a coverage percentage: green >=90%, yellow >=50%, else red. */
function coverageColor(percent: number): vscode.ThemeColor {
  if (percent >= COVERAGE_GOOD_THRESHOLD) { return new vscode.ThemeColor("testing.iconPassed"); }
  if (percent >= COVERAGE_WARN_THRESHOLD) { return new vscode.ThemeColor("list.warningForeground"); }
  return new vscode.ThemeColor("list.errorForeground");
}

/** Module row description: coverage bar + % + error/warning counts + adopted badge. */
function moduleDescription(module: ModuleNode): string {
  const issues: string[] = [];
  if (module.errors > 0) { issues.push(`${module.errors}E`); }
  if (module.warnings > 0) { issues.push(`${module.warnings}W`); }
  const issueStr = issues.length > 0 ? ` — ${issues.join(" ")}` : "";
  const badge = module.adopted ? " [adopted]" : "";
  return `${coverageBar(module.coveragePercent)} ${module.coveragePercent}%${issueStr}${badge}`;
}

/** Module row tooltip: name + path + coverage + diagnostics + adoption. */
function moduleTooltip(module: ModuleNode): string {
  return [
    module.name,
    module.path,
    `Coverage: ${module.coveragePercent}%`,
    `Errors: ${module.errors}`,
    `Warnings: ${module.warnings}`,
    module.adopted ? "Status: Adopted (errors demoted to warnings)" : "",
  ].filter(Boolean).join("\n");
}

// ── Workspace health chrome [EXTACT-MODULES-HEADER] ──────────────────────

/**
 * Workspace summary rendered into the tree view's native `message` chrome.
 *
 * [EXTACT-HEALTH] An empty workspace (no Python files) renders an explicit
 * "No Python files found" — never a misleading 100% for 0/0 symbols (#57).
 */
export function workspaceHealthMessage(stats: HealthStats | undefined): string {
  if (stats === undefined) { return ""; }
  if (stats.totalFiles === 0) { return "No Python files found"; }
  const issues: string[] = [];
  if (stats.errors > 0) { issues.push(`${stats.errors}E`); }
  if (stats.warnings > 0) { issues.push(`${stats.warnings}W`); }
  const issueStr = issues.length > 0 ? ` · ${issues.join(" ")}` : "";
  return `${stats.coveragePercent}% typed${issueStr}`;
}

/** Numeric view badge: outstanding diagnostics (errors + warnings), or none. */
export function workspaceHealthBadge(stats: HealthStats | undefined): vscode.ViewBadge | undefined {
  if (stats === undefined || stats.totalFiles === 0) { return undefined; }
  const count = stats.errors + stats.warnings;
  if (count === 0) { return undefined; }
  const errs = `${stats.errors} error${stats.errors === 1 ? "" : "s"}`;
  const warns = `${stats.warnings} warning${stats.warnings === 1 ? "" : "s"}`;
  return { value: count, tooltip: `${errs}, ${warns}` };
}

// ── Provider ─────────────────────────────────────────────────────────────

/** View mode for module explorer: tree (hierarchical) or flat (all symbols). */
type ViewMode = "tree" | "flat";

/** Sort mode applied in flat view (tree view stays structural). */
type SortMode = "worst" | "best" | "alpha";

const SORT_CYCLE: readonly SortMode[] = ["worst", "best", "alpha"];

export class ModuleExplorerProvider implements vscode.TreeDataProvider<TreeItem>, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<TreeItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private modules: readonly ModuleNode[] = [];
  private workspace: HealthStats | undefined;
  public readonly disposables: vscode.Disposable[] = [];
  private viewMode: ViewMode = "tree";
  private sortMode: SortMode = "worst";
  private filterPattern = "";
  private treeView: vscode.TreeView<TreeItem> | undefined;

  constructor(private readonly store: Store) {}

  /** Bind the tree view so the provider can drive its native message + badge chrome. */
  public setTreeView(treeView: vscode.TreeView<TreeItem>): void {
    this.treeView = treeView;
  }

  public refresh(): void {
    this.modules = [];
    this.workspace = undefined;
    this.emitter.fire(undefined);
  }

  /** Cycle the flat-view sort: worst-first -> best-first -> alphabetical. */
  public cycleSortMode(): void {
    const idx = SORT_CYCLE.indexOf(this.sortMode);
    this.sortMode = SORT_CYCLE[(idx + 1) % SORT_CYCLE.length];
    this.emitter.fire(undefined);
  }

  /** Toggle between tree and flat view modes, persisted in workspaceState. */
  public toggleViewMode(context: vscode.ExtensionContext): void {
    this.viewMode = this.viewMode === "tree" ? "flat" : "tree";
    void context.workspaceState.update("basilisk.moduleExplorerView", this.viewMode);
    void vscode.commands.executeCommand("setContext", "basilisk.moduleExplorerView", this.viewMode);
    this.emitter.fire(undefined);
  }

  /** Restore view mode from workspaceState. */
  public restoreViewMode(context: vscode.ExtensionContext): void {
    this.viewMode = context.workspaceState.get<ViewMode>("basilisk.moduleExplorerView") ?? "tree";
    void vscode.commands.executeCommand("setContext", "basilisk.moduleExplorerView", this.viewMode);
  }

  /** Set the glob filter pattern and re-render. */
  public setFilter(pattern: string): void {
    this.filterPattern = pattern;
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
    if (element instanceof SymbolTreeItem) {
      return (element.symbol.children ?? []).map(
        (child) => new SymbolTreeItem(child, element.modulePath, element.moduleName),
      );
    }

    if (element instanceof ModuleTreeItem) {
      return element.module.symbols.map(
        (sym) => new SymbolTreeItem(sym, element.module.path, element.module.name),
      );
    }

    // Root: fetch modules from LSP.
    if (this.modules.length === 0) {
      await this.fetchModules();
    }
    this.updateViewChrome();

    const filtered = this.applyFilter(this.modules);

    if (this.viewMode === "flat") {
      // Flat view honours the sort toggle; tree view stays structural.
      return ModuleExplorerProvider.flattenModules(this.sortModules([...filtered]));
    }
    return filtered.map((mod) => new ModuleTreeItem(mod));
  }

  /** Order modules for flat view per the current sort toggle. */
  private sortModules(modules: ModuleNode[]): ModuleNode[] {
    switch (this.sortMode) {
      case "worst": return modules.sort((a, b) => a.coveragePercent - b.coveragePercent);
      case "best": return modules.sort((a, b) => b.coveragePercent - a.coveragePercent);
      case "alpha": return modules.sort((a, b) => a.name.localeCompare(b.name));
    }
  }

  /** Push the workspace rollup into the tree view's native message + badge chrome. */
  private updateViewChrome(): void {
    if (this.treeView === undefined) { return; }
    this.treeView.message = workspaceHealthMessage(this.workspace);
    this.treeView.badge = workspaceHealthBadge(this.workspace);
  }

  /** Apply glob-style filter to module list. */
  private applyFilter(modules: readonly ModuleNode[]): readonly ModuleNode[] {
    if (this.filterPattern === "") { return modules; }
    const pattern = this.filterPattern.toLowerCase();
    return modules.filter((mod) => {
      // Simple glob: * matches any chars, ? matches single char.
      if (pattern.includes("*") || pattern.includes("?")) {
        const regex = new RegExp(
          `^${pattern.replace(/\*/g, ".*").replace(/\?/g, ".")}$`,
        );
        return regex.test(mod.name.toLowerCase());
      }
      return mod.name.toLowerCase().includes(pattern);
    });
  }

  /** Flatten modules into a flat symbol list for flat view mode. */
  private static flattenModules(modules: readonly ModuleNode[]): TreeItem[] {
    const items: TreeItem[] = [];
    for (const mod of modules) {
      for (const sym of mod.symbols) {
        items.push(new SymbolTreeItem(sym, mod.path, mod.name));
      }
    }
    return items;
  }

  private async fetchModules(): Promise<void> {
    const client = this.store.client.value;
    if (!client?.isRunning()) {
      return;
    }
    try {
      const result = await client.sendRequest<WorkspaceModulesResponse>(
        "workspace/executeCommand",
        { command: "basilisk.workspaceModules", arguments: [{}] },
      );
      this.modules = result?.modules ?? [];
      this.workspace = result?.workspace;
    } catch (err: unknown) {
      Logger.error(`Module Explorer fetch failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
}

// ── Registration ─────────────────────────────────────────────────────────

/**
 * Register clipboard and action commands for the module explorer.
 *
 * Returns an array of Disposables so the caller can track them in
 * `singletonDisposables` — NOT in `context.subscriptions`. This ensures
 * `deactivate()` can dispose them before `firstInit = true` re-registers
 * them, preventing "command already exists" crashes on window reload.
 */
function registerExplorerCommands(
  context: vscode.ExtensionContext,
  provider: ModuleExplorerProvider,
): vscode.Disposable[] {
  return [
    vscode.commands.registerCommand("basilisk.refreshModuleExplorer", () => {
      provider.refresh();
    }),
    vscode.commands.registerCommand("basilisk.collapseModuleExplorer", () => {
      // TreeView collapse is handled natively by showCollapseAll.
    }),
    vscode.commands.registerCommand("basilisk.toggleModuleExplorerView", () => {
      provider.toggleViewMode(context);
    }),
    vscode.commands.registerCommand("basilisk.sortModuleExplorer", () => {
      provider.cycleSortMode();
    }),
    vscode.commands.registerCommand("basilisk.filterModuleExplorer", async () => {
      const input = await vscode.window.showInputBox({
        prompt: "Filter modules (supports * and ? globs)",
        placeHolder: "e.g. myapp.api.*",
      });
      provider.setFilter(input ?? "");
    }),
    vscode.commands.registerCommand("basilisk.copyImportPath", (item: TreeItem) => {
      if (item instanceof SymbolTreeItem) {
        const importPath = `from ${item.moduleName} import ${item.symbol.name}`;
        void vscode.env.clipboard.writeText(importPath);
        vscode.window.showInformationMessage(`Copied: ${importPath}`);
      } else if (item instanceof ModuleTreeItem) {
        const importPath = `import ${item.module.name}`;
        void vscode.env.clipboard.writeText(importPath);
        vscode.window.showInformationMessage(`Copied: ${importPath}`);
      }
    }),
    vscode.commands.registerCommand("basilisk.copyQualifiedName", (item: TreeItem) => {
      if (item instanceof SymbolTreeItem) {
        const name = `${item.moduleName}.${item.symbol.name}`;
        void vscode.env.clipboard.writeText(name);
        vscode.window.showInformationMessage(`Copied: ${name}`);
      } else if (item instanceof ModuleTreeItem) {
        void vscode.env.clipboard.writeText(item.module.name);
        vscode.window.showInformationMessage(`Copied: ${item.module.name}`);
      }
    }),
  ];
}

/**
 * Register the module explorer panel.
 *
 * Returns an array of Disposables for command registrations that must be
 * tracked in `singletonDisposables` so `deactivate()` can dispose them
 * before re-init. The tree view and provider are pushed to
 * `context.subscriptions` since they are safe to dispose via VS Code's
 * lifecycle (tree view IDs are inherently singleton).
 */
export function registerModuleExplorer(
  context: vscode.ExtensionContext,
  store: Store,
): { provider: ModuleExplorerProvider; disposables: vscode.Disposable[] } {
  const provider = new ModuleExplorerProvider(store);

  const treeView = vscode.window.createTreeView("basilisk.moduleExplorer", {
    treeDataProvider: provider,
    showCollapseAll: true,
  });
  provider.setTreeView(treeView);

  context.subscriptions.push(treeView, provider);

  provider.restoreViewMode(context);
  const commandDisposables = registerExplorerCommands(context, provider);
  wireReactiveRefresh(store, provider);

  return { provider, disposables: commandDisposables };
}

/**
 * Subscribe the panel to the store's centralized `analysisRevision` signal
 * ([EXTACT-REACTIVE-STATE], issue #58): server-Running, re-analysis
 * completion, and diagnostics changes all bump the revision in the store, so
 * the panel refreshes automatically — no per-panel polling or notification
 * plumbing.
 */
export function wireReactiveRefresh(store: Store, provider: ModuleExplorerProvider): void {
  subscribeRevision(store.analysisRevision, provider);
}
