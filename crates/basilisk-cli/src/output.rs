//! Rustc-style diagnostic output rendering.
//!
//! Example output:
//!
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

use basilisk_checker::Diagnostic;

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

/// Associates a file path with its source text for span-to-line-col mapping.
pub struct FileSource {
    /// The file path.
    pub path: String,
    /// The full source text.
    pub text: String,
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
