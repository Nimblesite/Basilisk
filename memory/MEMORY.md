# Basilisk Project Memory

## Status
Phase 1 (Foundation) is COMPLETE and all tests pass.

## Ruff Dependency
- NOT on crates.io — must use git dependency
- Pin: `tag = "0.12.12"` (compatible with rustc 1.87)
- `ruff_python_parser`, `ruff_python_ast`, `ruff_text_size` all from same tag
- ruff 0.13.x requires rustc 1.88+; 0.15.x requires rustc 1.91+

## AST API (ruff 0.12.x)
- `StmtIf` uses `elif_else_clauses: Vec<ElifElseClause>` NOT `orelse`
- `StmtTry` handlers are `Vec<ExceptHandler>` — NOT `&[Stmt]`
- `parse_module()` returns `Result<Parsed<ModModule>, ParseError>`
- `.into_syntax()` on `Parsed<T>` extracts the AST node

## Architecture
- Workspace: `Cargo.toml` at root with 9 crates
- Crate order: basilisk-db → basilisk-parser → basilisk-resolver → basilisk-checker → basilisk-cli
- Stub crates (empty): basilisk-stubs, basilisk-plugin, basilisk-lsp, basilisk-mojo
- Tests: integration tests only in `crates/*/tests/`
- Fixtures: `crates/basilisk-cli/tests/fixtures/*.py`

## Implemented Diagnostics
- BSK-E0001: Missing parameter type annotation
- BSK-E0002: Missing return type annotation
- NOTE: `self` is flagged as unannotated — no exception in Phase 1

## Next Phase
Phase 2: LSP server and VS Code extension
- `basilisk-lsp` crate stub already exists
- Add `tower-lsp` or `lsp-server` dependency
- Implement textDocument/diagnostic, hover, completions
