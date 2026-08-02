# Activity Panel Follow-ups {#EXTACT-PLAN}

Spec: [EXTENSION-ACTIVITY-PANEL-SPEC.md](../specs/EXTENSION-ACTIVITY-PANEL-SPEC.md).

The LSP module/health APIs, VS Code panels, Zed slash commands, and Neovim
module/health views are implemented and tested. This plan owns only remaining
behavioral gaps; the shipped inventory belongs in code and tests, not here.

## Server configuration domain {#EXTACT-PLAN-CONFIG}

- [ ] Define one typed server-settings model for initialization and
  `didChangeConfiguration` instead of parsing unrelated settings ad hoc.
- [ ] Apply the same validation/defaults on initialization and live reload.
- [ ] Log unknown fields without logging source, paths containing user data, or
  other sensitive values.

## Inlay-hint settings {#EXTACT-PLAN-INLAY}

- [ ] Gate parameter-name hints on `inlayHints.parameterNames` and variable-type
  hints on `inlayHints.variableTypes` in the server.
- [ ] Toggle each setting in a VS Code integration test and assert the returned
  hints change, not merely that the stored setting changed.

## Feature disablement {#EXTACT-PLAN-DISABLEMENT}

- [ ] Make `testExplorer.enabled = false` suppress initial discovery, discovery
  commands, and the VS Code `TestController`.
- [ ] Make `uv.enabled = false` suppress uv watchers and server command handling,
  matching the already-hidden client UI.
- [ ] Decide whether debugger disablement is a product feature. If retained,
  gate adapter registration and prove it with an integration test; otherwise
  remove the unused setting.

A setting may appear as a panel toggle only when flipping it has an observable,
tested effect. Do not add placeholder AI or profiler toggles.

## Modules panel context menus {#EXTACT-PLAN-CONTEXT-MENU}

`[EXTACT-MODULES-CONTEXT-MENU]` already specifies Go to Definition, Find
References, Rename, Copy Import Path, Copy Qualified Name, Organize Imports, and
Fix All. Only the two copy actions exist, and the tree has no multi-select — so a
panel that surfaces the whole workspace's type health offers almost no way to act
on it ([#111](https://github.com/Nimblesite/Basilisk/issues/111)).

- [ ] Give every node type a grouped menu. Module/file nodes: navigation (open,
  open to the side, reveal); LSP-gated fixes (`basilisk.fixFile`,
  `basilisk.organizeImports`, adopt/unadopt); copy; delete. Symbol nodes:
  navigation (definition, type definition, references, peek); kind-appropriate
  fixes; refactor (rename, safe delete); copy.
- [ ] Set `canSelectMany: true` on the tree view and make every handler
  selection-aware — VS Code passes `(focusedItem, selectedItems[])`, and each
  action must operate on the whole selection. Mixed-type selections filter to the
  applicable items and report what was skipped.
- [ ] Batch fix as one server round trip: a new `basilisk.fixFiles(uris[])`
  returning a single consolidated workspace edit, so it is one undo step rather
  than N sequential `basilisk.fixFile` calls. Progress notification with
  cancellation, and a result summary with real numbers.
- [ ] Delete safely. Files go to trash via `workspace.fs.delete({ useTrash:
  true })` with the watcher clearing diagnostics and rows immediately. Symbol
  delete is LSP-side and reference-checked: show the count with a Peek option
  before confirming, and return an undoable workspace edit. Confirmation modal
  by default behind `basilisk.confirmDelete`.
- [ ] Extend the `contextValue` scheme for precise targeting (`module.package`
  vs `module.file`, adopted/unadopted flags) and document the
  `/^(module|symbol)/` when-clause convention.
- [ ] Keep it LSP-driven ([LSPARCH-CMDREG]): every server-backed action is a
  command advertised in `executeCommandProvider`, auto-registered by
  `syncServerCommands`, never pre-registered in `contributes.commands`, and
  `when`-gated on `basilisk.serverState == 'running'`. Client-only actions
  (copy, reveal, file delete) stay client-side.
- [ ] Add the new spec IDs — `[EXTACT-MODULES-CONTEXT-MENU-MULTISELECT]`,
  `-DELETE`, `-BATCH-FIXES` — plus the new commands in the `[LSPARCH-CMDS]`
  table, and wire those IDs through code and tests.
- [ ] Cover it: Rust e2e per new LSP command; VSIX e2e asserting through the UI
  or internal state (never `getCommands(true)`) that multi-select fix across two
  fixture files drops the diagnostic counts, and that delete leaves no stale row
  or diagnostic.

Apply the same framework to the other views (test explorer) once it settles here.

## Quality follow-ups {#EXTACT-PLAN-QUALITY}

- [ ] Benchmark `basilisk.workspaceModules`, `basilisk.typeHealth`, and
  `basilisk/moduleChanged` on a reproducible 1,000-file fixture before setting
  numeric budgets.
- [ ] Run a VS Code screen-reader and keyboard-only accessibility audit.
- [ ] Add concise user documentation for module navigation, health summaries,
  filters, and copy actions.

## Acceptance {#EXTACT-PLAN-ACCEPTANCE}

- Every visible feature toggle changes server or editor behavior and has a
  behavior-level test.
- Disabled features do not advertise or execute their commands in any editor.
- Panel updates remain incremental, responsive, and accessible on the benchmark
  fixture.
