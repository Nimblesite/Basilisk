//! Utility functions for BSK-E0036.

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::types::TypeParamKind;

pub(super) const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0036",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0036",
};

/// Build a `BSK-E0036` diagnostic.
pub(super) fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some("`ClassVar` is only valid as a class body attribute annotation".to_owned()),
        note: Some(
            "PEP 526: `ClassVar` cannot appear in function signatures, local variables, \
             or module-level annotations, and cannot be nested inside another type"
                .to_owned(),
        ),
    }
}

/// Returns the text slice for a span within the source.
pub(super) fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    use crate::span_util::slice_span;
    slice_span(source, span?)
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

/// Extract the content between the outer `[...]` of a `ClassVar[...]` or `CV[...]`
/// annotation text.  Returns `None` when there is no subscript.
pub(super) fn extract_classvar_inner(ann: &str) -> Option<&str> {
    // Find the start: "ClassVar[" or "CV["
    let prefix_len = if ann.starts_with("ClassVar[") {
        "ClassVar[".len()
    } else if ann.starts_with("CV[") {
        "CV[".len()
    } else if ann.starts_with("Annotated[ClassVar[") {
        // Skip Annotated wrapper — valid per spec
        return None;
    } else {
        return None;
    };

    // Find the matching closing bracket by counting nesting
    let bytes = ann.as_bytes();
    let mut depth: u32 = 1;
    let mut end_idx = None;
    for (idx, &byte) in bytes.iter().enumerate().skip(prefix_len) {
        match byte {
            b'[' => depth = depth.saturating_add(1),
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end_idx = Some(idx);
                    break;
                }
            }
            _ => {}
        }
    }

    end_idx.and_then(|end| ann.get(prefix_len..end))
}

/// Count the number of top-level comma-separated arguments in a bracket body.
pub(super) fn count_top_level_args(inner: &str) -> usize {
    if inner.trim().is_empty() {
        return 0;
    }
    let mut depth: u32 = 0;
    let mut count: usize = 1;
    for byte in inner.as_bytes() {
        match byte {
            b'[' | b'(' => depth = depth.saturating_add(1),
            b']' | b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => count = count.saturating_add(1),
            _ => {}
        }
    }
    count
}

/// Returns `true` when the argument text looks like a numeric literal (e.g. `3`, `3.14`).
pub(super) fn is_numeric_literal(arg: &str) -> bool {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return false;
    }
    // A numeric literal: all digits, optionally with a single dot
    trimmed
        .chars()
        .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+')
        && trimmed.chars().any(|ch| ch.is_ascii_digit())
}

/// Known built-in type names that are valid as `ClassVar` arguments even though
/// they start with lowercase (e.g. `int`, `str`, `float`, `bool`, `bytes`, `list`,
/// `dict`, `set`, `tuple`, `type`, `object`, `complex`, `range`, `slice`,
/// `frozenset`, `bytearray`, `memoryview`).
const LOWERCASE_TYPE_NAMES: &[&str] = &[
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "list",
    "dict",
    "set",
    "tuple",
    "type",
    "object",
    "complex",
    "range",
    "slice",
    "frozenset",
    "bytearray",
    "memoryview",
    "property",
    "staticmethod",
    "classmethod",
    "super",
];

/// Returns `true` when the argument text looks like a runtime variable reference
/// (a simple identifier that is not a known type name).
///
/// A bare identifier that starts with a lowercase letter and is NOT one of the
/// known built-in types is considered a runtime variable.
pub(super) fn is_runtime_variable(arg: &str, module_var_names: &[String]) -> bool {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Must be a simple identifier (no brackets, dots, etc.)
    if !trimmed.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
        return false;
    }
    // Check if it's a known module-level variable (runtime value)
    if module_var_names.iter().any(|name| name == trimmed) {
        return true;
    }
    // A bare lowercase identifier that is not a known type is likely a runtime variable
    let first_char = trimmed.chars().next();
    if first_char.is_some_and(|ch| ch.is_ascii_lowercase()) {
        return !LOWERCASE_TYPE_NAMES.contains(&trimmed);
    }
    false
}

/// Check if an annotation's `ClassVar` argument contains any of the given type
/// parameter names (`TypeVar`, `ParamSpec`, `TypeVarTuple` names).
pub(super) fn contains_type_param(
    ann_inner: &str,
    type_param_names: &[(String, TypeParamKind)],
) -> Option<TypeParamKind> {
    for (name, kind) in type_param_names {
        // Check for the name appearing as a standalone word or as part of a subscript
        // e.g. `T` in `list[T]`, `P` in `Callable[P, Any]`
        if contains_word(ann_inner, name) {
            return Some(*kind);
        }
    }
    None
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

/// Returns `true` if the byte is an ASCII alphanumeric or underscore character.
pub(super) fn is_ident_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
