//! Diagnostic data types for Basilisk.

use basilisk_resolver::Span;
use basilisk_stubs::TypeProvenance;

/// The severity level of a diagnostic.
///
/// Every rule has a default severity determined by its code prefix (`E` = Error,
/// `W` = Warning). All rules can be overridden to any severity at the line,
/// block, file, path, or project level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational hint — does not block CI, shown as blue in LSP.
    Info,
    /// A stylistic suggestion — does not block CI by default.
    Warning,
    /// A type error that must be resolved.
    Error,
    /// Critical Mojo safety violation (ownership/move semantics).
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

/// A BSK diagnostic code such as `BSK-E0001`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorCode {
    /// The full code string, e.g. `"BSK-E0001"`.
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
#[derive(Debug, Clone)]
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
