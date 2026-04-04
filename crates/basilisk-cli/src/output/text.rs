//! Rustc-style text rendering for diagnostics with terminal colours.
//!
//! Example output (without ANSI codes):
//! ```text
//! error[BSK-E0001]: Missing parameter type annotation for `data`
//!   --> src/utils.py:14:5
//!    |
//! 14 | def process(data):
//!    |             ^^^^ parameter `data` has no type annotation
//!    |
//!    = help: Add a type annotation: `data: <type>`
//!    = note: In Basilisk, all function parameters require explicit types
//!    = see: https://www.basilisk-python.dev/errors/BSK-E0001
//! ```

use std::fmt::Write as _;

use basilisk_checker::{Diagnostic, Severity};
use colored::Colorize as _;

use super::FileSource;

/// Render all diagnostics to stdout in rustc style.
///
/// Returns the count of error-severity diagnostics.
pub fn render_diagnostics(diagnostics: &[Diagnostic], sources: &[FileSource]) -> usize {
    diagnostics
        .iter()
        .inspect(|d| {
            let source = sources.iter().find(|s| s.path == d.path);
            print!("{}", format_one(d, source.map(|s| s.text.as_str())));
        })
        .filter(|d| d.severity == Severity::Error)
        .count()
}

/// Apply the appropriate colour to a severity label.
fn color_severity(severity: Severity, text: &str) -> String {
    match severity {
        Severity::Error | Severity::SafetyViolation => text.red().bold().to_string(),
        Severity::Warning => text.yellow().bold().to_string(),
        Severity::Info => text.blue().bold().to_string(),
    }
}

/// Format a single diagnostic as a rustc-style string with ANSI colours.
pub(super) fn format_one(diag: &Diagnostic, source: Option<&str>) -> String {
    let mut out = String::new();

    // Header: error[BSK-E0001]: Message
    let severity_label = color_severity(diag.severity, &format!("{}", diag.severity));
    let code = format!("[{}]", diag.code.code).bold();
    let message = diag.message.bold();
    let _ = writeln!(out, "{severity_label}{code}: {message}");

    // Location: --> path:line:col
    let location = source.map_or_else(
        || diag.path.clone(),
        |src| {
            let (line, col) = byte_offset_to_line_col(src, diag.span.start_usize());
            format!("{}:{}:{}", diag.path, line, col)
        },
    );

    let _ = writeln!(out, "  {} {location}", "-->".blue().bold());

    // Source snippet with underline
    if let Some(src) = source {
        out.push_str(&format_snippet(
            src,
            diag.span.start_usize(),
            diag.span.end_usize(),
            diag.severity,
        ));
    }

    // Annotations
    if let Some(help) = &diag.help {
        let _ = writeln!(
            out,
            "   {} {}: {help}",
            "=".blue().bold(),
            "help".cyan().bold(),
        );
    }
    if let Some(note) = &diag.note {
        let _ = writeln!(
            out,
            "   {} {}: {note}",
            "=".blue().bold(),
            "note".cyan().bold(),
        );
    }
    let _ = writeln!(
        out,
        "   {} {}: {}",
        "=".blue().bold(),
        "see".cyan().bold(),
        diag.code.docs_url,
    );
    out.push('\n');
    out
}

/// Convert a byte offset into (1-based line number, 1-based column number).
pub(super) fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let clamped = offset.min(source.len());
    let before = &source[..clamped];
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = before.rfind('\n').map_or(clamped, |pos| clamped - pos - 1) + 1;
    (line, col)
}

/// Format a source line with a `^^^^` underline for the highlighted span.
pub(super) fn format_snippet(source: &str, start: usize, end: usize, severity: Severity) -> String {
    let (line_num, _) = byte_offset_to_line_col(source, start);
    let line_start = source[..start].rfind('\n').map_or(0, |p| p + 1);
    let line_text = source[line_start..].lines().next().unwrap_or("");

    let col_start = start - line_start;
    let col_end = (end - line_start).min(line_text.len());
    let underline_len = col_end.saturating_sub(col_start).max(1);

    let line_num_width = line_num.to_string().len();
    let pad = " ".repeat(line_num_width);
    let pipe = "|".blue().bold();
    let line_num_str = line_num.to_string().blue().bold();
    let underline = color_severity(severity, &"^".repeat(underline_len));

    let mut out = String::new();
    let _ = writeln!(out, "{pad}   {pipe}");
    let _ = writeln!(out, "{line_num_str} {pipe} {line_text}");
    let _ = writeln!(
        out,
        "{pad}   {pipe} {spaces}{underline}",
        spaces = " ".repeat(col_start),
    );
    let _ = writeln!(out, "{pad}   {pipe}");
    out
}
