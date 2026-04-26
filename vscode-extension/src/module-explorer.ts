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
}

interface WorkspaceModulesResponse {
  readonly modules: readonly ModuleNode[];
}

// ── Tree items ───────────────────────────────────────────────────────────

type TreeItem = ModuleTreeItem | SymbolTreeItem;

class ModuleTreeItem extends vscode.TreeItem {
  constructor(
    public readonly module: ModuleNode,
  ) {
    super(
      module.name,
      module.symbols.length > 0
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None,
    );
    this.iconPath = module.kind === "package"
      ? new vscode.ThemeIcon("symbol-namespace")
      : new vscode.ThemeIcon("symbol-file");
    this.contextValue = "module";
    this.description = module.kind;
    this.tooltip = module.path;
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

// ── Provider ─────────────────────────────────────────────────────────────

/** View mode for module explorer: tree (hierarchical) or flat (all symbols). */
type ViewMode = "tree" | "flat";

export class ModuleExplorerProvider implements vscode.TreeDataProvider<TreeItem>, vscode.Disposable {
  private readonly emitter = new vscode.EventEmitter<TreeItem | undefined>();
  public readonly onDidChangeTreeData = this.emitter.event;

  private modules: readonly ModuleNode[] = [];
  public readonly disposables: vscode.Disposable[] = [];
  private viewMode: ViewMode = "tree";
  private filterPattern = "";

  constructor(private readonly store: Store) {}

  public refresh(): void {
    this.modules = [];
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

    const filtered = this.applyFilter(this.modules);

    if (this.viewMode === "flat") {
      return ModuleExplorerProvider.flattenModules(filtered);
    }
    return filtered.map((mod) => new ModuleTreeItem(mod));
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
    } catch (err: unknown) {
      Logger.error(`Module Explorer fetch failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
}

// ── Registration ─────────────────────────────────────────────────────────

/** Interval (ms) for polling client readiness to wire notification listeners. */
const CLIENT_POLL_INTERVAL_MS = 1000;

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

  context.subscriptions.push(treeView, provider);

  provider.restoreViewMode(context);
  const commandDisposables = registerExplorerCommands(context, provider);
  wireModuleChangedListener(store, provider);

  return { provider, disposables: commandDisposables };
}

/**
 * Watch the store's client signal and register a notification listener for
 * `basilisk/moduleChanged` whenever a new client connects.
 */
function wireModuleChangedListener(store: Store, provider: ModuleExplorerProvider): void {
  let registered = false;

  // Poll for client availability (same pattern as test-explorer).
  const interval = setInterval(() => {
    const client = store.client.value;
    if (!client?.isRunning() || registered) {
      if (client === undefined) { registered = false; }
      return;
    }
    registered = true;
    client.onNotification("basilisk/moduleChanged", () => {
      provider.refresh();
    });
  }, CLIENT_POLL_INTERVAL_MS);

  provider.disposables.push({ dispose: () => { clearInterval(interval); } });
}
