//! BSK-E0045: Invalid first argument to `Annotated[...]`.
//!
//! PEP 593 requires that the first argument to `Annotated[...]` be a valid type
//! expression. The following are errors:
//!
//! - List literals: `Annotated[[int, str], ""]`
//! - Tuple literals: `Annotated[((int, str),), ""]`
//! - Dict literals: `Annotated[{"a": "b"}, ""]`
//! - List comprehensions: `Annotated[[x for x in ...], ""]`
//! - Lambda calls: `Annotated[(lambda: int)(), ""]`
//! - Conditional expressions: `Annotated[int if cond else str, ""]`
//! - Boolean literals: `Annotated[True, ""]`
//! - Integer literals: `Annotated[1, ""]`
//! - Binary boolean operators: `Annotated[list or set, ""]`
//! - F-strings: `Annotated[f"...", ""]`
//! - Subscript-into-subscript: `Annotated[[int][0], ""]`
//!
//! Additionally, `Annotated[int]` with fewer than 2 arguments is an error.
//!
//! ```python
//! Bad1: Annotated[[int, str], ""]   # E — list literal not valid type
//! Bad9: Annotated[True, ""]          # E — bool literal not valid type
//! Bad13: Annotated[int]              # E — requires at least two arguments
//! ```

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0045",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0045",
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
        help: Some("The first argument to `Annotated[...]` must be a valid type expression".to_owned()),
        note: Some("PEP 593: `Annotated[T, metadata...]` requires T to be a type, not a literal or expression".to_owned()),
    }
}

/// Extract the inner content of `Annotated[...]`.
///
/// Returns `None` if the annotation does not start with `Annotated[`.
fn annotated_inner(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    if !ann.starts_with("Annotated[") {
        return None;
    }
    let inner_start = "Annotated[".len();
    let inner_end = ann.rfind(']')?;
    if inner_end <= inner_start {
        return None;
    }
    Some(&ann[inner_start..inner_end])
}

/// Extract just the first argument from the inner content of `Annotated[T, ...]`.
///
/// Handles nested brackets correctly by tracking depth.
fn first_arg(inner: &str) -> &str {
    let mut depth = 0i32;
    let mut end = inner.len();
    for (i, ch) in inner.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => {
                depth -= 1;
            }
            ',' if depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    inner[..end].trim()
}

/// Count top-level arguments in `Annotated[...]` inner content.
fn count_args(inner: &str) -> usize {
    if inner.trim().is_empty() {
        return 0;
    }
    let mut depth = 0i32;
    let mut count = 1usize;
    for ch in inner.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

/// Returns `true` when the first-argument text is an invalid type expression.
fn is_invalid_type_expr(first: &str) -> bool {
    let first = first.trim();

    // Boolean literals: True, False
    if first == "True" || first == "False" {
        return true;
    }

    // Integer or float literals: starts with a digit
    if first.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }

    // Negative numeric literals: -1, -3.14
    if first.starts_with('-')
        && first[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }

    // F-string: starts with f" or f'
    if first.starts_with("f\"") || first.starts_with("f'") {
        return true;
    }

    // List literal: starts with [ but not [int][0] (subscript) — detect list literal by
    // checking if it opens with [ and contains elements that look like a list.
    // A list literal starts with `[` and the content is not a subscript operation.
    if first.starts_with('[') && !is_subscript_expression(first) {
        return true;
    }

    // Dict literal: starts with {
    if first.starts_with('{') {
        return true;
    }

    // Tuple literal: starts with ( and contains a trailing comma or is a tuple
    // e.g. `((int, str),)` — outer parens wrapping a tuple
    if first.starts_with('(') && is_tuple_literal(first) {
        return true;
    }

    // Conditional expression: `X if cond else Y` — detect `if` keyword at depth 0
    if has_top_level_if(first) {
        return true;
    }

    // Boolean binary operator `or` / `and` at depth 0 (not `|` which is valid union)
    if has_top_level_bool_op(first) {
        return true;
    }

    // Lambda call: `(lambda: ...)()`
    if first.contains("lambda") {
        return true;
    }

    // Subscript-into-subscript: `[int][0]` — list literal then subscript
    // Detected by starting with `[` and having `][` pattern
    if first.contains("][") {
        return true;
    }

    false
}

