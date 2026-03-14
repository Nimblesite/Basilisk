//! Machine-readable JSON output for diagnostics.
//!
//! JSON output is a flat array consumed by the VS Code extension:
//! ```json
//! [
//!   {
//!     "code": "BSK-E0001",
//!     "severity": "error",
//!     "message": "Missing parameter type annotation for `x`",
//!     "path": "src/utils.py",
//!     "line": 1,
//!     "col": 9,
//!     "end_line": 1,
//!     "end_col": 10
//!   }
//! ]
//! ```

use serde::Serialize;

use basilisk_checker::Diagnostic;

use super::{text::byte_offset_to_line_col, FileSource};

/// Serialisable form of a single diagnostic for JSON output.
#[derive(Serialize)]
pub(super) struct JsonDiagnostic<'a> {
    /// The diagnostic error/warning code (e.g. `BSK-E0001`).
    pub(super) code: &'a str,
    /// Severity string: `"error"`, `"warning"`, `"info"`, or `"safety violation"`.
    pub(super) severity: &'a str,
    /// Human-readable diagnostic message.
    pub(super) message: &'a str,
    /// Path to the file containing the diagnostic.
    pub(super) path: &'a str,
    /// 1-based line number of the start of the span.
    pub(super) line: usize,
    /// 1-based column number of the start of the span.
    pub(super) col: usize,
    /// 1-based line number of the end of the span.
    pub(super) end_line: usize,
    /// 1-based column number of the end of the span (exclusive).
    pub(super) end_col: usize,
}

/// Render all diagnostics as a JSON array to stdout.
pub fn render_diagnostics_json(diagnostics: &[Diagnostic], sources: &[FileSource]) {
    let items: Vec<JsonDiagnostic<'_>> = diagnostics
        .iter()
        .map(|d| {
            let source = sources
                .iter()
                .find(|s| s.path == d.path)
                .map(|s| s.text.as_str());
            let (line, col) = source.map_or((1, 1), |src| {
                byte_offset_to_line_col(src, d.span.start_usize())
            });
            let (end_line, end_col) = source.map_or((line, col + 1), |src| {
                byte_offset_to_line_col(src, d.span.end_usize())
            });
            JsonDiagnostic {
                code: d.code.code,
                severity: match d.severity {
                    basilisk_checker::Severity::Error => "error",
                    basilisk_checker::Severity::Warning => "warning",
                    basilisk_checker::Severity::Info => "info",
                    basilisk_checker::Severity::SafetyViolation => "safety violation",
                },
                message: &d.message,
                path: &d.path,
                line,
                col,
                end_line,
                end_col,
            }
        })
        .collect();

    match serde_json::to_string_pretty(&items) {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("basilisk: failed to serialize diagnostics: {e}"),
    }
}
