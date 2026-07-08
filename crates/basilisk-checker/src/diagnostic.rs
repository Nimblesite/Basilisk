//! Implements [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Diagnostic data types for Basilisk.

use basilisk_resolver::Span;
use basilisk_stubs::TypeProvenance;

/// The severity level of a diagnostic.
///
/// Implements [CHKARCH-STRICTNESS-SEVERITY] / [CHKARCH-DIAG-CODES]: every rule
/// has a default severity determined by its code prefix (`E` = Error,
/// `W` = Warning, `I` = Info). All rules can be overridden to any severity at
/// the line, block, file, path, or project level (the override application lives
/// in `basilisk-checker/src/lib.rs`, the precedence ladder in
/// [CHKARCH-STRICTNESS-PRECEDENCE]).
/// See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-SEVERITY
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Severity {
    /// Informational hint — does not block CI, shown as blue in LSP.
    Info,
    /// A stylistic suggestion — does not block CI by default.
    Warning,
    /// A type error that must be resolved.
    Error,
    /// Critical safety violation reported by the opt-in, off-by-default
    /// ownership/move-semantics rules inspired by Mojo.
    SafetyViolation,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
            Severity::SafetyViolation => write!(f, "safety violation"),
        }
    }
}

/// The mode a rule can be set to via inline comments or configuration.
///
/// This controls what happens to a diagnostic after it is emitted by a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleMode {
    /// Rule fires at its default severity.
    Error,
    /// Rule fires but is demoted to a warning.
    Warning,
    /// Rule fires but is demoted to an informational hint.
    Info,
    /// Rule is completely disabled — not checked at all.
    Disabled,
    /// Suppress the diagnostic entirely (like `# type: ignore`).
    Ignore,
}

/// A parsed inline suppression or mode override from a source comment.
///
/// The data shape behind [CHKARCH-STRICTNESS-SUPPRESSION] / the compat table in
/// [CHKARCH-STRICTNESS-COMPAT] — a `# type: <mode>[codes]` or block/file
/// directive. The comment scanner that produces these and the precedence ladder
/// that applies them live in `basilisk-checker` (out of this audit's scope).
/// See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-SUPPRESSION
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineOverride {
    /// The mode to apply.
    pub mode: RuleMode,
    /// Specific rule codes this applies to, or empty for all rules.
    pub codes: Vec<String>,
    /// Whether this is a block-start directive.
    pub is_block_start: bool,
    /// Whether this is a block-end directive.
    pub is_block_end: bool,
}

/// A BSK diagnostic code such as `returns_compatibility`.
///
/// Implements [CHKARCH-DIAG-PHILOSOPHY] (stable codes, every diagnostic links to
/// docs) and underpins the [CHKARCH-DIAG-REFERENCE] table generated from rule
/// source. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-PHILOSOPHY
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorCode {
    /// The full code string, e.g. `"returns_compatibility"`.
    pub code: &'static str,
    /// URL to the documentation for this diagnostic.
    pub docs_url: &'static str,
}

/// Build an `Error`-severity diagnostic with optional static `help`/`note` text.
///
/// Used by per-rule `make_diag` helpers whose only per-call inputs are the
/// message, span and path, with `code`/`help`/`note` fixed per rule.
pub(crate) fn error_diagnostic(
    code: ErrorCode,
    message: String,
    span: Span,
    path: &str,
    help: Option<&'static str>,
    note: Option<&'static str>,
) -> Diagnostic {
    error_diagnostic_owned(
        code,
        message,
        span,
        path,
        help.map(str::to_owned),
        note.map(str::to_owned),
    )
}

/// Build an `Error`-severity diagnostic with `String`-owned `help`/`note` text.
///
/// Used by call sites whose `help`/`note` are produced via `format!(...)`.
pub(crate) fn error_diagnostic_owned(
    code: ErrorCode,
    message: String,
    span: Span,
    path: &str,
    help: Option<String>,
    note: Option<String>,
) -> Diagnostic {
    diagnostic_owned(code, Severity::Error, message, span, path, help, note)
}

/// Convenience: build an `Error` diagnostic with a `format!`-produced `help`
/// and a static `&'static str` note. Many rules pair a dynamic help string
/// with an immutable PEP/spec reference note; this collapses that pattern.
pub(crate) fn error_diag_help_note(
    code: ErrorCode,
    message: String,
    span: Span,
    path: &str,
    help: String,
    note: &'static str,
) -> Diagnostic {
    error_diagnostic_owned(code, message, span, path, Some(help), Some(note.to_owned()))
}

