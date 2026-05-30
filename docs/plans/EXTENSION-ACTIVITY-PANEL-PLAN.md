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

## Feature Status toggles — make them REAL {#EXTACT-PLAN-FEATURE-TOGGLES}

> Implements the "Not yet implemented" table in
> [EXTACT-INFO-FEATURE-STATUS](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-FEATURE-STATUS).

**Background (audit, 2026-05-30).** The Feature Status section shipped eight
toggles. Six of them were no-ops: the extension wrote the setting via
`basilisk.toggleFeature`, but nothing on either side read it back. Root cause:
the LSP server's `did_change_configuration`
([`crates/basilisk-lsp/src/server/init.rs`](../../crates/basilisk-lsp/src/server/init.rs))
only parses `analysisMode` and `testExplorer.*` — every other forwarded field
(`inlayHints.*`, `ruff.*`, `uv.*`) is silently dropped. The no-op toggles were
**removed** from the panel; only `Type Checking` (`basilisk.enabled`, gates
diagnostic publication client-side) and `uv Integration` (`basilisk.uv.enabled`,
gates the uv surface in the panel) remain.

A toggle returns to the panel ONLY when both of these are true:
1. Flipping the setting produces a real, observable effect that matches the label.
2. A VSIX test under `vscode-extension/src/test/suite/` proves that effect
   (toggle the setting, assert the behavior changed — not merely that the setting
   value flipped or that a command exists).

### Server config plumbing (prerequisite) {#EXTACT-PLAN-CONFIG-STRUCT}
- [ ] Define a single serde `Deserialize` config struct in `basilisk-lsp` that
      mirrors the JSON forwarded by `readBasiliskSettings`
      (`inlayHints`, `ruff`, `uv`, `testExplorer`, `analysisMode`).
- [ ] Parse it once in `initialize` (from `params.initialization_options`) and
      again in `did_change_configuration`; store it behind the server's `RwLock`
      next to `test_config`.
- [ ] Reject/log unknown fields so future drift is visible (no more silent drops).

### Inlay Hints (Params) / (Types) {#EXTACT-PLAN-INLAY-TOGGLES}
- [ ] In `crates/basilisk-lsp/src/inlay_hints.rs` / `server/handlers/features.rs`,
      gate parameter-name hints on `inlayHints.parameterNames` and variable-type
      hints on `inlayHints.variableTypes` (currently both are emitted unconditionally).
- [ ] VSIX test: open a file with call-site params, toggle `parameterNames` off,
      assert `vscode.executeInlayHintProvider` returns no parameter hints; repeat
      for `variableTypes`.

### Ruff Integration {#EXTACT-PLAN-RUFF-TOGGLE}
- [ ] When `ruff.enabled` is false: skip ruff-backed code actions / formatting /
      organize-imports in `code_actions/` and `formatting.rs`, and do not advertise
      `basilisk.organizeImports` as an available action for the document.
- [ ] Honor `ruff.executablePath` instead of resolving `ruff` from PATH.
- [ ] VSIX test: toggle `ruff.enabled` off, assert organize-imports code action is
      absent / formatting is a no-op.

### Test Explorer {#EXTACT-PLAN-TEST-EXPLORER-TOGGLE}
- [ ] `testExplorer.enabled` currently only gates auto-discovery-on-save. Make it
      gate the whole feature: when false, do not run initial discovery
      (`spawn_initial_test_discovery`), do not advertise the test commands, and have
      the extension's `test-explorer.ts` skip registering the `TestController`.
- [ ] VSIX test: toggle off, assert no `TestController` items appear.

### Debugger {#EXTACT-PLAN-DEBUGGER-TOGGLE}
- [ ] Decide whether a debugger on/off switch is wanted at all. If yes: declare
      `basilisk.debugger.enabled` in `package.json` and gate
      `registerDebugSupport` (`vscode-extension/src/extension.ts`) on it.
- [ ] VSIX test: toggle off, assert the debug adapter factory is not registered.

### uv Integration (server-side) {#EXTACT-PLAN-UV-TOGGLE}
- [ ] The panel already hides uv actions when `uv.enabled` is false, but the server
      still executes uv commands if invoked elsewhere. Gate the uv command handlers
      (`server/uv_handlers.rs`) and uv file watchers on `uv.enabled` for consistency.
- [ ] VSIX test already needed: assert uv Quick Actions disappear when toggled off
      (added in this PR for the client-side effect).

### AI Suggestions / Profiler toggles {#EXTACT-PLAN-FUTURE-TOGGLES}
- [ ] AI Suggestions: no provider exists. Do not surface a toggle until the
      `LSP-AI-PLAN.md` work lands and a provider actually consumes
      `basilisk.aiTyping.*`. The dead `aiTyping.*` settings were removed from
      `package.json`.
- [ ] Profiler: there is no `basilisk.profiler.enabled` gate; the profiler is always
      available. Only add a toggle if disabling it becomes meaningful.
