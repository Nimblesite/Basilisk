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

#[path = "lsp/lsp_e2e_common.rs"]
mod lsp_e2e_common;

#[path = "lsp/lsp_e2e_advanced.rs"]
mod lsp_e2e_advanced;
#[path = "lsp/lsp_e2e_basics.rs"]
mod lsp_e2e_basics;
#[path = "lsp/lsp_e2e_change_signature.rs"]
mod lsp_e2e_change_signature;
#[path = "lsp/lsp_e2e_code_actions.rs"]
mod lsp_e2e_code_actions;
#[path = "lsp/lsp_e2e_completion.rs"]
mod lsp_e2e_completion;
#[path = "lsp/lsp_e2e_hierarchies.rs"]
mod lsp_e2e_hierarchies;
#[path = "lsp/lsp_e2e_hover.rs"]
mod lsp_e2e_hover;
#[path = "lsp/lsp_e2e_navigation.rs"]
mod lsp_e2e_navigation;
#[path = "lsp/lsp_e2e_refactoring.rs"]
mod lsp_e2e_refactoring;
#[path = "lsp/lsp_tests.rs"]
mod lsp_tests;
