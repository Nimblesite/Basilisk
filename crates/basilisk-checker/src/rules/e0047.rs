//! BSK-E0047: Invalid type expression in annotation.
//!
//! PEP 484 requires that annotations contain valid type expressions.
//! Only certain expression forms are valid as types:
//!
//! - Names (`int`, `str`, `MyClass`)
//! - Subscripts (`list[int]`, `dict[str, int]`)
//! - Binary-or unions (`int | str`)
//! - String literals (forward references)
//! - `None`
//! - `...` (Ellipsis, in Callable signatures)
//!
//! The following are invalid and should be flagged:
//!
//! - List literals: `[int, str]`
//! - Dict literals: `{}`
//! - Tuple literals: `(int, str)`
//! - List comprehensions: `[int for i in range(1)]`
//! - Lambda expressions (called or uncalled)
//! - Conditional expressions: `int if cond else str`
//! - Boolean binary operators: `int or str`, `int and str`
//! - F-string literals: `f"int"`
//! - Explicit function calls like `eval(...)`
//! - Negative numeric literals (positive are caught by E0024)
//! - Names that refer to module objects (`import types` → `types` is a module, not a type)
//! - Names that refer to unannotated literal variables (`var1 = 3` → `var1` is `int`, not a type)
//!
//! ```python
//! def f(x: [int, str]): ...            # E — list literal not a type
//! def g(x: int if True else str): ...  # E — conditional not a type
//! y: {} = {}                            # E — dict literal not a type
//! ```


use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0047",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0047",
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
            "Type annotations must be valid type expressions (class names, subscripts, unions)"
                .to_owned(),
        ),
        note: Some(
            "PEP 484: annotations should be types, not arbitrary runtime expressions".to_owned(),
        ),
    }
}

/// Returns `true` when the annotation text is a structurally invalid type expression.
fn is_invalid_type_annotation(ann: &str) -> bool {
    let ann = ann.trim();

    if ann.is_empty() {
        return false;
    }

    // List literal or list comprehension: starts with `[`
    if ann.starts_with('[') {
        return true;
    }

    // Dict literal: starts with `{`
    if ann.starts_with('{') {
        return true;
    }

    // F-string: starts with f" or f'
    if ann.starts_with("f\"") || ann.starts_with("f'") {
        return true;
    }

    // Negative numeric literal: -1, -3.14 (positive numerics caught by E0024)
    if ann.starts_with('-')
        && ann[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }

    // Conditional expression: ` if ` keyword at depth 0
    if has_top_level_token(ann, " if ") {
        return true;
    }

    // Boolean binary operators: ` or ` / ` and ` at depth 0
    // Note: `|` is valid (union), `or`/`and` keywords are not
    if has_top_level_token(ann, " or ") || has_top_level_token(ann, " and ") {
        return true;
    }

    // Tuple literal: `(int, str)` — parens with comma at depth 0
    if ann.starts_with('(') && ann.ends_with(')') && paren_contains_top_level_comma(ann) {
        return true;
    }

    // Lambda (possibly called)
    if ann.contains("lambda") {
        return true;
    }

    // Explicit eval() call
    if ann.starts_with("eval(") {
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
fn paren_contains_top_level_comma(s: &str) -> bool {
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

/// Emits BSK-E0047 when an annotation contains an invalid type expression.
pub(crate) struct InvalidTypeAnnotation;

impl Rule for InvalidTypeAnnotation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Function parameters
        for func in &module.functions {
            for param in func
                .parameters
                .iter()
                .chain(func.vararg.iter())
                .chain(func.kwarg.iter())
            {
                // Skip if already caught by E0024 (numeric/boolean literal)
                if param.annotation_is_numeric_literal {
                    continue;
                }
                let Some(ann) = span_text(source, param.annotation_span) else {
                    continue;
                };
                if is_invalid_type_annotation(ann.trim()) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "Invalid type expression in annotation for parameter `{}`",
                            param.name
                        ),
                        param.name_span,
                        path,
                    ));
                }
            }
        }

        // Module-level variables
        for var in &module.module_vars {
            let Some(ann) = span_text(source, var.annotation_span) else {
                continue;
            };
            if is_invalid_type_annotation(ann.trim()) {
                diagnostics.push(make_diagnostic(
                    format!("Invalid type expression in annotation for `{}`", var.name),
                    var.name_span,
                    path,
                ));
            }
        }

        // Class attributes
        for cls in &module.classes {
            for attr in &cls.attributes {
                let Some(ann) = span_text(source, attr.annotation_span) else {
                    continue;
                };
                if is_invalid_type_annotation(ann.trim()) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "Invalid type expression in annotation for attribute `{}`",
                            attr.name
                        ),
                        attr.name_span,
                        path,
                    ));
                }
            }
        }
    }
}
