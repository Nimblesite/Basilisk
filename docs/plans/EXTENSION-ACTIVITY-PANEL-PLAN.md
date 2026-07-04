# Extension Activity Panel — Implementation Plan {#EXTACT-PLAN}

> Spec: [EXTENSION-ACTIVITY-PANEL-SPEC.md](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md)

## Status {#EXTACT-PLAN-STATUS}

Core panels are SHIPPED across all editors. The three LSP custom commands
(`basilisk/workspaceModules`, `basilisk/typeHealth`, `basilisk/moduleChanged`) are
implemented in `crates/basilisk-lsp/src/server/activity_panel/`. Live with e2e tests:
VS Code panels (`vscode-extension/src/module-explorer.ts`, type-health, basilisk-info;
views + walkthrough + icon in `package.json`), Zed slash commands
(`/modules`, `/symbols`, `/health`, `/basilisk` in `basilisk-zed/src/logic.rs`),
and Neovim modules (`:BasiliskModules`, `:BasiliskHealth`, `:BasiliskInfo`).

Remaining: **(1) make the Feature Status toggles real**, **(2)
performance/accessibility polish**, and cross-editor follow-ups.

---

## Shipped Panel Inventory {#EXTACT-PLAN-SHIPPED-INVENTORY}

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
- [x] Implement explicit module sort picker (module name / path / type coverage)
- [x] Implement `BasiliskInfoProvider` — static tree with four sections
- [x] Implement Feature Status toggle-on-click
- [x] Implement Server Info section (version, binary, python, analysis mode, file count)
- [x] Register `basilisk.refreshModuleExplorer` command
- [x] Register `basilisk.toggleModuleExplorerView` command
- [x] Collapse All uses VS Code's native `showCollapseAll` — no contributed command (issue #113)
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
toggles; six were no-ops — the extension wrote the setting via
`basilisk.toggleFeature`, but nothing read it back. Root cause: the LSP server's
`did_change_configuration`
([`crates/basilisk-lsp/src/server/init.rs`](../../crates/basilisk-lsp/src/server/init.rs))
parses only `analysisMode` and `testExplorer.*`; every other forwarded field
(`inlayHints.*`, `ruff.*`, `uv.*`) is silently dropped. The no-op toggles were
**removed**; only `Type Checking` (`basilisk.enabled`, gates diagnostic
publication client-side) and `uv Integration` (`basilisk.uv.enabled`, gates the
uv surface in the panel) remain.

A toggle returns to the panel ONLY when both are true:
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
      hints on `inlayHints.variableTypes` (currently both emitted unconditionally).
- [ ] VSIX test: open a file with call-site params, toggle `parameterNames` off,
      assert `vscode.executeInlayHintProvider` returns no parameter hints; repeat
      for `variableTypes`.

### Formatter Engine {#EXTACT-PLAN-FORMATTER-TOGGLE}
The external `ruff` binary is jettisoned — there is no `ruff.enabled`/`ruff.executablePath`
to honor. Formatting is the Ruff formatter embedded in the Basilisk binary, in-process
([LSPFMT-DECISION](../specs/LSP-FORMATTING-SPEC.md#LSPFMT-DECISION)). The only setting is
the `basilisk.formatter` engine selector ([LSPFMT-CONFIG](../specs/LSP-FORMATTING-SPEC.md#LSPFMT-CONFIG)).
- [ ] When `basilisk.formatter` is `"none"`: do not advertise `documentFormattingProvider`
      / `documentRangeFormattingProvider` in `formatting.rs` / `server/init.rs`, so no
      Basilisk formatter appears in any editor. (Native import hygiene stays available —
      it is not gated by this flag.)
- [ ] VSIX test: set `basilisk.formatter` to `"none"`, assert formatting is not offered;
      set it back to `"ruff"`, assert formatting works with no `ruff` binary installed.

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

### AI Suggestions / Profiler toggles {#EXTACT-PLAN-FUTURE-TOGGLES}
- [ ] AI Suggestions: no provider exists. Do not surface a toggle until the
      `LSP-AI-PLAN.md` work lands and a provider actually consumes
      `basilisk.aiTyping.*`. The dead `aiTyping.*` settings were removed from
      `package.json`.
- [ ] Profiler: there is no `basilisk.profiler.enabled` gate; the profiler is always
      available. Only add a toggle if disabling it becomes meaningful.

---

## Remaining: polish & cross-editor follow-ups {#EXTACT-PLAN-POLISH}

- [ ] Performance test: `basilisk/workspaceModules` < 100ms for 1000-file workspace.
- [ ] Performance test: `basilisk/typeHealth` < 50ms for 1000-file workspace.
- [ ] Performance test: `basilisk/moduleChanged` notification < 20ms per file change.
- [ ] Accessibility audit: VS Code screen reader testing.
- [ ] Documentation: add panel usage to README / user guide.
- [ ] Neovim test: `:BasiliskModules` renders correct tree for test workspace.
- [ ] Neovim test: `:BasiliskHealth` renders correct coverage stats.
- [ ] Zed: when Zed adds a panel API, implement native panels using the same LSP commands.
