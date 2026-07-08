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
//
// Implements the client mirror of [EXTACT-DATA-MODEL] — the shared
// WorkspaceModulesResponse / ModuleNode / SymbolNode / HealthStats wire shapes
// returned by basilisk.workspaceModules. (The spec's DiagnosticNode /
// ModuleNode.diagnostics drill-down [EXTACT-MODULES-DIAGNOSTICS] is not yet
// modelled here — see the activity-panel audit notes.)

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
  // merged panel needs no separate basilisk.typeHealth round-trip. ABSENT while
  // Type Checking is disabled ([ANALYSIS-ENABLED], #119): the server omits all
  // grading, so there is nothing to render as "% typed" or a red tint.
  readonly coveragePercent?: number;
  readonly errors?: number;
  readonly warnings?: number;
  readonly adopted?: boolean;
}

/** Workspace-wide health rollup carried alongside the module list. */
interface HealthStats {
  // The Type Checking toggle state stamped by the server ([ANALYSIS-ENABLED],
  // #119). `false` means the grading fields below are absent by construction.
  readonly typeCheckingEnabled?: boolean;
  readonly totalSymbols?: number;
  readonly annotatedSymbols?: number;
  readonly coveragePercent?: number;
  readonly errors?: number;
  readonly warnings?: number;
  readonly adoptedFiles?: number;
  readonly totalFiles: number;
  // Whether the server's initial workspace scan has finished. A zero-file
  // rollup only means "empty workspace" when this is true; before that it
  // means "not scanned yet" ([EXTACT-MODULES-HEADER-LOADING], #144).
  readonly scanComplete?: boolean;
}

interface WorkspaceModulesResponse {
  readonly modules: readonly ModuleNode[];
  readonly workspace: HealthStats;
}

/**
 * A node in the client-reconstructed package/folder tree
 * [EXTACT-MODULES-TREE-STRUCTURE] (#149). The LSP returns a *flat* list of
 * modules keyed by dotted name (e.g. `pkg.sub.mod`); the nested tree is rebuilt
 * here by splitting each name into path segments. Intermediate folders that are
 * not themselves Python packages are synthesised as container nodes with no
 * `module`, so the panel renders `pkg/ → sub/ → mod` instead of a flat list.
 */
interface PackageTreeNode {
  /** Last path segment — the row's display label (e.g. `auth`). */
  readonly segment: string;
  /** Fully-qualified dotted prefix up to and including this node. */
  readonly fullName: string;
  /** The module/package file mapping exactly here, if one exists. */
  module?: ModuleNode;
  /** Child packages and modules, keyed by their segment. */
  readonly children: Map<string, PackageTreeNode>;
  // Diagnostics rolled up across this node's whole subtree (self module +
  // every descendant). Surfaced on the folder/package row so errors are
  // visible without drilling into the hierarchy (#149). Set by `rollup`.
  errors: number;
  warnings: number;
}

// ── Tree items ───────────────────────────────────────────────────────────

type TreeItem = ModuleTreeItem | SymbolTreeItem | PackageTreeItem;