/// Returns `true` when the expression is a subscript like `list[int]` (NOT `[int][0]`).
fn is_subscript_expression(s: &str) -> bool {
    // A subscript expression starts with a name, not `[`
    s.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
}

/// Returns `true` when the expression looks like a tuple literal.
fn is_tuple_literal(s: &str) -> bool {
    // A tuple literal has a trailing comma before the closing paren,
    // or contains commas at depth 0 inside parens.
    if !s.starts_with('(') || !s.ends_with(')') {
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

/// Returns `true` when the expression has an `if` keyword at depth 0 — a conditional expr.
fn has_top_level_if(s: &str) -> bool {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'i' if depth == 0 => {
                // Check for ` if ` at this position
                if bytes.get(i..i + 4) == Some(b" if ")
                    || (i > 0 && bytes.get(i - 1..i + 3) == Some(b" if"))
                {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    // Simpler: just look for " if " in the string at depth 0
    // Use a char-by-char walk
    let mut depth2 = 0i32;
    let chars: Vec<char> = s.chars().collect();
    let mut j = 0;
    while j < chars.len() {
        match chars.get(j).copied() {
            Some('[' | '(' | '{') => depth2 += 1,
            Some(']' | ')' | '}') => depth2 -= 1,
            Some(_) if depth2 == 0 => {
                // Look for " if " starting at j
                let rest: String = chars[j..].iter().collect();
                if rest.starts_with(" if ") {
                    return true;
                }
            }
            _ => {}
        }
        j += 1;
    }
    false
}

/// Returns `true` when the expression has `or` or `and` at depth 0 — boolean binary op.
fn has_top_level_bool_op(s: &str) -> bool {
    let mut depth = 0i32;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars.get(i).copied() {
            Some('[' | '(' | '{') => depth += 1,
            Some(']' | ')' | '}') => depth -= 1,
            Some(_) if depth == 0 => {
                let rest: String = chars[i..].iter().collect();
                if rest.starts_with(" or ") || rest.starts_with(" and ") {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Emits BSK-E0045 when `Annotated[...]` has an invalid first argument or too few args.
pub(crate) struct AnnotatedInvalidFirstArg;

impl Rule for AnnotatedInvalidFirstArg {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        check_annotated_in_vars(&module.module_vars, source, path, diagnostics);

        for cls in &module.classes {
            check_annotated_in_attrs(&cls.attributes, source, path, diagnostics);
        }

        check_annotated_in_functions(&module.functions, source, path, diagnostics);
    }
}

fn check_annotated_in_vars(
    vars: &[basilisk_resolver::VariableInfo],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in vars {
        let Some(ann) = span_text(source, var.annotation_span) else {
            continue;
        };
        check_annotated_annotation(ann.trim(), var.name_span, &var.name, path, diagnostics);
    }
}

fn check_annotated_in_attrs(
    attrs: &[basilisk_resolver::AttributeInfo],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for attr in attrs {
        let Some(ann) = span_text(source, attr.annotation_span) else {
            continue;
        };
        check_annotated_annotation(ann.trim(), attr.name_span, &attr.name, path, diagnostics);
    }
}

fn check_annotated_in_functions(
    funcs: &[basilisk_resolver::FunctionInfo],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for func in funcs {
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            let Some(ann) = span_text(source, param.annotation_span) else {
                continue;
            };
            check_annotated_annotation(ann.trim(), param.name_span, &param.name, path, diagnostics);
        }
    }
}

fn check_annotated_annotation(
    ann: &str,
    span: Span,
    name: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(inner) = annotated_inner(ann) else {
        return;
    };

    let arg_count = count_args(inner);

    // Annotated[int] — too few arguments
    if arg_count < 2 {
        diagnostics.push(make_diagnostic(
            format!("`Annotated` requires at least two arguments for `{name}`"),
            span,
            path,
        ));
        return;
    }

    // Check that the first argument is a valid type expression
    let first = first_arg(inner);
    if is_invalid_type_expr(first) {
        diagnostics.push(make_diagnostic(
            format!("Invalid type expression as first argument to `Annotated` for `{name}`"),
            span,
            path,
        ));
    }
}
