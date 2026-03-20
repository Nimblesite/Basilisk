#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    missing_docs,
    clippy::needless_raw_string_hashes,
    dead_code,
    unused_imports
)]

#[path = "lsp/ws_test_common.rs"]
mod ws_test_common;

#[path = "lsp/ws_test_cross_module.rs"]
mod ws_test_cross_module;
#[path = "lsp/ws_test_document_highlight.rs"]
mod ws_test_document_highlight;
#[path = "lsp/ws_test_document_symbols.rs"]
mod ws_test_document_symbols;
#[path = "lsp/ws_test_find_references.rs"]
mod ws_test_find_references;
#[path = "lsp/ws_test_folding_ranges.rs"]
mod ws_test_folding_ranges;
#[path = "lsp/ws_test_formatting.rs"]
mod ws_test_formatting;
#[path = "lsp/ws_test_goto_definition.rs"]
mod ws_test_goto_definition;
#[path = "lsp/ws_test_hierarchies.rs"]
mod ws_test_hierarchies;
#[path = "lsp/ws_test_rename.rs"]
mod ws_test_rename;
#[path = "lsp/ws_test_selection_ranges.rs"]
mod ws_test_selection_ranges;
#[path = "lsp/ws_test_workspace_symbols.rs"]
mod ws_test_workspace_symbols;
