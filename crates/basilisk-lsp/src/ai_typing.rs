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
//
// Implements [LSPAI-ERRORS] — PARTIAL: spec names `AiProviderError` with variants
// `NotConfigured`, `Transport`, `MalformedResponse`, `RateLimited(Option<Duration>)`,
// `Refused`, `Other`. This module ships a narrower `AiTypingError`
// (`Unavailable`/`ProviderError`/`Timeout`) for the type-annotation slice only.
// DEVIATION: type name and variant set differ from the spec.
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
//
// Implements [LSPAI-PRINCIPLES] (principle 3 "Always unsafe") — `FixSource::AiAssisted`
// is the marker every AI-generated fix carries; pairs with `FixSafety::Unsafe`.
// (The spec section is otherwise narrative; only this concrete enum is behavioral.)
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
//
// Implements [LSPAI-CONTEXT] — PARTIAL: a single flat request carrying the diagnostic,
// surrounding source, and position. DEVIATION: the spec's structured payload
// ([LSPAI-TYPES-FIX]: `InferredTypeInfo`, `CallSiteContext`, available-import types,
// `is_batch`) is NOT present — only a `source_context: String` blob.
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
//
// Implements [LSPAI-TYPES-FIX] — PARTIAL (response side, annotation use-case only):
// carries `suggested_type`, `confidence`, and `reasoning`. DEVIATION: the spec returns
// a full `Fix` plus ranked `alternatives`; this returns a single type string.
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
//
// Implements [LSPAI-TRAIT] — PARTIAL: spec's `AiProvider` declares `name`,
// `capabilities`, and 9 `suggest_*`/`generate_*`/`explain_*`/`enhance_*` methods.
// This `AiTypingProvider` ships only `suggest_fix` + `is_available` (the
// type-annotation slice). DEVIATION: trait name and method surface differ;
// `name()`/`capabilities()` and all non-fix features are UNIMPLEMENTED.
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
//
// Implements [LSPAI-PROVIDERS] (the `NoOpProvider` row) — the offline-first default
// from [LSPAI-PRINCIPLES] principle 4: `is_available() == false`, zero overhead.
// The other rows (`OpenAiCompatibleProvider`, `AnthropicProvider`, `CopilotProvider`,
// `ProcessProvider`) are UNIMPLEMENTED.
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

    // Exercises [LSPAI-PROVIDERS] (NoOpProvider) + [LSPAI-TRAIT] suggest_fix.
    #[test]
    fn noop_provider_returns_none() {
        let provider = NoOpAiTypingProvider;
        let result = provider.suggest_fix(&sample_request()).unwrap();
        assert!(result.is_none());
    }

    // Exercises [LSPAI-PROVIDERS] (NoOpProvider) + [LSPAI-PRINCIPLES] principle 4
    // (offline-first default): is_available() == false.
    #[test]
    fn noop_provider_is_not_available() {
        let provider = NoOpAiTypingProvider;
        assert!(!provider.is_available());
    }

    // Exercises [LSPAI-ERRORS] — Display for the (partial) error type.
    #[test]
    fn error_display_unavailable() {
        let err = AiTypingError::Unavailable;
        assert_eq!(err.to_string(), "AI typing provider is not available");
    }

    // Exercises [LSPAI-ERRORS].
    #[test]
    fn error_display_provider_error() {
        let err = AiTypingError::ProviderError("connection refused".to_owned());
        assert_eq!(
            err.to_string(),
            "AI typing provider error: connection refused"
        );
    }

    // Exercises [LSPAI-ERRORS].
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

    // Exercises [LSPAI-PRINCIPLES] principle 3 — FixSource::AiAssisted marker.
    #[test]
    fn fix_source_equality() {
        assert_eq!(FixSource::RuleBased, FixSource::RuleBased);
        assert_ne!(FixSource::RuleBased, FixSource::AiAssisted);
        assert_ne!(FixSource::Heuristic, FixSource::AiAssisted);
    }
}
