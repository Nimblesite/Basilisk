# Extension Activity Panel — Implementation Plan

> Spec: [EXTENSION-ACTIVITY-PANEL-SPEC.md](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md)

## Phase 1: LSP Backend (Rust)

Implement the three custom LSP commands in `basilisk-lsp`. This unblocks all editors simultaneously.

**`basilisk/workspaceModules`**:
- Walk the resolver's module graph (already built during analysis)
- For each module, extract top-level symbols from `ResolvedModule`
- Build `ModuleNode` / `SymbolNode` tree from resolver data
- Support `scope` parameter for prefix filtering
- Lazy: return only top-level modules initially, symbols on demand when `scope` narrows to a single module

**`basilisk/moduleChanged`**:
- Hook into the file-change -> re-analysis pipeline
- After a module is re-resolved, diff against previous state
- If changed, push notification with updated `ModuleNode`
- Debounce: 300ms after last save before sending

**`basilisk/typeHealth`**:
- Count annotated vs unannotated symbols per module (resolver already tracks `annotated` on symbols)
- Aggregate diagnostic counts per module (already computed)
- Read adoption state per file
- Return `TypeHealthResponse`

## Phase 2: VS Code Panels (TypeScript)

Reference implementation. All three panels.

1. Register `viewsContainers` and `views` in `package.json`
2. Implement `ModuleExplorerProvider` (`TreeDataProvider<ModuleNode | SymbolNode>`)
   - `getTreeItem()`: map to `TreeItem` with codicons, descriptions, tooltips
   - `getChildren()`: call `basilisk/workspaceModules` with scope
   - Handle `basilisk/moduleChanged` notifications for incremental refresh
3. Implement `TypeHealthProvider` (`TreeDataProvider<ModuleHealth>`)
   - Summary header row with coverage bar
   - Per-module rows with coverage %, errors, warnings, adoption badge
   - Sort cycling
4. Implement `BasiliskInfoProvider` (`TreeDataProvider`)
   - Static tree: Getting Started, Feature Status, Quick Actions, Server Info
   - Feature Status items read settings, click toggles them
   - Quick Actions items fire existing commands
   - Server Info fetched from LSP init response + `basilisk/typeHealth` stats
5. Register all new commands (refresh, toggle view, copy import path, etc.)
6. Wire `basilisk/moduleChanged` notification handler to refresh providers
7. Add walkthrough contribution to `package.json`
8. Create `basilisk-icon.svg` for activity bar

## Phase 3: Zed Slash Commands (Rust/WASM)

Surface the same data through Zed's available extension points.

1. Register `/modules`, `/symbols`, `/health`, `/basilisk` slash commands in `run_slash_command()`
2. Each command calls the corresponding LSP custom command
3. Format responses as clean markdown tables/trees
4. Add argument completion (module names for `/modules` and `/symbols`)

## Phase 4: Neovim Panels (Lua)

Lua-rendered buffers using the nvim LSP client.

1. Implement `basilisk.modules` Lua module — renders module tree in a split buffer
2. Implement `basilisk.health` Lua module — renders type health with colored highlights
3. Implement `basilisk.info` Lua module — floating window with server info
4. Register `:BasiliskModules`, `:BasiliskHealth`, `:BasiliskInfo` commands
5. Default keymaps: `<leader>bm`, `<leader>bh`, `<leader>bi`
6. Handle `basilisk/moduleChanged` via `vim.lsp.handlers`

## Phase 5: Polish

1. End-to-end tests: open workspace, verify module tree matches actual modules, verify health stats
2. Performance testing: large workspace (1000+ files), measure LSP response time for `basilisk/workspaceModules`
3. Accessibility audit: screen reader testing in VS Code
4. Icon design: commission or create final `basilisk-icon.svg`

---

## TODOs

### LSP Backend
- [ ] Implement `basilisk/workspaceModules` handler in `basilisk-lsp`
- [ ] Implement `ModuleNode` / `SymbolNode` construction from `ResolvedModule`
- [ ] Implement `scope` parameter filtering for `basilisk/workspaceModules`
- [ ] Implement `basilisk/moduleChanged` notification on re-analysis
- [ ] Implement 300ms debounce for `basilisk/moduleChanged`
- [ ] Implement `basilisk/typeHealth` handler
- [ ] Implement `HealthStats` computation (annotated vs unannotated symbol counting)
- [ ] Implement `ModuleHealth` per-module breakdown
- [ ] Wire adoption state into `ModuleHealth.adopted`
- [ ] Add all three custom commands to `LSP-ARCHITECTURE-SPEC.md`
- [ ] Unit tests: `basilisk/workspaceModules` returns correct tree for test workspace
- [ ] Unit tests: `basilisk/typeHealth` returns correct coverage percentages
- [ ] Unit tests: `basilisk/moduleChanged` fires after file change, not before

