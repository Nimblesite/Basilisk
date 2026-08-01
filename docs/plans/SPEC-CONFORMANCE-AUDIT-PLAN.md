# Spec/Code Remediation Backlog {#CONFAUDIT-BACKLOG}

This tracker contains only confirmed, unresolved deviations where the spec is
the intended behavior. Closed audit history is intentionally omitted. Each fix
requires a failing behavior test before the implementation change.

Every row carries its tracker issue. A row leaves this file only when the code,
the test, and the spec agree — closing the issue and deleting the row are the
same act.

## uv and test execution {#CONFAUDIT-UV-TESTS}

| Spec ID | Issue | Current deviation | Location |
|---|---|---|---|
| `LSPUV-WORKSPACE-DETECTION` | [#204](https://github.com/Nimblesite/Basilisk/issues/204) | Workspace `exclude` patterns are parsed but not subtracted from resolved members. | `basilisk-uv/src/workspace.rs` |
| `LSPUV-CONFIG-BINARY-RESOLUTION` | [#205](https://github.com/Nimblesite/Basilisk/issues/205) | The resolver cascade is unused; command execution spawns bare `uv` from `PATH`. | `basilisk-uv/src/binary.rs`, `basilisk-lsp/src/uv_commands.rs` |
| `LSPUV-LOCK-IMPORT-MAPPING` | [#207](https://github.com/Nimblesite/Basilisk/issues/207) | Distribution metadata and known mappings exist, but the documented site-packages top-level-module scan does not. | `basilisk-uv/src/import_map.rs` |
| `LSPTEST-TEST-EXECUTION-UV-AWARE-ENVIRONMENT-VARIABLES` | [#211](https://github.com/Nimblesite/Basilisk/issues/211) | Test processes do not set `PYTHONDONTWRITEBYTECODE=1`. | `basilisk-lsp/src/test_discovery/runner.rs` |
| `LSPTEST-UV-INTEGRATION-COVERAGE` | [#211](https://github.com/Nimblesite/Basilisk/issues/211) | Coverage uses bare `--cov` instead of the specified source root. | `basilisk-lsp/src/test_discovery/runner.rs` |
| `LSPTEST-TEST-EXECUTION-UV-AWARE-PYTEST-RESOLUTION-CASCADE` | [#211](https://github.com/Nimblesite/Basilisk/issues/211) | Explicit `pytestPath` does not pre-empt `uv run`. | `basilisk-lsp/src/test_discovery/runner.rs` |

## Refactoring {#CONFAUDIT-REFACTORING}

| Spec ID | Issue | Current deviation | Location |
|---|---|---|---|
| `REFACTOR-SIGNATURE-OPS` | [#213](https://github.com/Nimblesite/Basilisk/issues/213) | Parameter reordering edits only the definition and can break positional callers. | `code_actions/refactor/change_signature.rs` |
| `REFACTOR-MOVE-ALGO` | [#214](https://github.com/Nimblesite/Basilisk/issues/214) | Move copies every preceding import and does not fully update/prune importers. | `code_actions/refactor/move_symbol.rs` |
| `REFACTOR-EXTRACT-VAR-ALGO` | [#215](https://github.com/Nimblesite/Basilisk/issues/215) | Replace-all uses substring matching rather than AST-equivalent expressions. | `code_actions/refactor/extract.rs` |
| `REFACTOR-EXTRACT-FUNC-EDGE` | [#216](https://github.com/Nimblesite/Basilisk/issues/216) | Control-flow and scope rejection is incomplete for `return`, `global`, `nonlocal`, and loop boundaries. | `code_actions/refactor/extract_function.rs` |
| `REFACTOR-FORMATTER` | [#217](https://github.com/Nimblesite/Basilisk/issues/217) | Generated text trims trailing whitespace but does not run the configured formatter. | `code_actions/refactor/helpers.rs` |
| `REFACTOR-ABSTRACT-ALGO` | [#218](https://github.com/Nimblesite/Basilisk/issues/218) | Base lookup is direct and same-module; MRO and configured body style are not applied. | `code_actions/refactor/abstract_methods.rs` |
| `REFACTOR-RENAME-VALIDATE` | [#219](https://github.com/Nimblesite/Basilisk/issues/219) | `validate_rename` computes every `RenameRejection`, but only `InvalidIdentifier` short-circuits; shadowing and builtin-conflict rejections are dropped and the rename proceeds. | `basilisk-lsp/src/references.rs` |

## Adoption and CLI {#CONFAUDIT-ADOPTION-CLI}

| Spec ID | Issue | Current deviation | Location |
|---|---|---|---|
| `AUTOFIX-ADOPTION-RULES` | [#221](https://github.com/Nimblesite/Basilisk/issues/221) | Partially closed. The CLI now graduates on recompute (`run_adopt` drops entries whose debt is gone, covered by `run_adopt_rerun_graduates_fixed_rules`), but graduation requires an explicit re-run — fixing the last instance does not remove the override on its own — and the LSP adopt handlers never graduate at all. | `basilisk-cli/src/adopt.rs`, `basilisk-lsp/src/server/adoption.rs` |
| `AUTOFIX-ADOPTION-FLOW` | [#222](https://github.com/Nimblesite/Basilisk/issues/222) | Partially closed. Persistence is resolved by design — demotions are written into the owning root's configuration and reload with it, so they survive re-checks. The safe-autofix step is still missing: `execute_adopt_file`, `execute_adopt_workspace`, and CLI `run_adopt` record the current error codes without first applying safe fixes, so debt that autofix would have erased is demoted instead. | `basilisk-lsp/src/server/adoption.rs`, `basilisk-cli/src/adopt.rs` |
| `CHKARCH-CLI-EXITCODES` | [#227](https://github.com/Nimblesite/Basilisk/issues/227) | Partially closed. A `pep` rule resolved to `disabled` does exit `2` (`pipeline/mod.rs` raises `PipelineError::Config`, `main.rs` maps it, `pep_tag_disable_is_a_config_error` guards it). A `pyproject.toml` that fails to parse is still silently discarded and the run falls back to defaults, exiting `0`. | `basilisk-config/src/lib.rs`, `basilisk-config/src/parse.rs` |

## Profiler and incremental analysis {#CONFAUDIT-PROFILER-INCREMENTAL}

| Spec ID | Issue | Current deviation | Location |
|---|---|---|---|
| `PROFILE-CONFIG-CODES` | [#234](https://github.com/Nimblesite/Basilisk/issues/234) | `BSK-PROF-GIL` is declared in `basilisk-common` but nothing constructs it. Only `BSK-PROF-LINE` and `BSK-PROF-FUNC` are emitted, so the spec's GIL-contention diagnostic never reaches a user. | `basilisk-common/src/lib.rs`, `basilisk-lsp/src/profiler/diagnostics.rs` |
| `ANALYSIS-INCR-DEBOUNCE` | [#244](https://github.com/Nimblesite/Basilisk/issues/244) | The watched-file debounce aborts and replaces the pending task, discarding its `reload_targets` instead of coalescing them. Two notifications for different files inside the 200 ms window drop the first file's re-analysis. The spec text was deliberately written not to enshrine replace-not-coalesce, so no spec change is needed. | `basilisk-lsp/src/server/document.rs`, `basilisk-lsp/src/server/mod.rs` |

## Verification {#CONFAUDIT-VERIFICATION}

For each row: add a spec-ID-linked regression test, observe the intended failure,
fix the narrow behavior, then run the focused suite plus `make test`. Remove the
row immediately after the code, test, and spec agree.

Rows marked *Partially closed* carry a specific residue. Do not close the issue
on the part that already works — the remaining clause is the acceptance gate, and
it needs its own failing test first.
