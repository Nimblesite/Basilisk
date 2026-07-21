//! Implements [`classes_classvar`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! Shared helper utilities for `classes_classvar`: text-based `ClassVar` detection,
//! diagnostic construction, and the `TypeParamKind` classification enum.

use basilisk_resolver::Span;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

/// The error code for this rule.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "classes_classvar",
    docs_url: "https://www.basilisk-python.dev/errors/classes_classvar",
};

/// Classification of a type parameter for error messaging.
#[derive(Debug, Clone, Copy)]
pub(super) enum TypeParamKind {
    /// A `TypeVar` type parameter.
    TypeVar,
    /// A `ParamSpec` type parameter.
    ParamSpec,
    /// A `TypeVarTuple` type parameter.
    TypeVarTuple,
}

/// Returns `true` when the annotation text contains `ClassVar[` at all —
/// used for contexts where ANY `ClassVar` usage is invalid (function params,
/// return types, module-level annotations).
pub(super) fn has_classvar(ann: &str) -> bool {
    ann.contains("ClassVar[") || ann.contains("ClassVar ")
}

/// Returns `true` when `ClassVar` or an alias like `CV` appears as a bare
/// name or subscript in an annotation string.
pub(super) fn has_classvar_or_alias(ann: &str) -> bool {
    has_classvar(ann) || ann.contains("CV[") || ann == "ClassVar" || ann == "CV"
}

/// Returns `true` when the annotation text contains `ClassVar` nested inside
/// another type constructor.  `Annotated[ClassVar[...], ...]` is excluded
/// because that is a valid usage per the typing spec.
///
/// Pattern: `[ClassVar[` appears in the annotation (meaning something wraps it)
/// AND the annotation does not begin with `Annotated[`.
pub(super) fn has_nested_classvar(ann: &str) -> bool {
    ann.contains("[ClassVar[") && !ann.starts_with("Annotated[")
}

/// Construct a `classes_classvar` diagnostic with standard help and note text.
pub(super) fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some("`ClassVar` is only valid as a class body attribute annotation".to_owned()),
        Some(
            "PEP 526: `ClassVar` cannot appear in function signatures, local variables, \
             or module-level annotations, and cannot be nested inside another type"
                .to_owned(),
        ),
    )
}

/// Returns the text slice for an optional span within the source.
///
/// Returns `None` when the span is `None` or out of bounds.
pub(super) fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

/// Returns `true` if the byte is an ASCII alphanumeric or underscore character.
pub(super) fn is_ident_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Check if `text` contains `word` as a standalone identifier (not part of a larger name).
pub(super) fn contains_word(text: &str, word: &str) -> bool {
    let word_bytes = word.as_bytes();
    let text_bytes = text.as_bytes();
    let word_len = word_bytes.len();

    if word_len > text_bytes.len() {
        return false;
    }

    for start_idx in 0..=text_bytes.len().saturating_sub(word_len) {
        if text_bytes.get(start_idx..start_idx + word_len) == Some(word_bytes) {
            // Check that the character before (if any) is not alphanumeric or underscore
            let before_ok = start_idx == 0
                || !text_bytes
                    .get(start_idx.saturating_sub(1))
                    .is_some_and(|&b| is_ident_char(b));
            // Check that the character after (if any) is not alphanumeric or underscore
            let after_ok = start_idx + word_len >= text_bytes.len()
                || !text_bytes
                    .get(start_idx + word_len)
                    .is_some_and(|&b| is_ident_char(b));
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
}
