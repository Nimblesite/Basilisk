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
- [x] Implement `basilisk/workspaceModules` handler in `basilisk-lsp`
- [x] Implement `ModuleNode` / `SymbolNode` construction from `ResolvedModule`
- [x] Implement `scope` parameter filtering for `basilisk/workspaceModules`
- [x] Implement `basilisk/moduleChanged` notification on re-analysis
- [x] Implement 300ms debounce for `basilisk/moduleChanged`
- [x] Implement `basilisk/typeHealth` handler
- [x] Implement `HealthStats` computation (annotated vs unannotated symbol counting)
- [x] Implement `ModuleHealth` per-module breakdown
- [x] Wire adoption state into `ModuleHealth.adopted`
- [x] Add all three custom commands to `LSP-ARCHITECTURE-SPEC.md`
- [x] Unit tests: `basilisk/workspaceModules` returns correct tree for test workspace
- [x] Unit tests: `basilisk/typeHealth` returns correct coverage percentages
- [x] Unit tests: `basilisk/moduleChanged` fires after file change, not before

### VS Code Extension
- [x] Create `basilisk-icon.svg` (monochrome, 24x24, light + dark theme compatible)
- [x] Add `viewsContainers` and `views` to `package.json`
- [x] Add `viewsWelcome` entries to `package.json`
- [x] Add all menu contributions to `package.json`
- [x] Add walkthrough contribution to `package.json`
- [x] Implement `ModuleExplorerProvider` — `TreeDataProvider` with lazy child loading
- [x] Implement module tree item rendering (codicons, descriptions, tooltips, click-to-open)
- [x] Implement symbol decorations (unannotated italic, private dimmed, exported overlay, error dot)
- [x] Implement `basilisk/moduleChanged` notification handler -> incremental tree refresh
- [x] Implement tree/flat view toggle with `workspaceState` persistence
- [x] Implement module filter input box with glob support
- [x] Implement `TypeHealthProvider` — `TreeDataProvider` with summary header
- [x] Implement coverage bar rendering in description field
- [x] Implement sort cycling (worst-first / best-first / alphabetical)
- [x] Implement `BasiliskInfoProvider` — static tree with four sections
- [x] Implement Feature Status toggle-on-click
- [x] Implement Server Info section (version, binary, python, analysis mode, file count)
- [x] Register `basilisk.refreshModuleExplorer` command
- [x] Register `basilisk.toggleModuleExplorerView` command
- [x] Register `basilisk.collapseModuleExplorer` command
- [x] Register `basilisk.copyImportPath` command (clipboard: `from x.y import Z`)
- [x] Register `basilisk.copyQualifiedName` command (clipboard: `x.y.Z`)
- [x] Register `basilisk.refreshTypeHealth` command
- [x] Register `basilisk.sortTypeHealth` command
- [x] Register `basilisk.openWalkthrough` command
- [x] Set context keys: `basilisk.serverState`, `basilisk.hasWorkspace`, `basilisk.moduleExplorerView`
- [x] E2E test: activity bar icon appears, clicking opens sidebar
- [x] E2E test: module explorer shows correct tree for test workspace
- [x] E2E test: type health shows correct coverage for test workspace
- [x] E2E test: copy import path produces correct `from x import y` string
- [x] E2E test: feature toggle click changes setting and updates tree item

### Zed Extension
- [x] Register `/modules` slash command
- [x] Register `/symbols` slash command
- [x] Register `/health` slash command
- [x] Register `/basilisk` slash command
- [x] Implement markdown tree formatting for module output
- [x] Implement markdown table formatting for health output
- [x] Implement argument completion for `/modules` and `/symbols` (module names)
- [x] Test: `/modules` output matches `basilisk/workspaceModules` data
- [x] Test: `/health` output matches `basilisk/typeHealth` data
- [ ] When Zed adds panel API: implement native panels using same LSP commands

### Neovim Plugin
- [x] Implement `basilisk.modules` Lua module (split buffer, foldable tree)
- [x] Implement tree rendering with `nvim_buf_set_lines` + virtual text for types
- [x] Implement keybindings: `<CR>` open, `o` toggle, `r` refresh, `y` copy import, `q` close
- [x] Implement `basilisk.health` Lua module (split buffer, colored highlights)
- [x] Implement green/yellow/red highlights via `nvim_buf_add_highlight`
- [x] Implement `basilisk.info` Lua module (floating window)
- [x] Register `:BasiliskModules`, `:BasiliskHealth`, `:BasiliskInfo` commands
- [x] Set default keymaps: `<leader>bm`, `<leader>bh`, `<leader>bi`
- [x] Handle `basilisk/moduleChanged` via `vim.lsp.handlers` for live refresh
- [ ] Test: `:BasiliskModules` renders correct tree for test workspace
- [ ] Test: `:BasiliskHealth` renders correct coverage stats

### Polish
- [ ] Performance test: `basilisk/workspaceModules` < 100ms for 1000-file workspace
- [ ] Performance test: `basilisk/typeHealth` < 50ms for 1000-file workspace
- [ ] Performance test: `basilisk/moduleChanged` notification < 20ms per file change
- [ ] Accessibility audit: VS Code screen reader testing
- [x] Final icon design for activity bar
- [ ] Documentation: add panel usage to README / user guide
