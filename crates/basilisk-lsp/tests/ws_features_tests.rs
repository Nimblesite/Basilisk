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

#[path = "common/mod.rs"]
mod common;

#[path = "lsp/ws_test_code_actions.rs"]
mod ws_test_code_actions;
#[path = "lsp/ws_test_code_lens.rs"]
mod ws_test_code_lens;
#[path = "lsp/ws_test_completion.rs"]
mod ws_test_completion;
#[path = "lsp/ws_test_completion_advanced.rs"]
mod ws_test_completion_advanced;
#[path = "lsp/ws_test_completion_generics.rs"]
mod ws_test_completion_generics;
#[path = "lsp/ws_test_configuration_adoption.rs"]
mod ws_test_configuration_adoption;
#[path = "lsp/ws_test_configuration_editor.rs"]
mod ws_test_configuration_editor;
#[path = "lsp/ws_test_configuration_preview.rs"]
mod ws_test_configuration_preview;
#[path = "lsp/ws_test_configuration_watch.rs"]
mod ws_test_configuration_watch;
#[path = "lsp/ws_test_editor_journey.rs"]
mod ws_test_editor_journey;
#[path = "lsp/ws_test_execute_command.rs"]
mod ws_test_execute_command;
#[path = "lsp/ws_test_execute_stubs.rs"]
mod ws_test_execute_stubs;
#[path = "lsp/ws_test_execute_uv.rs"]
mod ws_test_execute_uv;
#[path = "lsp/ws_test_hover.rs"]
mod ws_test_hover;
#[path = "lsp/ws_test_inlay_hints.rs"]
mod ws_test_inlay_hints;
#[path = "lsp/ws_test_inlay_hints_display.rs"]
mod ws_test_inlay_hints_display;
#[path = "lsp/ws_test_inlay_hints_recovery.rs"]
mod ws_test_inlay_hints_recovery;
#[path = "lsp/ws_test_memory.rs"]
mod ws_test_memory;
#[path = "lsp/ws_test_processes.rs"]
mod ws_test_processes;
#[path = "lsp/ws_test_refactoring.rs"]
mod ws_test_refactoring;
#[path = "lsp/ws_test_semantic_tokens.rs"]
mod ws_test_semantic_tokens;
#[path = "lsp/ws_test_signature_help.rs"]
mod ws_test_signature_help;
#[path = "lsp/ws_test_typeshed_no_source.rs"]
mod ws_test_typeshed_no_source;
