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

#[path = "lsp/zed_e2e_common.rs"]
mod zed_e2e_common;

#[path = "lsp/zed_extension_e2e_advanced.rs"]
mod zed_extension_e2e_advanced;
#[path = "lsp/zed_extension_e2e_tests.rs"]
mod zed_extension_e2e_tests;
