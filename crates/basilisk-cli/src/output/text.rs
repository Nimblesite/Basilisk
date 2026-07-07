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
    use std::io::Write;

    // Precompute one line index per source; every diagnostic then converts its
    // span to line/col in O(log n) instead of rescanning the source prefix.
    let indexes = super::SourceIndexes::new(sources);
    // Buffer the whole render: stdout is line-buffered, so unbuffered printing
    // costs one write syscall per output line — thousands on error-dense files.
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let count = diagnostics
        .iter()
        .inspect(|d| {
            let _ = write!(out, "{}", format_one(d, indexes.for_path(&d.path)));
        })
        .filter(|d| d.severity == Severity::Error)
        .count();
    let _ = out.flush();
    count
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
///
/// Implements [CHKARCH-DIAGEXP-QUALITY]: emits the rustc-standard layout —
/// `severity[CODE]: message`, `--> path:line:col`, source snippet with caret
/// underline, then `= help:` / `= note:` / `= see:` annotation lines.
/// See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAGEXP-QUALITY
pub(super) fn format_one(
    diag: &Diagnostic,
    source: Option<(&str, &basilisk_common::text::LineIndex)>,
) -> String {
    let mut out = String::new();

    // Header: error[BSK-E0001]: Message
    let severity_label = color_severity(diag.severity, &format!("{}", diag.severity));
    let code = format!("[{}]", diag.code.code).bold();
    let message = diag.message.bold();
    let _ = writeln!(out, "{severity_label}{code}: {message}");

    // Location: --> path:line:col
    let location = source.map_or_else(
        || diag.path.clone(),
        |(_, index)| {
            let (line, col) = index.line_col(diag.span.start_usize());
            format!("{}:{}:{}", diag.path, line, col)
        },
    );

    let _ = writeln!(out, "  {} {location}", "-->".blue().bold());

    // Source snippet with underline
    if let Some((src, index)) = source {
        out.push_str(&format_snippet(
            src,
            index,
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
///
/// Production rendering builds a [`LineIndex`](basilisk_common::text::LineIndex)
/// once per file and calls its `line_col`; this single-shot wrapper remains only
/// for the focused line/col unit tests below.
#[cfg(test)]
pub(super) fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    basilisk_common::text::line_col(source, offset)
}

/// Format a source line with a `^^^^` underline for the highlighted span.
pub(super) fn format_snippet(
    source: &str,
    index: &basilisk_common::text::LineIndex,
    start: usize,
    end: usize,
    severity: Severity,
) -> String {
    let line_num = index.line(start);
    let line_start = index.line_start(start);
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
