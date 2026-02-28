//! Diagnostic data types for Basilisk.

use basilisk_resolver::Span;

/// The severity level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// A stylistic suggestion — does not block CI by default.
    Warning,
    /// A type error that must be resolved.
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// A BSK diagnostic code such as `BSK-E0001`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorCode {
    /// The full code string, e.g. `"BSK-E0001"`.
    pub code: &'static str,
    /// URL to the documentation for this diagnostic.
    pub docs_url: &'static str,
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
}
