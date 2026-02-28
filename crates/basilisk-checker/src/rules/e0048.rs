//! BSK-E0048: Invalid right-hand side for a `TypeAlias` annotation.
//!
//! PEP 613 requires that the RHS of an explicit `TypeAlias` annotation must be
//! a valid type expression. The following are errors:
//!
//! - List literals: `x: TypeAlias = [int, str]`
//! - Tuple literals: `x: TypeAlias = ((int, str),)`
//! - Dict literals: `x: TypeAlias = {"a": "b"}`
//! - List comprehensions: `x: TypeAlias = [int for i in range(1)]`
//! - Lambda calls: `x: TypeAlias = (lambda: int)()`
//! - Conditional expressions: `x: TypeAlias = int if cond else str`
//! - Boolean literals: `x: TypeAlias = True`
//! - Integer literals: `x: TypeAlias = 1`
//! - Binary boolean operators: `x: TypeAlias = list or set`
//! - F-strings: `x: TypeAlias = f"..."`
//! - Subscript-into-subscript: `x: TypeAlias = [int][0]`
//! - Runtime calls: `x: TypeAlias = eval("int")`
//!
//! ```python
//! from typing import TypeAlias
//! BadTypeAlias2: TypeAlias = [int, str]   # E — list literal
//! BadTypeAlias10: TypeAlias = True         # E — bool literal
//! ```

use basilisk_resolver::{ImportKind, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0048",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0048",
};

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    let span = span?;
    source.get(span.start as usize..span.end as usize)
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "The RHS of a `TypeAlias` annotation must be a valid type expression".to_owned(),
        ),
        note: Some(
            "PEP 613: `x: TypeAlias = T` requires T to be a type, not a literal or expression"
                .to_owned(),
        ),
    }
}

/// Collect all local names that refer to `typing.TypeAlias` in this module.
///
/// Handles:
/// - `from typing import TypeAlias`
/// - `from typing import TypeAlias as TA`
/// - `import typing` (used as `typing.TypeAlias`)
fn collect_type_alias_names(module: &ResolvedModule) -> Vec<String> {
    let mut names = vec!["TypeAlias".to_owned()];
    for import in &module.imports {
        if import.kind != ImportKind::From {
            continue;
        }
        if import.module != "typing" && import.module != "typing_extensions" {
            continue;
        }
        // Scan the raw import source text for `TypeAlias as <alias>` patterns.
        let import_span = import.span;
        let Some(import_text) =
            module.source.get(import_span.start as usize..import_span.end as usize)
        else {
            continue;
        };
        // Find all occurrences of `TypeAlias as <identifier>`
        let mut search = import_text;
        while let Some(pos) = search.find("TypeAlias as ") {
            let after = &search[pos + "TypeAlias as ".len()..];
            // Extract the identifier following `TypeAlias as `
            let alias: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !alias.is_empty() && alias != "TypeAlias" {
                names.push(alias);
            }
            search = &search[pos + 1..];
        }
    }
    names
}

/// Returns `true` when the annotation text matches one of the known `TypeAlias` names.
fn is_type_alias_annotation(ann: &str, type_alias_names: &[String]) -> bool {
    let ann = ann.trim();
    type_alias_names.iter().any(|n| ann == n) || ann.ends_with(".TypeAlias")
}

/// Returns `true` when the RHS text is an invalid type expression for a `TypeAlias`.
fn is_invalid_rhs(rhs: &str) -> bool {
    let rhs = rhs.trim();

    // Boolean literals
    if rhs == "True" || rhs == "False" {
        return true;
    }

    // Integer or float literals: starts with a digit
    if rhs.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }

    // Negative numeric literals
    if rhs.starts_with('-')
        && rhs[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }

    // F-string
    if rhs.starts_with("f\"") || rhs.starts_with("f'") {
        return true;
    }

    // List literal (starts with `[`) — also catches list comprehensions
    if rhs.starts_with('[') {
        return true;
    }

    // Dict literal
    if rhs.starts_with('{') {
        return true;
    }

    // Tuple literal: starts with `(` and has a comma at depth 0 inside
    if rhs.starts_with('(') && paren_has_top_level_comma(rhs) {
        return true;
    }

    // Conditional expression: has ` if ` at depth 0
    if has_top_level_token(rhs, " if ") {
        return true;
    }

    // Boolean binary operator `or` / `and` at depth 0
    if has_top_level_token(rhs, " or ") || has_top_level_token(rhs, " and ") {
        return true;
    }

    // Lambda (possibly called)
    if rhs.contains("lambda") {
        return true;
    }

    // Runtime call: eval(...)
    if rhs.starts_with("eval(") {
        return true;
    }

    false
}

/// Returns `true` when the text contains `token` at bracket depth 0.
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

/// Returns `true` when `(...)` contains a comma at depth 0 inside the parens.
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

/// Emits BSK-E0048 when a `TypeAlias`-annotated variable has an invalid RHS type expression.
pub(crate) struct TypeAliasInvalidRhs;

impl Rule for TypeAliasInvalidRhs {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;
        let type_alias_names = collect_type_alias_names(module);

        for var in &module.module_vars {
            let Some(ann) = span_text(source, var.annotation_span) else {
                continue;
            };
            if !is_type_alias_annotation(ann.trim(), &type_alias_names) {
                continue;
            }
            let Some(rhs_span) = var.rhs_span else {
                continue;
            };
            let Some(rhs) = span_text(source, Some(rhs_span)) else {
                continue;
            };
            if is_invalid_rhs(rhs.trim()) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression as right-hand side of `TypeAlias` for `{}`",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            }
        }
    }
}
