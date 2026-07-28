//! Implements [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
//! Machine-readable JSON output for diagnostics.
//!
//! JSON output is a flat array consumed by the VS Code extension:
//! ```json
//! [
//!   {
//!     "code": "BSK-0001",
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
//!
//! A file the run could not read at all is reported in the same array with a
//! `null` code, because no rule produced it. Leaving it out rendered `[]` — the
//! answer a clean file gets — for a file that was never checked, so every
//! consumer that reads the report rather than the exit status was told a file
//! with a syntax error had no problems.

use serde::Serialize;

use basilisk_checker::Diagnostic;

use super::FileSource;

/// Serialisable form of a single diagnostic for JSON output.
#[derive(Serialize)]
pub(super) struct JsonDiagnostic<'a> {
    /// The diagnostic error/warning code (e.g. `BSK-0001`), or `None` for a
    /// file the run could not analyse — no rule ran, so none can be named.
    pub(super) code: Option<&'a str>,
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

/// A file the run could not analyse at all, rendered alongside the diagnostics.
pub struct JsonFailure<'a> {
    /// Path of the file that could not be analysed.
    pub path: &'a str,
    /// Why it could not be analysed, as the parser or reader reported it.
    pub message: &'a str,
}

/// Render every diagnostic, and every file that failed outright, to stdout.
pub fn render_diagnostics_json(
    diagnostics: &[Diagnostic],
    sources: &[FileSource],
    failures: &[JsonFailure<'_>],
) {
    // One line index per source, reused for every diagnostic in that file — the
    // span→line/col conversions become O(log n) instead of prefix rescans.
    let indexes = super::SourceIndexes::new(sources);
    let items: Vec<JsonDiagnostic<'_>> = diagnostics
        .iter()
        .map(|d| {
            let index = indexes.for_path(&d.path).map(|(_, index)| index);
            let (line, col) = index.map_or((1, 1), |index| index.line_col(d.span.start_usize()));
            let (end_line, end_col) =
                index.map_or((line, col + 1), |index| index.line_col(d.span.end_usize()));
            JsonDiagnostic {
                code: Some(d.code.code),
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
        .chain(failures.iter().map(failure_entry))
        .collect();

    match serde_json::to_string_pretty(&items) {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("basilisk: failed to serialize diagnostics: {e}"),
    }
}

/// One unanalysable file as a JSON entry.
///
/// The location is the start of the file: the failure is about the file as a
/// whole, and the parser's own message carries whatever position it knows.
pub(super) fn failure_entry<'a>(failure: &'a JsonFailure<'a>) -> JsonDiagnostic<'a> {
    JsonDiagnostic {
        code: None,
        severity: "error",
        message: failure.message,
        path: failure.path,
        line: 1,
        col: 1,
        end_line: 1,
        end_col: 1,
    }
}
