//! BSK-E0051: Invalid `Literal` parameterization.
//!
//! PEP 586 restricts what values may appear inside `Literal[...]`.
//! Only these are legal:
//!   - Integer literals (decimal, hex, binary, octal; optionally signed)
//!   - String literals (`str` and `bytes`)
//!   - Boolean literals (`True`, `False`)
//!   - `None`
//!   - Enum member access (`Color.RED`)
//!   - Nested `Literal[...]`
//!
//! Everything else is illegal, including:
//!   - Arithmetic / unary expressions (`3 + 4`, `~5`, `not False`)
//!   - Function calls (`"foo".replace(...)`)
//!   - Containers (`(1, 2)`, `{"a": "b"}`)
//!   - Type objects, `TypeVar`s, `Any` (`Literal[int]`, `Literal[T]`)
//!   - Float literals (`3.14`)
//!   - Ellipsis (`...`)
//!   - Bare `Literal` with no arguments
//!   - Variables and function objects

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0051",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0051",
};

fn make_diag(message: String, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "Literal[] only accepts int, str, bytes, bool, None, enum members, or nested Literal"
                .to_owned(),
        ),
        note: Some(
            "PEP 586: expressions, floats, type objects, ellipsis, and variables are forbidden"
                .to_owned(),
        ),
    }
}

// ---------------------------------------------------------------------------
// Annotation text helpers
// ---------------------------------------------------------------------------

/// `true` when `ann` is a bare `Literal` with no subscript.
fn is_bare_literal(ann: &str) -> bool {
    ann == "Literal" || ann.ends_with(".Literal")
}

/// `true` when `ann` starts a `Literal[...]` subscript.
fn is_literal_subscript(ann: &str) -> bool {
    ann.starts_with("Literal[") || ann.contains(".Literal[")
}

/// Extract the content between the outermost `Literal[` and its matching `]`.
fn extract_literal_inner(ann: &str) -> Option<&str> {
    let start_bracket = ann.find("Literal[")? + "Literal[".len();
    let mut depth = 1i32;
    let bytes = ann.as_bytes();
    let mut i = start_bracket;
    while i < bytes.len() {
        match bytes.get(i).copied() {
            Some(b'[') => depth += 1,
            Some(b']') => {
                depth -= 1;
                if depth == 0 {
                    return ann.get(start_bracket..i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// `true` when the inner content of `Literal[<content>]` contains an invalid arg.
fn has_invalid_literal_param(ann: &str) -> bool {
    let Some(inner) = extract_literal_inner(ann) else {
        return false;
    };
    split_top_level_commas(inner.trim())
        .into_iter()
        .any(|arg| is_invalid_single_arg(arg.trim()))
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                result.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(&s[start..]);
    result
}

fn is_invalid_single_arg(arg: &str) -> bool {
    // Nested Literal — recurse, the outer nesting is fine
    if arg.starts_with("Literal[") || arg.contains(".Literal[") {
        return has_invalid_literal_param(arg);
    }
    // None, True, False — legal
    if matches!(arg, "None" | "True" | "False") {
        return false;
    }
    // Bytes literal — must be a complete literal (ends with closing quote), not a method call
    if (arg.starts_with("b\"") || arg.starts_with("b'")) && is_complete_string_literal(arg) {
        return false;
    }
    // String literal — must be a complete literal (ends with matching closing quote),
    // not a method call like `"foo".replace(...)`.
    if (arg.starts_with('"')
        || arg.starts_with('\'')
        || arg.starts_with("r\"")
        || arg.starts_with("r'"))
        && is_complete_string_literal(arg)
    {
        return false;
    }
    // Enum member (e.g. Color.RED) — no spaces, no parens, exactly one dot
    if is_enum_member(arg) {
        return false;
    }
    // Integer literal (optionally signed; hex/bin/oct ok)
    if is_integer_literal(arg) {
        return false;
    }
    // Anything else is illegal
    true
}

fn is_integer_literal(arg: &str) -> bool {
    let s = arg.trim();
    let s = s
        .strip_prefix('-')
        .or_else(|| s.strip_prefix('+'))
        .map_or(s, str::trim);
    if s.is_empty() {
        return false;
    }
    if let Some(rest) = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .or_else(|| s.strip_prefix("0b"))
        .or_else(|| s.strip_prefix("0B"))
        .or_else(|| s.strip_prefix("0o"))
        .or_else(|| s.strip_prefix("0O"))
    {
        return !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric());
    }
    s.chars().all(|c| c.is_ascii_digit())
}

fn is_enum_member(arg: &str) -> bool {
    if arg.contains('(') || arg.contains('[') || arg.contains(' ') {
        return false;
    }
    let mut parts = arg.splitn(2, '.');
    let Some(obj) = parts.next() else {
        return false;
    };
    let Some(attr) = parts.next() else {
        return false;
    };
    !attr.contains('.') && is_ident(obj) && is_ident(attr)
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Returns `true` when `arg` is a *complete* string literal — i.e. it starts and ends
/// with matching quote characters with nothing after the closing quote.
///
/// This distinguishes `"foo"` (valid Literal string) from `"foo".replace("o", "b")`
/// (a string method call, which is not a valid Literal argument).
fn is_complete_string_literal(arg: &str) -> bool {
    // Determine the quote character(s): triple quotes or single.
    let (quote, body_start) = if arg.starts_with("\"\"\"") || arg.starts_with("'''") {
        let q = &arg[..3];
        (q.to_owned(), 3usize)
    } else if arg.starts_with("b\"")
        || arg.starts_with("b'")
        || arg.starts_with("r\"")
        || arg.starts_with("r'")
    {
        let q = arg.chars().nth(1).map_or('"', |c| c).to_string();
        (q, 2usize)
    } else if arg.starts_with('"') || arg.starts_with('\'') {
        let q = arg.chars().next().map_or('"', |c| c).to_string();
        (q, 1usize)
    } else {
        return false;
    };
    // The body must end with the same closing quote and nothing more.
    let body = &arg[body_start..];
    body.ends_with(quote.as_str()) && body.len() >= quote.len()
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// Emits BSK-E0051 for invalid `Literal[...]` parameterizations.
pub(crate) struct InvalidLiteralParam;

impl Rule for InvalidLiteralParam {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Module-level variable annotations: `x: Literal[...]`
        for var in &module.module_vars {
            let Some(ann_span) = var.annotation_span else {
                continue;
            };
            let Some(ann) = slice_span(source, ann_span) else {
                continue;
            };
            let ann = ann.trim();

            if is_bare_literal(ann) {
                diagnostics.push(make_diag(
                    format!(
                        "`Literal` must be parameterized (variable `{}` has no arguments)",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            } else if is_literal_subscript(ann) && has_invalid_literal_param(ann) {
                diagnostics.push(make_diag(
                    format!(
                        "Invalid parameterization of `Literal` in annotation for `{}`",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            }
        }

        // Function parameter annotations
        for func in &module.functions {
            for param in &func.parameters {
                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann) = slice_span(source, ann_span) else {
                    continue;
                };
                let ann = ann.trim();

                if is_bare_literal(ann) {
                    diagnostics.push(make_diag(
                        format!(
                            "`Literal` must be parameterized (parameter `{}` has no arguments)",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                } else if is_literal_subscript(ann) && has_invalid_literal_param(ann) {
                    diagnostics.push(make_diag(
                        format!(
                            "Invalid parameterization of `Literal` for parameter `{}`",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
            }
        }
    }
}
