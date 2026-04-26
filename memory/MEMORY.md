# Basilisk Project Memory

## Status
Phase 1 COMPLETE. Phase 2 (VS Code Problems) COMPLETE.

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
- Stub crates: basilisk-stubs, basilisk-plugin, basilisk-mojo
- basilisk-lsp: implemented `check_source` wired to full pipeline
- Tests: integration tests only in `crates/*/tests/`
- Fixtures: `crates/basilisk-cli/tests/fixtures/*.py`

## Implemented Diagnostics
- BSK-E0001 through BSK-E0025 all implemented
- `self`/`cls` parameters exempt from BSK-E0001

## Phase 2: VS Code Problems (COMPLETE)
- `basilisk check --output json` emits JSON array: code/severity/message/path/line/col/end_line/end_col
- `basilisk-lsp::check_source` wired to full pipeline (powers LSP tests)
- VSIX extension: `vscode-extension/` — subprocess approach, no LSP needed
  - Runs binary on save/open, parses JSON, pushes to `DiagnosticCollection`
  - Config: `basilisk.executablePath` (default `"basilisk"`), `basilisk.enabled`
  - Build: `cd vscode-extension && npm install && npm run compile`
- LSP server deferred — see `docs/lsp-plan.md`

## Next Phase
Phase 3: strict-by-default rules (E0001–E0025), 80% PEP conformance

## Website
- Located at `/website/` — eleventy 3.x + eleventy-plugin-techdoc
- Build: `cd website && npm run build` (generates `_site/`)
- Dev: `npm start` (localhost:8080)
- Design: dark-only, orange (#e8500a) primary, purple (#7c3aed) accent
- Layout names from techdoc: `layouts/base.njk`, `layouts/docs.njk`, `layouts/blog.njk`
- Landing page uses `layouts/base.njk` (NOT home.njk — does not exist)
- Nunjucks processes markdown: avoid `{# ... #}` syntax in .md files (use HTML anchors instead)
- CSS: `src/assets/css/styles.css` — all custom properties, full component set
