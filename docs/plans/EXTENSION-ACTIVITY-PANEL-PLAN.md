# Extension Activity Panel — Implementation Plan {#EXTACT-PLAN}

> Spec: [EXTENSION-ACTIVITY-PANEL-SPEC.md](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md)

## Status {#EXTACT-PLAN-STATUS}

**Core panels are SHIPPED across all editors.** The three LSP custom commands
(`basilisk/workspaceModules`, `basilisk/typeHealth`, `basilisk/moduleChanged`) are
implemented in `crates/basilisk-lsp/src/server/activity_panel/`. VS Code panels
(`vscode-extension/src/module-explorer.ts`, type-health, basilisk-info; views +
walkthrough + icon registered in `package.json`), Zed slash commands
(`/modules`, `/symbols`, `/health`, `/basilisk` in `basilisk-zed/src/logic.rs`),
and Neovim modules (`:BasiliskModules`, `:BasiliskHealth`, `:BasiliskInfo`) are all
live with e2e tests.

What remains is **(1) making the Feature Status toggles real**, **(2)
performance/accessibility polish**, and a few cross-editor follow-ups.

---

## Remaining: Feature Status toggles — make them REAL {#EXTACT-PLAN-FEATURE-TOGGLES}

> Implements the "Not yet implemented" table in
> [EXTACT-INFO-FEATURE-STATUS](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md#EXTACT-INFO-FEATURE-STATUS).

**Background (audit, 2026-05-30).** The Feature Status section shipped eight
toggles. Six were no-ops: the extension wrote the setting via
`basilisk.toggleFeature`, but nothing on either side read it back. Root cause:
the LSP server's `did_change_configuration`
([`crates/basilisk-lsp/src/server/init.rs`](../../crates/basilisk-lsp/src/server/init.rs))
only parses `analysisMode` and `testExplorer.*`; every other forwarded field
(`inlayHints.*`, `ruff.*`, `uv.*`) is silently dropped. The no-op toggles were
**removed** from the panel; only `Type Checking` (`basilisk.enabled`, gates
diagnostic publication client-side) and `uv Integration` (`basilisk.uv.enabled`,
gates the uv surface in the panel) remain.

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
