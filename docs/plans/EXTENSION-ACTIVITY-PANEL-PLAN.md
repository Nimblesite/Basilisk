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
