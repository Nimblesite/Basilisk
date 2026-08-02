//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Coverage-boost test binary: aggregates new WebSocket LSP E2E modules that
// raise basilisk-lsp coverage by adding MORE user interactions per test and
// MORE assertions per interaction. Each module targets a specific handler
// surface whose lines were previously unreached.
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

#[path = "lsp/ws_test_call_hierarchy_extended.rs"]
mod ws_test_call_hierarchy_extended;
#[path = "lsp/ws_test_cross_file_navigation.rs"]
mod ws_test_cross_file_navigation;
#[path = "lsp/ws_test_document_lifecycle.rs"]
mod ws_test_document_lifecycle;
#[path = "lsp/ws_test_features_edge.rs"]
mod ws_test_features_edge;
#[path = "lsp/ws_test_file_operations.rs"]
mod ws_test_file_operations;
#[path = "lsp/ws_test_fix_commands.rs"]
mod ws_test_fix_commands;
#[path = "lsp/ws_test_folding_extended.rs"]
mod ws_test_folding_extended;
#[path = "lsp/ws_test_selection_extended.rs"]
mod ws_test_selection_extended;
#[path = "lsp/ws_test_test_explorer.rs"]
mod ws_test_test_explorer;
#[path = "lsp/ws_test_type_definition.rs"]
mod ws_test_type_definition;
