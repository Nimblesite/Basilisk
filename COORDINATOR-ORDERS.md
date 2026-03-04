# COORDINATOR ORDERS — Phase 4

## STATUS: Phases 0-3 COMPLETE. 106/106 tests. Clippy clean.

## Phase 4 Tasks — Workspace Features + Formatting

### TASK G: Workspace Symbols (symbols.rs + server.rs)
**Assigned to: Tiger Woods / Cline**

Add `workspace/symbol` handler:
1. In `crates/basilisk-lsp/src/symbols.rs`: add `pub fn workspace_symbols(documents: &[(Url, ResolvedModule, String)], query: &str) -> Vec<SymbolInformation>` — aggregate document symbols from all open docs, filter by query string
2. In `crates/basilisk-lsp/src/server.rs`: add `workspaceSymbolProvider: true` to capabilities, implement `symbol()` method that iterates `self.documents` DashMap and calls the new function
3. Import `SymbolInformation` and `WorkspaceSymbolParams` from tower_lsp
4. Add WS test: `test_ws_workspace_symbols` — open 2 docs, query symbols, verify results from both
5. Lock files: `symbols.rs`, `server.rs`, `lsp_ws_tests.rs`

### TASK H: Format Document (NEW formatting.rs + server.rs)
**Assigned to: Sub-agent**

Add `textDocument/formatting` via Ruff delegation:
1. Create `crates/basilisk-lsp/src/formatting.rs`: `pub fn format_document(source: &str, file_path: &str) -> Option<Vec<TextEdit>>` — spawn `ruff format --stdin-filename <path> -` with source on stdin, capture stdout, return single TextEdit replacing entire document if different
2. In `server.rs`: add `documentFormattingProvider: true`, implement `formatting()` method
3. In `lib.rs`: add `pub mod formatting;`
4. Add WS test: `test_ws_format_document`
5. Lock files: `formatting.rs` (new), `server.rs`, `lib.rs`, `lsp_ws_tests.rs`

### TASK I: Folding Ranges (server.rs)
**Assigned to: Sub-agent**

Add `textDocument/foldingRange`:
1. Create `crates/basilisk-lsp/src/folding.rs`: `pub fn folding_ranges(resolved: &ResolvedModule, source: &str) -> Vec<FoldingRange>` — emit FoldingRange for each function def_span, class def_span, and consecutive import block
2. In `server.rs`: add `foldingRangeProvider: true`, implement `folding_range()` method
3. In `lib.rs`: add `pub mod folding;`
4. Add WS test: `test_ws_folding_ranges`
5. Lock files: `folding.rs` (new), `server.rs`, `lib.rs`, `lsp_ws_tests.rs`

### TASK J: Selection Ranges (server.rs)
**Assigned to: Sub-agent**

Add `textDocument/selectionRange`:
1. Create `crates/basilisk-lsp/src/selection.rs`: `pub fn selection_ranges(resolved: &ResolvedModule, source: &str, positions: &[Position]) -> Vec<SelectionRange>` — build nested range tree from spans (identifier → param list → function → class → module)
2. In `server.rs`: add `selectionRangeProvider: true`, implement `selection_range()` method
3. In `lib.rs`: add `pub mod selection;`
4. Add WS test: `test_ws_selection_ranges`
5. Lock files: `selection.rs` (new), `server.rs`, `lib.rs`, `lsp_ws_tests.rs`

## RULES
- Lock files BEFORE editing via TMC
- NO `.unwrap()` or `.expect()` in production code
- NO `println!` in production code
- Run `cargo clippy --package basilisk-lsp -- -D warnings` after changes
- Run `cargo test --package basilisk-lsp` after changes
- Report completion to COORDINATOR via TMC message
