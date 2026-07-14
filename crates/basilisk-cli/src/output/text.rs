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
    let colorize = colored::control::SHOULD_COLORIZE.should_colorize();
    let count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count();

    let mut stdout = std::io::stdout().lock();
    if colorize {
        // Keep terminal output incremental so a large project starts showing
        // useful diagnostics immediately.
        let mut out = std::io::BufWriter::new(stdout);
        for diagnostic in diagnostics {
            let rendered = format_one(diagnostic, indexes.for_path(&diagnostic.path));
            let _ = out.write_all(rendered.as_bytes());
        }
        let _ = out.flush();
    } else {
        // Pipes, CI, editors, and benchmark runs are the overwhelmingly common
        // high-volume path. Build their plain render once and write it in one
        // operation instead of feeding the 8 KiB BufWriter once per diagnostic.
        // Cap only the initial reservation; String can still grow for genuinely
        // large output without an attacker-controlled eager allocation.
        let initial_capacity = diagnostics.len().saturating_mul(384).min(8 * 1024 * 1024);
        let mut rendered = String::with_capacity(initial_capacity);
        for diagnostic in diagnostics {
            format_one_plain_into(
                &mut rendered,
                diagnostic,
                indexes.for_path(&diagnostic.path),
            );
        }
        let _ = stdout.write_all(rendered.as_bytes());
        let _ = stdout.flush();
    }

    count
}

/// Format directly into a reusable buffer when ANSI colour is disabled.
///
/// CLI output is normally piped in editor, CI, and benchmark use. Avoiding the
/// temporary coloured strings and per-diagnostic output allocation keeps that
/// common path proportional to bytes written, even for error-dense files.
fn format_one_plain_into(
    out: &mut String,
    diag: &Diagnostic,
    source: Option<(&str, &basilisk_common::text::LineIndex)>,
) {
    let _ = writeln!(
        out,
        "{}[{}]: {}",
        diag.severity, diag.code.code, diag.message
    );

    if let Some((_, index)) = source {
        let (line, col) = index.line_col(diag.span.start_usize());
        let _ = writeln!(out, "  --> {}:{line}:{col}", diag.path);
    } else {
        let _ = writeln!(out, "  --> {}", diag.path);
    }

    if let Some((src, index)) = source {
        format_snippet_plain_into(
            out,
            src,
            index,
            diag.span.start_usize(),
            diag.span.end_usize(),
        );
    }

    if let Some(help) = &diag.help {
        let _ = writeln!(out, "   = help: {help}");
    }
    if let Some(note) = &diag.note {
        let _ = writeln!(out, "   = note: {note}");
    }
    let _ = writeln!(out, "   = see: {}\n", diag.code.docs_url);
}

fn format_snippet_plain_into(
    out: &mut String,
    source: &str,
    index: &basilisk_common::text::LineIndex,
    start: usize,
    end: usize,
) {
    let line_num = index.line(start);
    let line_start = index.line_start(start);
    let line_text = source[line_start..].lines().next().unwrap_or("");
    let col_start = start - line_start;
    let col_end = (end - line_start).min(line_text.len());
    let underline_len = col_end.saturating_sub(col_start).max(1);
    let line_num_width = decimal_width(line_num);

    push_repeated(out, ' ', line_num_width);
    out.push_str(" |\n");
    let _ = writeln!(out, "{line_num} | {line_text}");
    push_repeated(out, ' ', line_num_width);
    out.push_str(" | ");
    push_repeated(out, ' ', col_start);
    push_repeated(out, '^', underline_len);
    out.push('\n');
    push_repeated(out, ' ', line_num_width);
    out.push_str(" |\n");
}

fn push_repeated(out: &mut String, character: char, count: usize) {
    out.extend(std::iter::repeat_n(character, count));
}

fn decimal_width(mut value: usize) -> usize {
    let mut width = 1;
    while value >= 10 {
        value /= 10;
        width += 1;
    }
    width
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
    let _ = writeln!(out, "{pad} {pipe}");
    let _ = writeln!(out, "{line_num_str} {pipe} {line_text}");
    let _ = writeln!(
        out,
        "{pad} {pipe} {spaces}{underline}",
        spaces = " ".repeat(col_start),
    );
    let _ = writeln!(out, "{pad} {pipe}");
    out
}
