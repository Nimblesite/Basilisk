//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
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

#[path = "lsp/ws_test_analysis_modes.rs"]
mod ws_test_analysis_modes;
#[path = "lsp/ws_test_basics.rs"]
mod ws_test_basics;
#[path = "lsp/ws_test_capabilities.rs"]
mod ws_test_capabilities;
#[path = "lsp/ws_test_diagnostics_rules.rs"]
mod ws_test_diagnostics_rules;
#[path = "lsp/ws_test_diagnostics_rules_advanced.rs"]
mod ws_test_diagnostics_rules_advanced;
#[path = "lsp/ws_test_diagnostics_structure.rs"]
mod ws_test_diagnostics_structure;
#[path = "lsp/ws_test_shutdown.rs"]
mod ws_test_shutdown;
#[path = "lsp/ws_test_type_checking_toggle.rs"]
mod ws_test_type_checking_toggle;
