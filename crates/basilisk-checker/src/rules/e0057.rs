//! Implements [BSK-E0057] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-structural
//! BSK-E0057: Invalid RHS in a PEP 695 `type X = rhs` statement.
//!
//! PEP 695 requires the RHS of a `type` statement to be a valid type expression.
//! The same restrictions as `TypeAlias` (BSK-E0048) apply.
//!
//! ```python
//! type BadAlias1 = [int, str]   # E — list literal
//! type BadAlias2 = True         # E — bool literal
//! type BadAlias3 = 1            # E — int literal
//! ```

use std::collections::HashSet;

use basilisk_resolver::{ResolvedModule, RhsKind, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0057",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0057",
};

fn make_diag(name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Invalid type expression in `type {name}` alias"),
        span,
        path,
        Some("The RHS of a `type` statement must be a valid type expression".to_owned()),
        Some(
            "PEP 695: `type X = T` requires T to be a type, not a literal or expression".to_owned(),
        ),
    )
}

fn span_text(source: &str, span: Span) -> Option<&str> {
    slice_span(source, span)
}

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
        match bytes.get(i).copied() {
            Some(b'[' | b'(' | b'{') => depth += 1,
            Some(b']' | b')' | b'}') => depth -= 1,
            Some(_) if depth == 0 && bytes.get(i..i + tok_len) == Some(tok) => {
                return true;
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
    crate::rules::shared::contains_top_level_comma(&s[1..s.len() - 1])
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

/// Emits BSK-E0057 when a `type X = rhs` statement has an invalid type expression.
pub(crate) struct TypeStatementInvalidRhs;

impl Rule for TypeStatementInvalidRhs {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;
        let non_type_names = collect_non_type_names(module);

        for stmt in &module.type_statements {
            let Some(rhs) = span_text(source, stmt.rhs_span) else {
                continue;
            };
            let rhs_trimmed = rhs.trim();
            if is_invalid_rhs(rhs_trimmed) || is_non_type_name(rhs_trimmed, &non_type_names) {
                diagnostics.push(make_diag(&stmt.name, stmt.name_span, path));
            }
        }
    }
}
