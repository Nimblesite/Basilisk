//! Implements [WASM-DIAGNOSTIC]. See docs/specs/WASM-SPEC.md#WASM-DIAGNOSTIC
//!
//! The JSON the browser receives, field-for-field identical to the CLI's
//! `--output json` entries (`basilisk-cli/src/output/json.rs`). A consumer can
//! move between the two surfaces without a second parser.
//!
//! The CLI's own DTO is `pub(super)` inside a crate that cannot compile to
//! wasm, so it is redeclared here rather than imported. `tests::CLI_JSON_FIELDS`
//! asserts the two field lists still agree, so drift fails the build.

use basilisk_checker::{Diagnostic, Severity};
use basilisk_common::text::LineIndex;

/// One diagnostic, positioned in 1-based line/column coordinates.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WasmDiagnostic {
    /// The rule code (e.g. `BSK-0001`), or `None` for source that could not be
    /// analysed at all — no rule ran, so none can be named.
    pub code: Option<String>,
    /// `"error"`, `"warning"`, `"info"`, or `"safety violation"`.
    pub severity: String,
    /// Human-readable diagnostic message.
    pub message: String,
    /// The path the source was checked under ([WASM-API] `path`).
    pub path: String,
    /// 1-based line of the start of the span.
    pub line: usize,
    /// 1-based column of the start of the span.
    pub col: usize,
    /// 1-based line of the end of the span.
    pub end_line: usize,
    /// 1-based column of the end of the span (exclusive).
    pub end_col: usize,
}

/// The full result of one [`crate::check`] call.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Report {
    /// Every diagnostic the checker produced, in emission order.
    pub diagnostics: Vec<WasmDiagnostic>,
}

/// The wire spelling of a severity, matching the CLI's renderer exactly.
fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

impl Report {
    /// Position every diagnostic against `source` and collect them.
    ///
    /// One [`LineIndex`] serves the whole batch, so each span→line/col lookup is
    /// O(log n) rather than a rescan from the top of the file — and it is the
    /// same index the CLI renders with, so a span can never land one column
    /// apart between the two surfaces.
    #[must_use]
    pub fn new(diagnostics: &[Diagnostic], source: &str) -> Self {
        let index = LineIndex::new(source);
        Self {
            diagnostics: diagnostics
                .iter()
                .map(|diagnostic| {
                    let (line, col) = index.line_col(diagnostic.span.start_usize());
                    let (end_line, end_col) = index.line_col(diagnostic.span.end_usize());
                    WasmDiagnostic {
                        code: Some(diagnostic.code.code.to_owned()),
                        severity: severity_label(diagnostic.severity).to_owned(),
                        message: diagnostic.message.clone(),
                        path: diagnostic.path.clone(),
                        line,
                        col,
                        end_line,
                        end_col,
                    }
                })
                .collect(),
        }
    }

    /// A report for source that could not be analysed at all.
    ///
    /// The location is the start of the file: the failure is about the source as
    /// a whole, and the parser's own message carries whatever position it knows.
    /// This mirrors the CLI's `failure_entry`, so an unparseable program is data
    /// on the same channel as any other finding rather than a thrown exception
    /// ([WASM-PIPELINE]).
    #[must_use]
    pub fn from_failure(path: &str, message: &str) -> Self {
        Self {
            diagnostics: vec![WasmDiagnostic {
                code: None,
                severity: "error".to_owned(),
                message: message.to_owned(),
                path: path.to_owned(),
                line: 1,
                col: 1,
                end_line: 1,
                end_col: 1,
            }],
        }
    }
}
