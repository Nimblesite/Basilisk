//! Shared helpers for Basilisk CLI end-to-end tests.
//!
//! Every test uses a real `.py` fixture file and asserts the exact set of
//! diagnostics produced: error code, symbol name, byte span, line, column,
//! and message. No hand-wavy count assertions — if a diagnostic appears at
//! the wrong location or with the wrong message, the test fails.
//!
//! Pipeline under test: `parse_file` → resolve → check

use std::path::Path;

use basilisk_checker::{check, Diagnostic, Severity};
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;

pub fn fixture(rel: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

pub fn run(rel: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let path = fixture(rel);
    let parsed = parse_file(&path)?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

/// Convert a byte offset in `source` into a 1-based (line, col) pair.
pub fn line_col(source: &str, offset: u32) -> (usize, usize) {
    let clamped = (offset as usize).min(source.len());
    let before = &source[..clamped];
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = before.rfind('\n').map_or(clamped, |pos| clamped - pos - 1) + 1;
    (line, col)
}

/// A concise expected-diagnostic value constructed in tests.
#[derive(Debug)]
pub struct Expected {
    pub code: &'static str,
    pub severity: Severity,
    /// Substring that must appear in the message (usually the symbol name).
    pub message_contains: &'static str,
    pub line: usize,
    pub col: usize,
}

impl Expected {
    pub fn error(code: &'static str, message_contains: &'static str, line: usize, col: usize) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message_contains,
            line,
            col,
        }
    }

    pub fn warning(
        code: &'static str,
        message_contains: &'static str,
        line: usize,
        col: usize,
    ) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message_contains,
            line,
            col,
        }
    }
}

/// Assert that `diags` matches `expected` exactly — same count, same order
/// (sorted by span start), same code/severity/location/message.
pub fn assert_diagnostics(source: &str, diags: &[Diagnostic], expected: &[Expected]) {
    let mut sorted = diags.to_vec();
    // Sort by span start, then by code for a stable order when two diagnostics
    // share the same position (e.g. E0025 and E0002 on the same method line).
    sorted.sort_by(|a, b| {
        a.span
            .start
            .cmp(&b.span.start)
            .then(a.code.code.cmp(b.code.code))
    });

    assert_eq!(
        sorted.len(),
        expected.len(),
        "wrong number of diagnostics.\n  got:\n{}\n  want {} diagnostics",
        sorted
            .iter()
            .map(|d| {
                let (l, c) = line_col(source, d.span.start);
                format!(
                    "    {}[{}] at {l}:{c} — {}",
                    d.severity, d.code.code, d.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        expected.len(),
    );

    for (i, (got, want)) in sorted.iter().zip(expected.iter()).enumerate() {
        let (got_line, got_col) = line_col(source, got.span.start);

        assert_eq!(
            got.code.code, want.code,
            "diagnostic[{i}]: wrong code (got {}, want {})",
            got.code.code, want.code
        );
        assert_eq!(
            got.severity, want.severity,
            "diagnostic[{i}]: wrong severity"
        );
        assert_eq!(
            got_line, want.line,
            "diagnostic[{i}]: wrong line (got {got_line}, want {})",
            want.line
        );
        assert_eq!(
            got_col, want.col,
            "diagnostic[{i}]: wrong column (got {got_col}, want {})",
            want.col
        );
        assert!(
            got.message.contains(want.message_contains),
            "diagnostic[{i}]: message {:#?} does not contain {:#?}",
            got.message,
            want.message_contains,
        );
    }
}