// Implements [EXTACT-MODULES-MODULE-ROW] — the module row: label, coverage-tinted
// icon, folded-health description, tooltip, and open-on-click action.
export class ModuleTreeItem extends vscode.TreeItem {
  constructor(
    public readonly module: ModuleNode,
    // Tree view labels each module by its last path segment (`auth`); flat view
    // keeps the full dotted name (`pkg.api.auth`) since there is no folder
    // nesting to give context. Defaults to the full name.
    displayName: string = module.name,
  ) {
    super(
      displayName,
      module.symbols.length > 0
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None,
    );
    // Tint the namespace/file icon by coverage so a module's type health is
    // visible at a glance [EXTACT-MODULES]; the per-symbol "untyped" decoration
    // is the drill-down. No coverage (Type Checking disabled, #119) → no tint.
    const codicon = module.kind === "package" ? "symbol-namespace" : "symbol-file";
    const tint = module.coveragePercent !== undefined
      ? coverageColor(module.coveragePercent)
      : undefined;
    this.iconPath = new vscode.ThemeIcon(codicon, tint);
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

/**
 * A package/folder container row in the nested tree view
 * [EXTACT-MODULES-TREE-STRUCTURE] (#149). Carries its tree node so the provider
 * can expand it to child packages/modules and — when the folder is a real
 * Python package (`__init__.py`) — the package's own top-level symbols. A pure
 * folder (no `__init__.py`) carries no `module`: it is a structural container
 * with no coverage rollup and no open-on-click action.
 */
export class PackageTreeItem extends vscode.TreeItem {
  constructor(
    public readonly node: PackageTreeNode,
  ) {
    super(node.segment, vscode.TreeItemCollapsibleState.Collapsed);
    const { module } = node;
    // Tint by the worst diagnostic in the subtree so a folder containing errors
    // reads red at a glance; fall back to the package's own coverage colour, or
    // a neutral folder icon for a pure (non-package) directory (#149).
    this.iconPath = new vscode.ThemeIcon("symbol-namespace", packageIconColor(node));
    this.description = packageDescription(node);
    this.tooltip = packageTooltip(node);
    if (module === undefined) {
      this.contextValue = "folder";
      return;
    }
    this.contextValue = "module";
    this.resourceUri = vscode.Uri.file(module.path);
    this.command = {
      command: "vscode.open",
      title: "Open Module",
      arguments: [vscode.Uri.file(module.path)],
    };
  }
}

// Implements [EXTACT-MODULES-ITEM-PROPERTIES] and [EXTACT-MODULES-DECORATIONS]
// for symbol labels, suffixes, icons, and open-at-line commands.
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
/** Neutral coverage for ungraded rows (Type Checking disabled, #119). */
const FULL_COVERAGE_PERCENT = 100;

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

// [EXTACT-MODULES-COUNT-STYLE] is the diagnostic-tally surface for module rows.
/** Module row description: coverage bar + % + error/warning counts + adopted badge. */
function moduleDescription(module: ModuleNode): string {
  // Type Checking disabled (#119): the server serves no grading, so the row is
  // a plain navigation entry — no bar, no percentage, no tallies.
  if (module.coveragePercent === undefined) { return ""; }
  const issueTally = diagnosticTally(module.errors ?? 0, module.warnings ?? 0);
  const issueStr = issueTally === "" ? "" : ` — ${issueTally}`;
  const badge = module.adopted === true ? " [adopted]" : "";
  return `${coverageBar(module.coveragePercent)} ${module.coveragePercent}%${issueStr}${badge}`;
}

/** Module row tooltip: name + path + coverage + diagnostics + adoption. */
function moduleTooltip(module: ModuleNode): string {
  return [
    module.name,
    module.path,
    module.coveragePercent !== undefined ? `Coverage: ${module.coveragePercent}%` : "",
    module.errors !== undefined ? `Errors: ${module.errors}` : "",
    module.warnings !== undefined ? `Warnings: ${module.warnings}` : "",
    module.adopted === true ? "Status: Adopted (errors demoted to warnings)" : "",
  ].filter(Boolean).join("\n");
}

/** Implements [EXTACT-MODULES-COUNT-STYLE]: coloured glyphs `🔴 n` (errors) /
 *  `🟠 n` (warnings) — never `nE nW`; a zero severity is omitted, or "" when clean. */
function diagnosticTally(errors: number, warnings: number): string {
  const issues: string[] = [];
  if (errors > 0) { issues.push(`🔴 ${errors}`); }
  if (warnings > 0) { issues.push(`🟠 ${warnings}`); }
  return issues.join(" ");
}

/**
 * Folder/package icon tint: red if the subtree holds any error, else yellow if
 * any warning, else a package's own coverage colour (a pure folder stays
 * untinted). Lets a folder with hidden errors read red without expanding (#149).
 */
function packageIconColor(node: PackageTreeNode): vscode.ThemeColor | undefined {
  if (node.errors > 0) { return new vscode.ThemeColor("list.errorForeground"); }
  if (node.warnings > 0) { return new vscode.ThemeColor("list.warningForeground"); }
  // No coverage served (Type Checking disabled, #119) → untinted, like a folder.
  return node.module?.coveragePercent !== undefined
    ? coverageColor(node.module.coveragePercent)
    : undefined;
}

/**
 * Folder/package row description: the subtree's rolled-up count-style tally
 * ([EXTACT-MODULES-COUNT-STYLE]) so problems are visible without drilling in
 * (#149). A package (`__init__.py`) also keeps its own coverage bar.
 */
function packageDescription(node: PackageTreeNode): string {
  const coverage = node.module?.coveragePercent;
  const own = coverage !== undefined ? `${coverageBar(coverage)} ${coverage}%` : "";
  return [own, diagnosticTally(node.errors, node.warnings)].filter(Boolean).join(" — ");
}

/** Folder/package row tooltip: name + (package path/coverage) + subtree diagnostics. */
function packageTooltip(node: PackageTreeNode): string {
  const errs = `${node.errors} error${node.errors === 1 ? "" : "s"}`;
  const warns = `${node.warnings} warning${node.warnings === 1 ? "" : "s"}`;
  const coverage = node.module?.coveragePercent;
  return [
    node.fullName,
    node.module?.path,
    coverage !== undefined ? `Coverage: ${coverage}%` : "",
    `Subtree: ${errs}, ${warns}`,
  ].filter(Boolean).join("\n");
}

// ── Workspace health chrome [EXTACT-MODULES-HEADER] ──────────────────────

/** Loading affordance while the analyzer starts up or its initial workspace
 *  scan is still running ([EXTACT-MODULES-HEADER-LOADING], #144). */
const ANALYZING_MESSAGE = "Analyzing workspace…";

/**
 * Workspace summary rendered into the tree view's native `message` chrome.
 *
 * Implements [EXTACT-MODULES-HEADER] (`treeView.message`: "73% typed · …").
 * [EXTACT-HEALTH-HEADER] An empty workspace (no Python files) renders an explicit
 * "No Python files found" — never a misleading 100% for 0/0 symbols (#57).
 * [EXTACT-MODULES-HEADER-LOADING] That empty-state is gated on the server's
 * initial scan having finished: before then (or before any stats are fetched
 * at all) the panel shows a loading message, never a false "zero files" (#144).
 */
export function workspaceHealthMessage(stats: HealthStats | undefined): string {
  // No stats yet: the server is idle/starting, or the first fetch hasn't
  // answered. Never render a terminal state from nothing (#144).
  if (stats === undefined) { return ANALYZING_MESSAGE; }
  // Type Checking off ([ANALYSIS-ENABLED], #119): the panel must state that
  // plainly instead of grading the workspace — no "% typed", no tallies.
  if (stats.typeCheckingEnabled === false) { return "Type checking disabled"; }
  if (stats.totalFiles === 0) {
    // Zero files is only the honest empty-state once the scan finished;
    // mid-scan it just means "not scanned yet" (#144).
    return stats.scanComplete === true ? "No Python files found" : ANALYZING_MESSAGE;
  }
  const issueTally = diagnosticTally(stats.errors ?? 0, stats.warnings ?? 0);
  const issueStr = issueTally === "" ? "" : ` · ${issueTally}`;
  return `${stats.coveragePercent ?? FULL_COVERAGE_PERCENT}% typed${issueStr}`;
}

// Implements [EXTACT-MODULES-HEADER] `treeView.badge`: numeric count of
// outstanding diagnostics, hidden when zero or on an empty workspace.
/** Numeric view badge: outstanding diagnostics (errors + warnings), or none.
 *  No badge while Type Checking is disabled ([ANALYSIS-ENABLED], #119). */
export function workspaceHealthBadge(stats: HealthStats | undefined): vscode.ViewBadge | undefined {
  if (stats === undefined || stats.typeCheckingEnabled === false || stats.totalFiles === 0) {
    return undefined;
  }
  const errors = stats.errors ?? 0;
  const warnings = stats.warnings ?? 0;
  const count = errors + warnings;
  if (count === 0) { return undefined; }
  const errs = `${errors} error${errors === 1 ? "" : "s"}`;
  const warns = `${warnings} warning${warnings === 1 ? "" : "s"}`;
  return { value: count, tooltip: `${errs}, ${warns}` };
}

// ── Provider ─────────────────────────────────────────────────────────────

/** View mode for module explorer: tree (hierarchical) or flat (all symbols). */
type ViewMode = "tree" | "flat";

/** Sort mode applied in flat view (tree view stays structural) — #189. */
type SortMode = "name" | "path" | "coverage";

/**
 * The three explicit, labelled sort options surfaced in the picker (#189),
 * replacing the old blind worst/best/alpha cycle. `coverage` is labelled "Type
 * Coverage" to match the panel's existing "Coverage"/"% typed" wording (the
 * `coveragePercent` field is type-coverage, not the PEP conformance score).
 */
const SORT_OPTIONS: readonly { readonly mode: SortMode; readonly label: string }[] = [
  { mode: "name", label: "Module Name" },
  { mode: "path", label: "Path" },
  { mode: "coverage", label: "Type Coverage" },
];

export class ModuleExplorerProvider implements vscode.TreeDataProvider<TreeItem>, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<TreeItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private modules: readonly ModuleNode[] = [];
  private workspace: HealthStats | undefined;
  public readonly disposables: vscode.Disposable[] = [];
  private viewMode: ViewMode = "tree";
  private sortMode: SortMode = "coverage";
  private filterPattern = "";
  private treeView: vscode.TreeView<TreeItem> | undefined;

  constructor(private readonly store: Store) {}

  /** Bind the tree view so the provider can drive its native message + badge chrome. */
  public setTreeView(treeView: vscode.TreeView<TreeItem>): void {
    this.treeView = treeView;
  }

  // Implements [EXTACT-MODULES-REFRESH] — the manual refresh button (and the
  // create/delete/rename full re-fetch) clear the cache and re-query the LSP.
  public refresh(): void {
    this.modules = [];
    this.workspace = undefined;
    this.emitter.fire(undefined);
  }

  /** The active flat-view sort mode (surfaced in the picker, #189). */
  public getSortMode(): SortMode {
    return this.sortMode;
  }

  /** Select the flat-view sort mode explicitly and re-render (#189). */
  public setSortMode(mode: SortMode): void {
    this.sortMode = mode;
    this.emitter.fire(undefined);
  }

  // Implements [EXTACT-MODULES-TOOLBAR] Sort — the explicit Module Name / Path /
  // Type Coverage picker (#189) with the active mode marked, never a blind cycle.
  /** Labelled sort options with the active one marked, to drive the picker (#189). */
  public sortOptions(): readonly { readonly mode: SortMode; readonly label: string; readonly current: boolean }[] {
    return SORT_OPTIONS.map((option) => ({ ...option, current: option.mode === this.sortMode }));
  }

  // Implements [EXTACT-MODULES-TOOLBAR] Toggle View — switch tree<->flat and
  // publish the `basilisk.moduleExplorerView` context key that gates Sort (#151).
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

  // Implements [EXTACT-MODULES-TOOLBAR] Filter — the glob search over module names.
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
      return ModuleExplorerProvider.symbolItems(element.module);
    }

