# Basilisk activity panel {#EXTACT}

The activity container exposes the shipped workspace module/health view and compact Basilisk
information view. Shared data comes from LSP commands; editor specs own their rendering.

## Shared LSP surface {#EXTACT-LSP-COMMANDS}

### `basilisk.workspaceModules` {#EXTACT-LSP-COMMANDS-WORKSPACE-MODULES}

Request params are `{scope?: string}`. The response is a flat, name-sorted module list plus
a workspace health rollup. `scope` filters by dotted module-name prefix.

### `basilisk/moduleChanged` {#EXTACT-LSP-COMMANDS-MODULE-CHANGED}

This server-to-client notification carries one changed module after re-analysis. Clients use
it as an invalidation signal and refetch canonical state rather than merging an invented
parallel model.

### `basilisk/scanComplete` {#EXTACT-LSP-COMMANDS-SCAN-COMPLETE}

This notification carries `{totalFiles}` after a workspace scan. It is required for a client
to distinguish a genuinely empty workspace from an initial scan that has not published any
diagnostics or modules.

### `basilisk.typeHealth` {#EXTACT-LSP-COMMANDS-TYPE-HEALTH}

This request accepts an optional module filter and returns workspace plus per-module health.
VS Code uses the health already folded into `workspaceModules`; the standalone command
remains available to other clients.

## Wire data {#EXTACT-DATA-MODEL}

The implementation in `crates/basilisk-lsp/src/server/activity_panel/` is authoritative.

```typescript
interface WorkspaceModulesResponse {
  modules: ModuleNode[];
  workspace: HealthStats;
}

interface ModuleNode {
  name: string;
  path: string;
  kind: "package" | "module";
  symbols: SymbolNode[];
  coveragePercent?: number;
  errors?: number;
  warnings?: number;
  adopted?: boolean;
}

interface SymbolNode {
  name: string;
  kind: "class" | "function" | "variable" | "constant" | "typeAlias";
  line: number;
  annotated: boolean;
  exported: boolean;
  children?: SymbolNode[];
}

interface HealthStats {
  typeCheckingEnabled?: boolean;
  totalSymbols?: number;
  annotatedSymbols?: number;
  coveragePercent?: number;
  errors?: number;
  warnings?: number;
  adoptedFiles?: number;
  totalFiles: number;
  scanComplete?: boolean;
}
```

The module list is flat: it has no `children` field. It also contains no diagnostic objects,
symbol type strings, or export analysis; `exported` is currently always false. When type
checking is disabled, grading fields are omitted and the workspace is stamped
`typeCheckingEnabled: false`.

## Modules view {#EXTACT-MODULES}

VS Code reconstructs a package/folder tree from dotted module names. Health stays folded
onto module rows and the workspace header; there is no separate Type Health tree in that
activity container.

### Module row {#EXTACT-MODULES-MODULE-ROW}

A module row shows its last path segment in tree view or full dotted name in flat view. Its
description contains a ten-cell coverage bar, percentage, nonzero error/warning tally, and
`[adopted]` when applicable. Package/file icons are tinted by coverage while grading exists.
Clicking opens the module file.

### Diagnostic count style {#EXTACT-MODULES-COUNT-STYLE}

Plain descriptions render nonzero counts as `🔴 n` and `🟠 n`; they do not use
invented inline color spans. Icons use editor theme colors where an icon is available.

### Diagnostic drill-down status {#EXTACT-MODULES-DIAGNOSTICS}

`ModuleNode` does not contain diagnostic locations, and the shipped tree has no diagnostic
child rows. Error/warning values are summary counts only; navigation remains in the editor's
Problems/diagnostic UI.

### Workspace header {#EXTACT-MODULES-HEADER}

The VS Code tree's native message shows coverage and diagnostic counts; its badge is the
nonzero total issue count. With type checking disabled it says `Type checking disabled` and
shows no grading or badge. A completed zero-file scan says `No Python files found`.

#### Loading state {#EXTACT-MODULES-HEADER-LOADING}

Before any health response, or while `totalFiles == 0` without `scanComplete == true`, the
message is `Analyzing workspace…` and no badge is shown. `basilisk/scanComplete` settles this
state even when the workspace is empty.

### Tree reconstruction {#EXTACT-MODULES-TREE-STRUCTURE}

Clients split dotted names into segments. Real package modules attach to their segment;
missing intermediate packages become structural folders. Containers precede leaf modules,
then sort alphabetically. Container issue counts roll up from descendants. Flat view removes
folders but keeps symbols under their owning module.

### Symbol items {#EXTACT-MODULES-ITEM-PROPERTIES}

Symbol rows show name and a kind icon, expand class children, and open the file at the
zero-based source line. No signature or docstring field exists in the wire payload.

### Decorations {#EXTACT-MODULES-DECORATIONS}

Unannotated symbols show `untyped` and a warning-colored icon. Single-underscore names show
`private` with a disabled color. The `exported` decoration exists client-side but the server
does not yet compute it.

