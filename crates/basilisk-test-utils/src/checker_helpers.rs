//! Helpers that depend on `basilisk-checker`. Gated behind the `checker` feature.

use basilisk_checker::{Diagnostic, Severity};

use crate::line_col;

/// A concise expected-diagnostic value constructed in tests.
#[derive(Debug)]
pub struct Expected {
    /// Diagnostic code (e.g. `"BSK-E0001"`).
    pub code: &'static str,
    /// Expected severity.
    pub severity: Severity,
    /// Substring that must appear in the message (usually the symbol name).
    pub message_contains: &'static str,
    /// Expected 1-based line.
    pub line: usize,
    /// Expected 1-based column.
    pub col: usize,
}

impl Expected {
    /// Shorthand for an error diagnostic.
    #[must_use]
    pub fn error(
        code: &'static str,
        message_contains: &'static str,
        line: usize,
        col: usize,
    ) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message_contains,
            line,
            col,
        }
    }

    /// Shorthand for a warning diagnostic.
    #[must_use]
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
///
/// # Panics
///
/// Panics if the number of diagnostics differs from expected, or if any
/// diagnostic has a mismatched code, severity, line, column, or message.
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

    for (idx, (got, want)) in sorted.iter().zip(expected.iter()).enumerate() {
        let (got_line, got_col) = line_col(source, got.span.start);

        assert_eq!(
            got.code.code, want.code,
            "diagnostic[{idx}]: wrong code (got {}, want {})",
            got.code.code, want.code
        );
        assert_eq!(
            got.severity, want.severity,
            "diagnostic[{idx}]: wrong severity"
        );
        assert_eq!(
            got_line, want.line,
            "diagnostic[{idx}]: wrong line (got {got_line}, want {})",
            want.line
        );
        assert_eq!(
            got_col, want.col,
            "diagnostic[{idx}]: wrong column (got {got_col}, want {})",
            want.col
        );
        assert!(
            got.message.contains(want.message_contains),
            "diagnostic[{idx}]: message {:#?} does not contain {:#?}",
            got.message,
            want.message_contains,
        );
    }
}
