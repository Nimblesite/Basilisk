//! Profiler command constants and shared utilities.
//!
//! The actual LSP command handlers live in `server::profiler_handlers` following
//! the codebase convention (see `server::uv_handlers`, `server::test_handlers`).
//! This module exists for the `mod.rs` re-export and any shared command logic.

/// Parse the export format string from a client request.
///
/// Returns `None` for `"summary"` (text-only, no file export).
#[must_use]
pub fn parse_export_format(format_str: &str) -> Option<super::export::ExportFormat> {
    match format_str {
        "flamegraph" => Some(super::export::ExportFormat::Flamegraph),
        "summary" => None,
        _ => Some(super::export::ExportFormat::Speedscope),
    }
}