### Context menu {#EXTACT-MODULES-CONTEXT-MENU}

The shipped module/symbol context menu contains only Copy Import Path and Copy Qualified
Name. Navigation comes from row activation; rename, references, organize-imports, and per-
module fix actions are not context-menu entries.

### Toolbar {#EXTACT-MODULES-TOOLBAR}

Contributed inline actions are Refresh, tree/flat Toggle, Filter, and flat-only Sort, in that
order. VS Code supplies Collapse All natively. Organize Imports and opt-in Fix Workspace live
in the modify overflow group; Restart Server is separate. Server actions are visible only
while the server is running, and Fix Workspace also requires
`basilisk.experimental.fixAll`.

### Refresh {#EXTACT-MODULES-REFRESH}

Manual refresh clears cached modules and refetches. Server start, module changes,
`scanComplete`, and debounced diagnostics update the shared analysis revision, which causes
reactive consumers to refetch.

### Centralized reactive state {#EXTACT-REACTIVE-STATE}

`vscode-extension/src/store.ts` owns LSP/client state and the analysis revision. Panel
providers subscribe through the shared signals helper; they must not create polling loops or
competing protocol listeners.

## Type health response {#EXTACT-HEALTH}

The standalone response is retained for clients without the merged VS Code module view.

### Tree structure {#EXTACT-HEALTH-TREE-STRUCTURE}

It contains one workspace `HealthStats` value and a per-module list sorted by ascending
coverage. Disabled type checking returns an empty module list and a neutral disabled stamp.

### Module properties {#EXTACT-HEALTH-ITEM-PROPERTIES}

Each module has name, path, coverage percentage, error/warning counts, adopted flag, and the
names of unannotated symbols.

### Header {#EXTACT-HEALTH-HEADER}

Consumers must render an explicit empty/disabled state rather than interpreting zero symbols
as a perfect score.

### Toolbar {#EXTACT-HEALTH-TOOLBAR}

No separate VS Code health toolbar ships. Other clients may expose refresh/filter actions
over the shared response without changing its semantics.

### Context menu {#EXTACT-HEALTH-CONTEXT-MENU}

The protocol defines no health-specific mutation commands. Clients should route navigation
and fixes through their existing module, diagnostic, and command surfaces.

### Refresh {#EXTACT-HEALTH-REFRESH}

Fetch on demand and invalidate from the same analysis-revision signals as the module view.

## Basilisk information view {#EXTACT-INFO}

This is a compact settings/status tree, not a second command palette.

### Structure {#EXTACT-INFO-STRUCTURE}

One Type Checking toggle appears at the root, followed by a read-only Server Info section.
The provider refreshes on Basilisk configuration and LSP lifecycle signal changes.

### Interaction affordance {#EXTACT-INFO-AFFORDANCE}

Actionable items have a command, imperative tooltip, action icon, and inline control.
Read-only rows have no command/inline action and put their value in the description. Shared
constructors enforce the distinction.

### Getting started {#EXTACT-INFO-GETTING-STARTED}

Onboarding content belongs to VS Code walkthroughs and welcome views, not permanent panel
rows.

### Feature status {#EXTACT-INFO-FEATURE-STATUS}

Only `basilisk.enabled` is a shipped toggle because the server demonstrably clears and
suppresses diagnostics while disabled and rechecks when enabled. Settings that are not read
by their feature path must not appear as functional toggles.

### Quick actions {#EXTACT-INFO-QUICK-ACTIONS}

There is no Quick Actions section. Fix/organize/restart live on the Modules toolbar, Show
Output lives on the status item, uv actions remain commands/code actions, and the
configuration editor has its own title action and command.

#### Action wiring {#EXTACT-INFO-ACTION-WIRING}

Every visible client-owned action must have a registered handler. Server-dependent actions
must be gated on the running state; server command names are discovered from LSP capability
advertisement rather than registered twice.

### Server information {#EXTACT-INFO-SERVER-INFO}

Read-only rows show available server version, analysis mode, Python selection, uv settings,
and Basilisk binary. The status bar, not this panel, owns live running/stopped state.

## Editor ownership {#EXTACT-EDITORS}

### VS Code {#EXTACT-EDITORS-VSCODE}

`module-explorer.ts`, `info-panel.ts`, `store.ts`, and `package.json` are the canonical client
implementation. Zed and Neovim own their command/output presentation in their editor specs;
instructional slash-command text must not be described as a live tree response.

## Accessibility {#EXTACT-ACCESSIBILITY}

Tree items require meaningful labels/tooltips, theme colors must not be the only state cue,
and every action must be keyboard/command-palette reachable. The VS Code accessibility suite
checks contributed views and actions.

## Performance {#EXTACT-PERFORMANCE}

The server returns flat, sorted JSON and the client rebuilds its tree locally. Refreshes are
event-driven and diagnostics are debounced; no performance latency or memory target is
claimed without a benchmark.
