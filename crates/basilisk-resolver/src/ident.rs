//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Python-identifier predicate.
//!
//! Used by checker rules, the resolver, and helpers to decide whether a string
//! token is a simple Python identifier (`[A-Za-z_][A-Za-z0-9_]*`, plus Unicode
//! letters at the start when the input may contain them). Centralised so each
//! rule does not re-implement the same six-line predicate.

/// Returns `true` when `s` is a simple Python identifier — non-empty,
/// the first character is a letter or `_`, and the rest are alphanumerics
/// or `_`. Accepts Unicode letters (matching Python 3 syntax for identifiers).
#[must_use]
pub fn is_simple_python_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// As [`is_simple_python_identifier`] but accepts only ASCII letters. Use this
/// when the identifier source is known to be ASCII (Python annotations, stub
/// content), where a Unicode pass would be a needless cost.
#[must_use]
pub fn is_simple_ascii_python_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
