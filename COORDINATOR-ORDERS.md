# COORDINATOR ORDERS — Phase 5

## STATUS: Phases 0-4 COMPLETE. 110/110 tests. Clippy clean.

## Phase 5 Tasks — Advanced LSP Features

### TASK K: Document Highlight (NEW highlight.rs + references.rs + server.rs)
**Assigned to: Tiger Woods / Cline**

Add `textDocument/documentHighlight`:
1. In `references.rs`: make `find_identifier_occurrences` and `is_in_string_or_comment` `pub(crate)` instead of private
2. Create `crates/basilisk-lsp/src/highlight.rs`: `pub fn document_highlights(resolved: &ResolvedModule, source: &str, byte_offset: usize) -> Vec<DocumentHighlight>` — reuse `references::find_identifier_occurrences` to find all occurrences, classify definition as WRITE, others as READ
3. In `server.rs`: add `document_highlight_provider: Some(OneOf::Left(true))`, implement `document_highlight()` method
4. In `lib.rs`: add `pub mod highlight;`
5. Add WS test: `test_ws_document_highlight`
6. Lock files: `highlight.rs` (new), `references.rs`, `server.rs`, `lib.rs`, `lsp_ws_tests.rs`

### TASK L: Call Hierarchy (NEW call_hierarchy.rs + server.rs)
**Assigned to: Sub-agent**

Add call hierarchy (prepare + incoming + outgoing):
1. Create `crates/basilisk-lsp/src/call_hierarchy.rs` with three pub functions using CallSite data from ResolvedModule
2. In `server.rs`: add `call_hierarchy_provider`, implement 3 methods
3. In `lib.rs`: add `pub mod call_hierarchy;`
4. Add WS test: `test_ws_call_hierarchy`
5. Lock files: `call_hierarchy.rs` (new), `server.rs`, `lib.rs`, `lsp_ws_tests.rs`

### TASK M: Type Hierarchy (NEW type_hierarchy.rs + server.rs)
**Assigned to: Sub-agent**

Add type hierarchy (prepare + supertypes + subtypes):
1. Create `crates/basilisk-lsp/src/type_hierarchy.rs` using ClassInfo.bases from ResolvedModule
2. In `server.rs`: add `type_hierarchy_provider`, implement 3 methods
3. In `lib.rs`: add `pub mod type_hierarchy;`
4. Add WS test: `test_ws_type_hierarchy`
5. Lock files: `type_hierarchy.rs` (new), `server.rs`, `lib.rs`, `lsp_ws_tests.rs`

### TASK N: Code Lens (NEW code_lens.rs + server.rs)
**Assigned to: Sub-agent**

Add `textDocument/codeLens` showing reference counts:
1. Create `crates/basilisk-lsp/src/code_lens.rs` — count references for each function/class, return CodeLens with "N references"
2. In `server.rs`: add `code_lens_provider`, implement `code_lens()` method
3. In `lib.rs`: add `pub mod code_lens;`
4. Add WS test: `test_ws_code_lens`
5. Lock files: `code_lens.rs` (new), `server.rs`, `lib.rs`, `lsp_ws_tests.rs`

## EXECUTION ORDER
Tiger does K first (makes references fns pub(crate)), then Sub-L, then Sub-M, then Sub-N.

## RULES
- Lock files BEFORE editing via TMC
- NO `.unwrap()` or `.expect()` in production code
- NO `println!` in production code
- Run `cargo clippy --package basilisk-lsp -- -D warnings` after changes
- Run `cargo test --package basilisk-lsp` after changes
- Report completion to COORDINATOR via TMC message
