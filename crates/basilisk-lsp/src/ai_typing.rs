//! Implements [LSPAI]. See docs/specs/LSP-AI-SPEC.md#LSPAI
//!
//! AI Typing hooks — trait-based interface for AI-assisted type inference.
//!
//! This module defines the [`AiTypingProvider`] trait and supporting types that
//! allow AI models to suggest type annotations for diagnostics that cannot be
//! resolved by deterministic analysis alone.
//!
//! **No AI provider is implemented here** — only the interface and a no-op
//! default. A future implementor can add a real provider by implementing the
//! trait and registering it.

use std::fmt;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for AI typing operations.
#[derive(Debug)]
pub enum AiTypingError {
    /// The AI provider is not configured or unavailable.
    Unavailable,
    /// The AI provider returned an error.
    ProviderError(String),
    /// The request timed out.
    Timeout,
}

impl fmt::Display for AiTypingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(f, "AI typing provider is not available"),
            Self::ProviderError(msg) => write!(f, "AI typing provider error: {msg}"),
            Self::Timeout => write!(f, "AI typing request timed out"),
        }
    }
}

impl std::error::Error for AiTypingError {}

// ---------------------------------------------------------------------------
// Classification enums
// ---------------------------------------------------------------------------

/// Safety classification for a proposed fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixSafety {
    /// Guaranteed not to change runtime semantics.
    Safe,
    /// Might change semantics or could be wrong. Requires review.
    Unsafe,
}

/// Origin of a proposed fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixSource {
    /// Deterministic fix derived from the rule definition.
    RuleBased,
    /// Heuristic fix based on usage patterns and type inference.
    Heuristic,
    /// AI-assisted fix (suggested by an AI model).
    AiAssisted,
}

// ---------------------------------------------------------------------------
// Request / Response
// ---------------------------------------------------------------------------

/// Context payload sent to an AI typing provider.
#[derive(Debug)]
pub struct AiTypingRequest {
    /// The diagnostic code (e.g. `"BSK-E0001"`).
    pub diagnostic_code: String,
    /// Human-readable diagnostic message.
    pub diagnostic_message: String,
    /// The function/class/module source code surrounding the diagnostic.
    pub source_context: String,
    /// File path of the diagnostic.
    pub file_path: String,
    /// 0-based line number of the diagnostic.
    pub line: u32,
    /// 0-based column of the diagnostic.
    pub column: u32,
}

/// Response from an AI typing provider.
#[derive(Debug)]
pub struct AiTypingResponse {
    /// The proposed type annotation text (e.g. `"int"`, `"list[str]"`, `"Optional[int]"`).
    pub suggested_type: String,
    /// Confidence score (`0.0`–`1.0`).
    pub confidence: f32,
    /// Human-readable explanation of why this type was chosen.
    pub reasoning: String,
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Trait for AI-assisted type inference providers.
///
/// Implementors suggest type annotations for diagnostics that deterministic
/// analysis cannot resolve. All AI-suggested fixes are classified as
/// [`FixSafety::Unsafe`] and [`FixSource::AiAssisted`].
pub trait AiTypingProvider: Send + Sync {
    /// Given diagnostic context, suggest a type annotation fix.
    ///
    /// Returns `Ok(None)` when the provider has no suggestion.
    /// Returns `Ok(Some(response))` with a suggested type.
    ///
    /// # Errors
    ///
    /// Returns `AiTypingError` if the provider encounters a failure
    /// (e.g. network error, rate limit, invalid response).
    fn suggest_fix(
        &self,
        request: &AiTypingRequest,
    ) -> Result<Option<AiTypingResponse>, AiTypingError>;

    /// Whether this provider is available and configured.
    fn is_available(&self) -> bool;
}

// ---------------------------------------------------------------------------
// No-op provider
// ---------------------------------------------------------------------------

/// No-op AI typing provider — always returns `None`.
///
/// This is the default provider used when AI typing is not configured.
#[derive(Debug)]
pub struct NoOpAiTypingProvider;

impl AiTypingProvider for NoOpAiTypingProvider {
    fn suggest_fix(
        &self,
        _request: &AiTypingRequest,
    ) -> Result<Option<AiTypingResponse>, AiTypingError> {
        Ok(None)
    }

    fn is_available(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;

    fn sample_request() -> AiTypingRequest {
        AiTypingRequest {
            diagnostic_code: "BSK-E0001".to_owned(),
            diagnostic_message: "Missing type annotation".to_owned(),
            source_context: "def foo(x): ...".to_owned(),
            file_path: "test.py".to_owned(),
            line: 0,
            column: 8,
        }
    }

    #[test]
    fn noop_provider_returns_none() {
        let provider = NoOpAiTypingProvider;
        let result = provider.suggest_fix(&sample_request()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn noop_provider_is_not_available() {
        let provider = NoOpAiTypingProvider;
        assert!(!provider.is_available());
    }

    #[test]
    fn error_display_unavailable() {
        let err = AiTypingError::Unavailable;
        assert_eq!(err.to_string(), "AI typing provider is not available");
    }

    #[test]
    fn error_display_provider_error() {
        let err = AiTypingError::ProviderError("connection refused".to_owned());
        assert_eq!(
            err.to_string(),
            "AI typing provider error: connection refused"
        );
    }

    #[test]
    fn error_display_timeout() {
        let err = AiTypingError::Timeout;
        assert_eq!(err.to_string(), "AI typing request timed out");
    }

    #[test]
    fn fix_safety_equality() {
        assert_eq!(FixSafety::Safe, FixSafety::Safe);
        assert_ne!(FixSafety::Safe, FixSafety::Unsafe);
    }

    #[test]
    fn fix_source_equality() {
        assert_eq!(FixSource::RuleBased, FixSource::RuleBased);
        assert_ne!(FixSource::RuleBased, FixSource::AiAssisted);
        assert_ne!(FixSource::Heuristic, FixSource::AiAssisted);
    }
}