### VS Code Extension
- [ ] Create `basilisk-icon.svg` (monochrome, 24x24, light + dark theme compatible)
- [ ] Add `viewsContainers` and `views` to `package.json`
- [ ] Add `viewsWelcome` entries to `package.json`
- [ ] Add all menu contributions to `package.json`
- [ ] Add walkthrough contribution to `package.json`
- [ ] Implement `ModuleExplorerProvider` — `TreeDataProvider` with lazy child loading
- [ ] Implement module tree item rendering (codicons, descriptions, tooltips, click-to-open)
- [ ] Implement symbol decorations (unannotated italic, private dimmed, exported overlay, error dot)
- [ ] Implement `basilisk/moduleChanged` notification handler -> incremental tree refresh
- [ ] Implement tree/flat view toggle with `workspaceState` persistence
- [ ] Implement module filter input box with glob support
- [ ] Implement `TypeHealthProvider` — `TreeDataProvider` with summary header
- [ ] Implement coverage bar rendering in description field
- [ ] Implement sort cycling (worst-first / best-first / alphabetical)
- [ ] Implement `BasiliskInfoProvider` — static tree with four sections
- [ ] Implement Feature Status toggle-on-click
- [ ] Implement Server Info section (version, binary, python, analysis mode, file count)
- [ ] Register `basilisk.refreshModuleExplorer` command
- [ ] Register `basilisk.toggleModuleExplorerView` command
- [ ] Register `basilisk.collapseModuleExplorer` command
- [ ] Register `basilisk.copyImportPath` command (clipboard: `from x.y import Z`)
- [ ] Register `basilisk.copyQualifiedName` command (clipboard: `x.y.Z`)
- [ ] Register `basilisk.refreshTypeHealth` command
- [ ] Register `basilisk.sortTypeHealth` command
- [ ] Register `basilisk.openWalkthrough` command
- [ ] Set context keys: `basilisk.serverState`, `basilisk.hasWorkspace`, `basilisk.moduleExplorerView`
- [ ] E2E test: activity bar icon appears, clicking opens sidebar
- [ ] E2E test: module explorer shows correct tree for test workspace
- [ ] E2E test: type health shows correct coverage for test workspace
- [ ] E2E test: copy import path produces correct `from x import y` string
- [ ] E2E test: feature toggle click changes setting and updates tree item

### Zed Extension
- [ ] Register `/modules` slash command
- [ ] Register `/symbols` slash command
- [ ] Register `/health` slash command
- [ ] Register `/basilisk` slash command
- [ ] Implement markdown tree formatting for module output
- [ ] Implement markdown table formatting for health output
- [ ] Implement argument completion for `/modules` and `/symbols` (module names)
- [ ] Test: `/modules` output matches `basilisk/workspaceModules` data
- [ ] Test: `/health` output matches `basilisk/typeHealth` data
- [ ] When Zed adds panel API: implement native panels using same LSP commands

### Neovim Plugin
- [ ] Implement `basilisk.modules` Lua module (split buffer, foldable tree)
- [ ] Implement tree rendering with `nvim_buf_set_lines` + virtual text for types
- [ ] Implement keybindings: `<CR>` open, `o` toggle, `r` refresh, `y` copy import, `q` close
- [ ] Implement `basilisk.health` Lua module (split buffer, colored highlights)
- [ ] Implement green/yellow/red highlights via `nvim_buf_add_highlight`
- [ ] Implement `basilisk.info` Lua module (floating window)
- [ ] Register `:BasiliskModules`, `:BasiliskHealth`, `:BasiliskInfo` commands
- [ ] Set default keymaps: `<leader>bm`, `<leader>bh`, `<leader>bi`
- [ ] Handle `basilisk/moduleChanged` via `vim.lsp.handlers` for live refresh
- [ ] Test: `:BasiliskModules` renders correct tree for test workspace
- [ ] Test: `:BasiliskHealth` renders correct coverage stats

### Polish
- [ ] Performance test: `basilisk/workspaceModules` < 100ms for 1000-file workspace
- [ ] Performance test: `basilisk/typeHealth` < 50ms for 1000-file workspace
- [ ] Performance test: `basilisk/moduleChanged` notification < 20ms per file change
- [ ] Accessibility audit: VS Code screen reader testing
- [ ] Final icon design for activity bar
- [ ] Documentation: add panel usage to README / user guide