/// Build a `Warning`-severity diagnostic with `String`-owned `help`/`note` text.
pub(crate) fn warning_diagnostic_owned(
    code: ErrorCode,
    message: String,
    span: Span,
    path: &str,
    help: Option<String>,
    note: Option<String>,
) -> Diagnostic {
    diagnostic_owned(code, Severity::Warning, message, span, path, help, note)
}

/// Build an `Info`-severity diagnostic with `String`-owned `help`/`note` text.
pub(crate) fn info_diagnostic_owned(
    code: ErrorCode,
    message: String,
    span: Span,
    path: &str,
    help: Option<String>,
    note: Option<String>,
) -> Diagnostic {
    diagnostic_owned(code, Severity::Info, message, span, path, help, note)
}

fn diagnostic_owned(
    code: ErrorCode,
    severity: Severity,
    message: String,
    span: Span,
    path: &str,
    help: Option<String>,
    note: Option<String>,
) -> Diagnostic {
    Diagnostic {
        code,
        severity,
        message,
        span,
        path: path.to_owned(),
        help,
        note,
        provenance: None,
    }
}

/// A single diagnostic emitted by the checker.
///
/// `PartialEq` lets consumers detect "diagnostics unchanged" — the LSP's
/// workspace sweep compares a file's fresh diagnostics against the stored ones
/// and republishes only on a real change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The error/warning code.
    pub code: ErrorCode,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable primary message.
    pub message: String,
    /// The span being highlighted in the source.
    pub span: Span,
    /// The source file path.
    pub path: String,
    /// Optional help text shown after the snippet.
    pub help: Option<String>,
    /// Optional note shown after the snippet.
    pub note: Option<String>,
    /// Where the type information came from, if this diagnostic relates to
    /// an imported symbol.  Used for tier-based severity adjustment (Tier3
    /// diagnostics are downgraded to Info) and hover annotations.
    pub provenance: Option<TypeProvenance>,
}

impl Diagnostic {
    /// Render the `help`/`note` lines as a plain-text suffix to append to a
    /// primary message.
    ///
    /// The terminal renderer prints `help`/`note` as their own coloured lines,
    /// but the LSP `Diagnostic` has no equivalent fields — so editors would
    /// otherwise drop this guidance entirely. Folding it onto the message keeps
    /// the explanation (and any documentation links it carries) visible in the
    /// hover and Problems panel. Returns an empty string when neither is present
    /// so callers that concatenate leave a help/note-free message untouched.
    #[must_use]
    pub fn help_note_suffix(&self) -> String {
        let mut suffix = String::new();
        if let Some(help) = &self.help {
            suffix.push_str("\n\nhelp: ");
            suffix.push_str(help);
        }
        if let Some(note) = &self.note {
            suffix.push_str(if suffix.is_empty() {
                "\n\nnote: "
            } else {
                "\nnote: "
            });
            suffix.push_str(note);
        }
        suffix
    }
}

// Tests for [CHKARCH-DIAGEXP-QUALITY]: the help/note guidance every diagnostic
// carries (folded onto the LSP message via `help_note_suffix`).
#[cfg(test)]
mod tests {
    use super::{Diagnostic, ErrorCode, Severity};
    use basilisk_resolver::Span;

    fn diag(help: Option<&str>, note: Option<&str>) -> Diagnostic {
        Diagnostic {
            code: ErrorCode {
                code: "BSK-E0000",
                docs_url: "https://www.basilisk-python.dev",
            },
            severity: Severity::Error,
            message: "msg".to_owned(),
            span: Span::new(0, 1),
            path: "t.py".to_owned(),
            help: help.map(str::to_owned),
            note: note.map(str::to_owned),
            provenance: None,
        }
    }

    #[test]
    fn suffix_empty_when_neither_present() {
        assert_eq!(diag(None, None).help_note_suffix(), "");
    }

    #[test]
    fn suffix_help_only() {
        assert_eq!(diag(Some("H"), None).help_note_suffix(), "\n\nhelp: H");
    }

    #[test]
    fn suffix_note_only_starts_with_blank_line() {
        assert_eq!(diag(None, Some("N")).help_note_suffix(), "\n\nnote: N");
    }

    #[test]
    fn suffix_help_and_note_share_one_blank_line() {
        assert_eq!(
            diag(Some("H"), Some("N")).help_note_suffix(),
            "\n\nhelp: H\nnote: N"
        );
    }
}