    if (element instanceof PackageTreeItem) {
      return ModuleExplorerProvider.packageChildren(element.node);
    }

    // Root: fetch modules from LSP.
    if (this.modules.length === 0) {
      await this.fetchModules();
    }
    this.updateViewChrome();

    const filtered = this.applyFilter(this.modules);

    if (this.viewMode === "flat") {
      // Flat view: one sortable row per module (full dotted name); the sort
      // picker reorders this list (#151/#189). Symbols stay grouped under their
      // owning module — never dumped bare at the tree root (#149).
      return this.sortModules([...filtered]).map((mod) => new ModuleTreeItem(mod));
    }

    // Tree view: the nested package/folder hierarchy reconstructed from the
    // flat dotted names [EXTACT-MODULES-TREE-STRUCTURE] (#149). The order is
    // structural (containers first, then alphabetical); sort is flat-only.
    const root = ModuleExplorerProvider.buildPackageTree(filtered);
    ModuleExplorerProvider.rollup(root);
    return ModuleExplorerProvider.sortNodes([...root.children.values()])
      .map((node) => ModuleExplorerProvider.nodeToItem(node));
  }

  /** Symbol drill-down rows for a module (methods/attributes/top-level defs). */
  private static symbolItems(module: ModuleNode): TreeItem[] {
    return module.symbols.map(
      (sym) => new SymbolTreeItem(sym, module.path, module.name),
    );
  }

  /**
   * Reconstruct the nested package/folder tree from the flat module list the LSP
   * returns [EXTACT-MODULES-TREE-STRUCTURE] (#149). Each module's dotted name
   * (e.g. `pkg.sub.mod`) is split into path segments; intermediate folders that
   * are not Python packages are synthesised as container nodes. The module is
   * attached to the node at the end of its segment path, so a package
   * (`pkg/__init__.py`, dotted name `pkg`) shares its node with the `pkg/` folder.
   */
  private static buildPackageTree(modules: readonly ModuleNode[]): PackageTreeNode {
    const root: PackageTreeNode = { segment: "", fullName: "", children: new Map(), errors: 0, warnings: 0 };
    for (const module of modules) {
      const segments = module.name.split(".").filter((seg) => seg !== "");
      let node = root;
      for (const segment of segments) {
        const fullName = node.fullName === "" ? segment : `${node.fullName}.${segment}`;
        const existing = node.children.get(segment);
        const child = existing ?? { segment, fullName, children: new Map(), errors: 0, warnings: 0 };
        if (existing === undefined) { node.children.set(segment, child); }
        node = child;
      }
      node.module = module;
    }
    return root;
  }

  /**
   * Roll each subtree's diagnostics up onto its container node (post-order), so a
   * folder/package row can show the total errors/warnings hiding beneath it
   * without the user expanding the whole hierarchy (#149).
   */
  private static rollup(node: PackageTreeNode): void {
    let errors = node.module?.errors ?? 0;
    let warnings = node.module?.warnings ?? 0;
    for (const child of node.children.values()) {
      ModuleExplorerProvider.rollup(child);
      errors += child.errors;
      warnings += child.warnings;
    }
    node.errors = errors;
    node.warnings = warnings;
  }

  /** Render a node: a container becomes a package row, a bare leaf a module row. */
  private static nodeToItem(node: PackageTreeNode): TreeItem {
    if (node.children.size === 0 && node.module !== undefined) {
      return new ModuleTreeItem(node.module, node.segment);
    }
    return new PackageTreeItem(node);
  }

  /** Children of a package/folder: nested nodes first, then the package's symbols. */
  private static packageChildren(node: PackageTreeNode): TreeItem[] {
    const childItems = ModuleExplorerProvider.sortNodes([...node.children.values()])
      .map((child) => ModuleExplorerProvider.nodeToItem(child));
    if (node.module === undefined) { return childItems; }
    return [...childItems, ...ModuleExplorerProvider.symbolItems(node.module)];
  }

  /** Structural sibling order: containers before leaf modules, each alphabetical. */
  private static sortNodes(nodes: PackageTreeNode[]): PackageTreeNode[] {
    return nodes.sort((a, b) => {
      const aContainer = a.children.size > 0;
      const bContainer = b.children.size > 0;
      if (aContainer !== bContainer) { return aContainer ? -1 : 1; }
      return a.segment.localeCompare(b.segment);
    });
  }

  /** Order modules for flat view per the current picker selection. */
  private sortModules(modules: ModuleNode[]): ModuleNode[] {
    switch (this.sortMode) {
      case "name": return modules.sort((a, b) => a.name.localeCompare(b.name));
      case "path": return modules.sort((a, b) => a.path.localeCompare(b.path));
      // Ascending coverage surfaces the least-typed modules first; ungraded
      // rows (Type Checking disabled, #119) sort as neutral 100.
      case "coverage": return modules.sort(
        (a, b) => (a.coveragePercent ?? FULL_COVERAGE_PERCENT) - (b.coveragePercent ?? FULL_COVERAGE_PERCENT),
      );
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

  // Implements [EXTACT-LSP-COMMANDS-WORKSPACE-MODULES] by requesting the flat
  // module list and folded health rollup from the LSP.
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

/** The module a row is backed by, if any — a leaf module or a package node. */
function itemModule(item: TreeItem): ModuleNode | undefined {
  if (item instanceof ModuleTreeItem) { return item.module; }
  if (item instanceof PackageTreeItem) { return item.node.module; }
  return undefined;
}

/** Copy text to the clipboard and confirm with a toast. */
function copyToClipboard(text: string): void {
  void vscode.env.clipboard.writeText(text);
  vscode.window.showInformationMessage(`Copied: ${text}`);
}

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
    vscode.commands.registerCommand("basilisk.toggleModuleExplorerView", () => {
      provider.toggleViewMode(context);
    }),
    vscode.commands.registerCommand("basilisk.sortModuleExplorer", async () => {
      // Implements [EXTACT-MODULES-TOOLBAR] Sort: explicit picker with the active
      // mode checked, so the current sort is always visible — never a blind cycle (#189).
      const items = provider.sortOptions().map((option) => ({
        label: option.current ? `$(check) ${option.label}` : option.label,
        mode: option.mode,
      }));
      const choice = await vscode.window.showQuickPick(items, {
        title: "Sort Modules",
        placeHolder: "Sort the flat module list by…",
      });
      if (choice !== undefined) { provider.setSortMode(choice.mode); }
    }),
    vscode.commands.registerCommand("basilisk.filterModuleExplorer", async () => {
      const input = await vscode.window.showInputBox({
        prompt: "Filter modules (supports * and ? globs)",
        placeHolder: "e.g. myapp.api.*",
      });
      provider.setFilter(input ?? "");
    }),
    // Implements [EXTACT-MODULES-CONTEXT-MENU] Copy Import Path.
    vscode.commands.registerCommand("basilisk.copyImportPath", (item: TreeItem) => {
      if (item instanceof SymbolTreeItem) {
        copyToClipboard(`from ${item.moduleName} import ${item.symbol.name}`);
        return;
      }
      const module = itemModule(item);
      if (module !== undefined) { copyToClipboard(`import ${module.name}`); }
    }),
    // Implements [EXTACT-MODULES-CONTEXT-MENU] Copy Qualified Name.
    vscode.commands.registerCommand("basilisk.copyQualifiedName", (item: TreeItem) => {
      if (item instanceof SymbolTreeItem) {
        copyToClipboard(`${item.moduleName}.${item.symbol.name}`);
        return;
      }
      const module = itemModule(item);
      if (module !== undefined) { copyToClipboard(module.name); }
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
