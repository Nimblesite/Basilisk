//! Diagnostic output rendering — rustc-style text and machine-readable JSON.
//!
//! Text example:
//! ```text
//! error[BSK-E0001]: Missing parameter type annotation for `data`
//!   --> src/utils.py:14:5
//!    |
//! 14 | def process(data):
//!    |             ^^^^ parameter `data` has no type annotation
//!    |
//!    = help: Add a type annotation: `data: <type>`
//!    = note: In Basilisk, all function parameters require explicit types
//!    = see: https://basilisk-lang.org/errors/BSK-E0001
//! ```
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

use clap::ValueEnum;
use serde::Serialize;

use basilisk_checker::Diagnostic;

/// Output format for the `check` subcommand.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable rustc-style text (default).
    Text,
    /// Machine-readable JSON array consumed by the VS Code extension.
    Json,
}

/// Associates a file path with its source text for span-to-line-col mapping.
pub struct FileSource {
    /// The file path.
    pub path: String,
    /// The full source text.
    pub text: String,
}

/// Render all diagnostics to stdout in rustc style.
///
/// Returns the count of error-severity diagnostics.
pub fn render_diagnostics(diagnostics: &[Diagnostic], sources: &[FileSource]) -> usize {
    diagnostics
        .iter()
        .inspect(|d| {
            let source = sources.iter().find(|s| s.path == d.path);
            render_one(d, source.map(|s| s.text.as_str()));
        })
        .filter(|d| d.severity == basilisk_checker::Severity::Error)
        .count()
}

