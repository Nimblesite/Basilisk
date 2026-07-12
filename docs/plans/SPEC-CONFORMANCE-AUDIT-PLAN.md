# Spec/Code Remediation Backlog {#CONFAUDIT-BACKLOG}

This tracker contains only confirmed, unresolved deviations where the spec is
the intended behavior. Closed audit history is intentionally omitted. Each fix
requires a failing behavior test before the implementation change.

## uv and test execution {#CONFAUDIT-UV-TESTS}

| Spec ID | Current deviation | Location |
|---|---|---|
| `LSPUV-WORKSPACE-DETECTION` | Workspace `exclude` patterns are parsed but not subtracted from resolved members. | `basilisk-uv/src/workspace.rs` |
| `LSPUV-CONFIG-BINARY-RESOLUTION` | The resolver cascade is unused; command execution spawns bare `uv` from `PATH`. | `basilisk-uv/src/binary.rs`, `basilisk-lsp/src/uv_commands.rs` |
| `LSPUV-LOCK-IMPORT-MAPPING` | Distribution metadata and known mappings exist, but the documented site-packages top-level-module scan does not. | `basilisk-uv/src/import_map.rs` |
| `LSPTEST-TEST-EXECUTION-UV-AWARE-ENVIRONMENT-VARIABLES` | Test processes do not set `PYTHONDONTWRITEBYTECODE=1`. | `basilisk-lsp/src/test_discovery/runner.rs` |
| `LSPTEST-UV-INTEGRATION-COVERAGE` | Coverage uses bare `--cov` instead of the specified source root. | `basilisk-lsp/src/test_discovery/runner.rs` |
| `LSPTEST-TEST-EXECUTION-UV-AWARE-PYTEST-RESOLUTION-CASCADE` | Explicit `pytestPath` does not pre-empt `uv run`. | `basilisk-lsp/src/test_discovery/runner.rs` |

## Refactoring {#CONFAUDIT-REFACTORING}

| Spec ID | Current deviation | Location |
|---|---|---|
| `REFACTOR-SIGNATURE-OPS` | Parameter reordering edits only the definition and can break positional callers. | `code_actions/refactor/change_signature.rs` |
| `REFACTOR-MOVE-ALGO` | Move copies every preceding import and does not fully update/prune importers. | `code_actions/refactor/move_symbol.rs` |
| `REFACTOR-EXTRACT-VAR-ALGO` | Replace-all uses substring matching rather than AST-equivalent expressions. | `code_actions/refactor/extract.rs` |
| `REFACTOR-EXTRACT-FUNC-EDGE` | Control-flow and scope rejection is incomplete for `return`, `global`, `nonlocal`, and loop boundaries. | `code_actions/refactor/extract_function.rs` |
| `REFACTOR-FORMATTER` | Generated text trims trailing whitespace but does not run the configured formatter. | `code_actions/refactor/helpers.rs` |
| `REFACTOR-ABSTRACT-ALGO` | Base lookup is direct and same-module; MRO and configured body style are not applied. | `code_actions/refactor/abstract_methods.rs` |

## Adoption and CLI {#CONFAUDIT-ADOPTION-CLI}

| Spec ID | Current deviation | Location |
|---|---|---|
| `AUTOFIX-ADOPTION-FLOW`, `AUTOFIX-ADOPTION-RULES` | Adoption demotions are applied only by command handlers, are lost on ordinary rechecks, and auto-graduation never runs in production. | `basilisk-lsp/src/server/adoption.rs` |
| `CHKARCH-CLI-EXITCODES` | Malformed configuration falls back to defaults, so configuration exit code `2` is never produced. | `basilisk-cli/src/main.rs`, `basilisk-config/src/lib.rs` |

## Verification {#CONFAUDIT-VERIFICATION}

For each row: add a spec-ID-linked regression test, observe the intended failure,
fix the narrow behavior, then run the focused suite plus `make test`. Remove the
row immediately after the code, test, and spec agree.
