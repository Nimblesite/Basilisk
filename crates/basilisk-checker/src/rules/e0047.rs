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

use std::collections::HashSet;

use basilisk_resolver::{ImportKind, RhsKind, ResolvedModule, Span};

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

    // Handle string literal annotations (forward references)
    // If the annotation is a string literal, check the content inside
    let content_to_check = if (ann.starts_with('"') && ann.ends_with('"'))
        || (ann.starts_with('\'') && ann.ends_with('\''))
    {
        &ann[1..ann.len() - 1]
    } else {
        ann
    };

    // `Annotated[...]` annotations are validated by E0045, which checks the first argument
    // and enforces the minimum-two-arguments rule.  The metadata arguments (2nd+) may be
    // arbitrary runtime values including lambdas, calls, and literals, so checking the full
    // `Annotated[...]` text here would produce false positives.
    if content_to_check.starts_with("Annotated[") {
        return false;
    }

    // List literal or list comprehension: starts with `[`
    if content_to_check.starts_with('[') {
        return true;
    }

    // Dict literal: starts with `{`
    if content_to_check.starts_with('{') {
        return true;
    }

    // F-string: starts with f" or f'
    if content_to_check.starts_with("f\"") || content_to_check.starts_with("f'") {
        return true;
    }

    // Numeric literal (positive or negative): 1, -1, 3.14
    // Positive numerics as direct annotations are caught by E0024, but when they appear
    // inside string annotations (e.g. `"1"`, `"3.14"`) E0024 does not fire.
    if content_to_check.starts_with('-')
        && content_to_check[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }
    // Positive numeric literal inside a string annotation: `"1"`, `"42"`, `"3.14"`
    if !content_to_check.is_empty()
        && content_to_check
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
        && content_to_check
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }
    // Boolean literal used as a type annotation: `"True"`, `"False"`
    if content_to_check == "True" || content_to_check == "False" {
        return true;
    }

    // Conditional expression: ` if ` keyword at depth 0
    if has_top_level_token(content_to_check, " if ") {
        return true;
    }

    // Boolean binary operators: ` or ` / ` and ` at depth 0
    // Note: `|` is valid (union), `or`/`and` keywords are not
    if has_top_level_token(content_to_check, " or ") || has_top_level_token(content_to_check, " and ") {
        return true;
    }

    // Tuple literal: `(int, str)` — parens with comma at depth 0
    if content_to_check.starts_with('(') && content_to_check.ends_with(')') && paren_contains_top_level_comma(content_to_check) {
        return true;
    }

    // Lambda (possibly called)
    if content_to_check.contains("lambda") {
        return true;
    }

    // Explicit eval() call
    if content_to_check.starts_with("eval(") {
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

/// Build a set of names that are definitely not valid type expressions:
/// - Names bound to modules via plain `import X` statements (the module object is not a type).
/// - Names bound to unannotated simple literal values (`var1 = 3` → `var1` is `int`, not a type).
fn collect_non_type_names(module: &ResolvedModule) -> HashSet<String> {
    let mut names = HashSet::new();

    // Plain `import X` binds `X` to a module object in scope.
    for import in &module.imports {
        if import.kind == ImportKind::Plain {
            // `import os.path` binds `os` (first component). For `import types`, binding is `types`.
            let local_name = import
                .module
                .split('.')
                .next_back()
                .unwrap_or(import.module.as_str());
            names.insert(local_name.to_owned());
        }
    }

    // Unannotated module-level variables with simple literal RHS are not types.
    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let is_simple_literal = matches!(
            var.rhs_kind,
            RhsKind::IntLiteral
                | RhsKind::FloatLiteral
                | RhsKind::StrLiteral
                | RhsKind::BoolLiteral
                | RhsKind::BytesLiteral
                | RhsKind::EmptyList
                | RhsKind::EmptyDict
                | RhsKind::NoneValue
        );
        if is_simple_literal {
            names.insert(var.name.clone());
        }
    }

    names
}

/// Returns `true` when the annotation text is exactly a name bound to a non-type in module scope.
fn is_non_type_name(ann: &str, non_type_names: &HashSet<String>) -> bool {
    let ann = ann.trim();
    
    // Handle string literal annotations (forward references)
    // If the annotation is a string literal, check the content inside
    let content_to_check = if (ann.starts_with('"') && ann.ends_with('"'))
        || (ann.starts_with('\'') && ann.ends_with('\''))
    {
        &ann[1..ann.len() - 1]
    } else {
        ann
    };

    // Only match bare identifiers — no subscripts, dot access, or call expressions.
    if content_to_check.contains('[') || content_to_check.contains('.') || content_to_check.contains('(') || content_to_check.contains(' ') {
        return false;
    }
    non_type_names.contains(content_to_check)
}

/// Emits BSK-E0047 when an annotation contains an invalid type expression.
pub(crate) struct InvalidTypeAnnotation;

impl Rule for InvalidTypeAnnotation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;
        let non_type_names = collect_non_type_names(module);

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
                let ann_trimmed = ann.trim();
                if is_invalid_type_annotation(ann_trimmed)
                    || is_non_type_name(ann_trimmed, &non_type_names)
                {
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
            let ann_trimmed = ann.trim();
            if is_invalid_type_annotation(ann_trimmed)
                || is_non_type_name(ann_trimmed, &non_type_names)
            {
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
                let ann_trimmed = ann.trim();
                if is_invalid_type_annotation(ann_trimmed)
                    || is_non_type_name(ann_trimmed, &non_type_names)
                {
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