/// Serialisable form of a single diagnostic for JSON output.
#[derive(Serialize)]
struct JsonDiagnostic<'a> {
    code: &'a str,
    severity: &'a str,
    message: &'a str,
    path: &'a str,
    /// 1-based line number of the start of the span.
    line: usize,
    /// 1-based column number of the start of the span.
    col: usize,
    /// 1-based line number of the end of the span.
    end_line: usize,
    /// 1-based column number of the end of the span (exclusive).
    end_col: usize,
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
                byte_offset_to_line_col(src, d.span.start as usize)
            });
            let (end_line, end_col) = source.map_or((line, col + 1), |src| {
                byte_offset_to_line_col(src, d.span.end as usize)
            });
            JsonDiagnostic {
                code: d.code.code,
                severity: match d.severity {
                    basilisk_checker::Severity::Error => "error",
                    basilisk_checker::Severity::Warning => "warning",
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

fn render_one(diag: &Diagnostic, source: Option<&str>) {
    // Header: error[BSK-E0001]: Message
    println!("{}[{}]: {}", diag.severity, diag.code.code, diag.message);

    // Location: --> path:line:col
    let location = source.map_or_else(
        || diag.path.clone(),
        |src| {
            let (line, col) = byte_offset_to_line_col(src, diag.span.start as usize);
            format!("{}:{}:{}", diag.path, line, col)
        },
    );

    println!("  --> {location}");

    // Source snippet with underline
    if let Some(src) = source {
        render_snippet(src, diag.span.start as usize, diag.span.end as usize);
    }

    // Annotations
    if let Some(help) = &diag.help {
        println!("   = help: {help}");
    }
    if let Some(note) = &diag.note {
        println!("   = note: {note}");
    }
    println!("   = see: {}", diag.code.docs_url);
    println!();
}

/// Convert a byte offset into (1-based line number, 1-based column number).
fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let clamped = offset.min(source.len());
    let before = &source[..clamped];
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = before.rfind('\n').map_or(clamped, |pos| clamped - pos - 1) + 1;
    (line, col)
}

/// Render a source line with a `^^^^` underline for the highlighted span.
fn render_snippet(source: &str, start: usize, end: usize) {
    let (line_num, _) = byte_offset_to_line_col(source, start);
    let line_start = source[..start].rfind('\n').map_or(0, |p| p + 1);
    let line_text = source[line_start..].lines().next().unwrap_or("");

    let col_start = start - line_start;
    let col_end = (end - line_start).min(line_text.len());
    let underline_len = col_end.saturating_sub(col_start).max(1);

    let line_num_width = line_num.to_string().len();
    let pad = " ".repeat(line_num_width);

    println!("{pad}   |");
    println!("{line_num} | {line_text}");
    println!(
        "{pad}   | {spaces}{underline}",
        spaces = " ".repeat(col_start),
        underline = "^".repeat(underline_len),
    );
    println!("{pad}   |");
}

#[cfg(test)]
mod tests {
    use super::*;
    use basilisk_checker::{ErrorCode, Severity};
    use basilisk_resolver::Span;

    fn make_diag(help: Option<&str>, note: Option<&str>) -> Diagnostic {
        Diagnostic {
            code: ErrorCode {
                code: "BSK-E0001",
                docs_url: "https://basilisk-lang.org/errors/BSK-E0001",
            },
            severity: Severity::Error,
            message: "missing annotation for `x`".to_owned(),
            span: Span { start: 8, end: 9 },
            path: "test.py".to_owned(),
            help: help.map(str::to_owned),
            note: note.map(str::to_owned),
        }
    }

    #[test]
    fn render_diagnostics_counts_only_errors() {
        let diag = make_diag(Some("add a type"), Some("all params need types"));
        let sources = vec![FileSource {
            path: "test.py".to_owned(),
            text: "def foo(x): pass".to_owned(),
        }];
        let count = render_diagnostics(&[diag], &sources);
        assert_eq!(count, 1);
    }

    #[test]
    fn render_one_without_source_falls_back_to_path() {
        let diag = make_diag(Some("help"), Some("note"));
        render_one(&diag, None);
    }

    #[test]
    fn render_one_without_help() {
        let diag = make_diag(None, Some("note text"));
        render_one(&diag, Some("def foo(x): pass"));
    }

    #[test]
    fn render_one_without_note() {
        let diag = make_diag(Some("help text"), None);
        render_one(&diag, Some("def foo(x): pass"));
    }

    #[test]
    fn render_one_without_help_or_note() {
        let diag = make_diag(None, None);
        render_one(&diag, Some("def foo(x): pass"));
    }

    #[test]
    fn byte_offset_to_line_col_second_line() {
        let source = "def foo(): pass\ndef bar(x): pass";
        // byte 16 is the 'd' starting "def bar"
        let (line, col) = byte_offset_to_line_col(source, 16);
        assert_eq!(line, 2);
        assert_eq!(col, 1);
    }

    #[test]
    fn render_snippet_on_second_line() {
        let source = "def foo(): pass\ndef bar(x): pass";
        render_snippet(source, 20, 23);
    }

    // ── JSON output ───────────────────────────────────────────────────────────

    #[test]
    fn json_produces_valid_array_with_correct_fields() -> Result<(), Box<dyn std::error::Error>> {
        let source = "def foo(x): pass";
        // byte 8 = 'x', so line=1, col=9 (1-based)
        let (line, col) = byte_offset_to_line_col(source, 8);
        let (end_line, end_col) = byte_offset_to_line_col(source, 9);
        let item = JsonDiagnostic {
            code: "BSK-E0001",
            severity: "error",
            message: "missing annotation for `x`",
            path: "test.py",
            line,
            col,
            end_line,
            end_col,
        };
        let json = serde_json::to_string(&item)?;
        assert!(json.contains("BSK-E0001"));
        assert!(json.contains("\"line\":1"));
        assert!(json.contains("\"col\":9"));
        assert!(json.contains("\"end_line\":1"));
        assert!(json.contains("\"end_col\":10"));
        Ok(())
    }

    #[test]
    fn json_empty_diagnostics_produces_empty_array() -> Result<(), Box<dyn std::error::Error>> {
        let items: Vec<JsonDiagnostic<'_>> = vec![];
        let json = serde_json::to_string(&items)?;
        assert_eq!(json, "[]");
        Ok(())
    }

    #[test]
    fn render_diagnostics_json_smoke_test() {
        let diag = make_diag(None, None);
        let sources = vec![FileSource {
            path: "test.py".to_owned(),
            text: "def foo(x): pass".to_owned(),
        }];
        // Just verify it doesn't panic.
        render_diagnostics_json(&[diag], &sources);
    }

    #[test]
    fn render_diagnostics_json_empty_is_safe() {
        render_diagnostics_json(&[], &[]);
    }

    #[test]
    fn json_severity_warning() -> Result<(), Box<dyn std::error::Error>> {
        let source = "def foo(x): pass";
        let (line, col) = byte_offset_to_line_col(source, 8);
        let (end_line, end_col) = byte_offset_to_line_col(source, 9);
        let item = JsonDiagnostic {
            code: "BSK-W0001",
            severity: "warning",
            message: "test warning",
            path: "test.py",
            line,
            col,
            end_line,
            end_col,
        };
        let json = serde_json::to_string(&item)?;
        assert!(json.contains("\"warning\""));
        Ok(())
    }
}
