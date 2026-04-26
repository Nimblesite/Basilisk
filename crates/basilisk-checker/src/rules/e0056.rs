//! BSK-E0056: Invalid second argument to `TypeAliasType(...)`.
//!
//! `TypeAliasType(name, type_expr)` requires the second argument to be a
//! valid type expression.  The same restrictions as `TypeAlias` (BSK-E0048)
//! apply: no list/dict/tuple literals, no comprehensions, no lambdas,
//! no f-strings, no boolean/integer literals, no conditional expressions,
//! no boolean binary operators, no runtime calls like `eval(...)`.
//!
//! ```python
//! from typing import TypeAliasType
//! Bad1 = TypeAliasType("Bad1", [int, str])   # E — list literal
//! Bad2 = TypeAliasType("Bad2", True)         # E — bool literal
//! ```

use std::collections::HashSet;

use basilisk_resolver::{RhsKind, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0056",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0056",
};

fn make_diag(lhs_name: &str, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Invalid type expression as second argument of `TypeAliasType` for `{lhs_name}`"
        ),
        span,
        path: path.to_owned(),
        help: Some(
            "The second argument of `TypeAliasType` must be a valid type expression".to_owned(),
        ),
        note: Some(
            "PEP 613 / PEP 695: `TypeAliasType(name, T)` requires T to be a type, \
             not a literal or expression"
                .to_owned(),
        ),
    }
}

fn span_text(source: &str, span: Span) -> Option<&str> {
    source.get(span.start as usize..span.end as usize)
}

/// Returns `true` when the RHS text is an invalid type expression.
/// Mirrors the logic from `e0048::is_invalid_rhs`.
fn is_invalid_rhs(rhs: &str) -> bool {
    let rhs = rhs.trim();

    if rhs == "True" || rhs == "False" {
        return true;
    }
    if rhs.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    if rhs.starts_with('-')
        && rhs[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }
    if rhs.starts_with("f\"") || rhs.starts_with("f'") {
        return true;
    }
    if rhs.starts_with('[') {
        return true;
    }
    if rhs.starts_with('{') {
        return true;
    }
    if rhs.starts_with('(') && paren_has_top_level_comma(rhs) {
        return true;
    }
    if has_top_level_token(rhs, " if ") {
        return true;
    }
    if has_top_level_token(rhs, " or ") || has_top_level_token(rhs, " and ") {
        return true;
    }
    if rhs.contains("lambda") {
        return true;
    }
    if rhs.starts_with("eval(") {
        return true;
    }
    false
}

fn has_top_level_token(s: &str, token: &str) -> bool {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let tok = token.as_bytes();
    let tok_len = tok.len();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            _ if depth == 0 => {
                if bytes.get(i..i + tok_len) == Some(tok) {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

fn paren_has_top_level_comma(s: &str) -> bool {
    if s.len() < 2 {
        return false;
    }
    let inner = &s[1..s.len() - 1];
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Collect names of module-level variables that are not valid types.
fn collect_non_type_names(module: &ResolvedModule) -> HashSet<String> {
    module
        .module_vars
        .iter()
        .filter(|v| !v.has_annotation)
        .filter(|v| {
            matches!(
                v.rhs_kind,
                RhsKind::IntLiteral
                    | RhsKind::FloatLiteral
                    | RhsKind::StrLiteral
                    | RhsKind::BoolLiteral
                    | RhsKind::BytesLiteral
                    | RhsKind::EmptyList
                    | RhsKind::EmptyDict
                    | RhsKind::NoneValue
            )
        })
        .map(|v| v.name.clone())
        .collect()
}

/// Returns `true` when the RHS text is a bare identifier bound to a non-type variable.
fn is_non_type_name(rhs: &str, non_type_names: &HashSet<String>) -> bool {
    let rhs = rhs.trim();
    if rhs.contains('[') || rhs.contains('.') || rhs.contains('(') || rhs.contains(' ') {
        return false;
    }
    non_type_names.contains(rhs)
}

/// Emits BSK-E0056 when `TypeAliasType(name, bad_rhs)` has an invalid type expression.
pub(crate) struct TypeAliasTypeInvalidRhs;

impl Rule for TypeAliasTypeInvalidRhs {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;
        let non_type_names = collect_non_type_names(module);

        for call in &module.type_alias_type_calls {
            let Some(rhs_span) = call.rhs_span else {
                continue;
            };
            let Some(rhs) = span_text(source, rhs_span) else {
                continue;
            };
            let rhs_trimmed = rhs.trim();
            if is_invalid_rhs(rhs_trimmed) || is_non_type_name(rhs_trimmed, &non_type_names) {
                diagnostics.push(make_diag(&call.lhs_name, call.span, path));
            }
        }
    }
}
