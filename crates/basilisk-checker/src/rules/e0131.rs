//! BSK-E0131: Generator yield/send/return type mismatch.
//!
//! When a function is annotated with `Generator[Y, S, R]`, `Iterator[Y]`,
//! or `Iterable[Y]`, the yield expressions must produce values compatible
//! with `Y`, and `yield from` expressions must delegate to generators whose
//! yield and send types are compatible.
//!
//! ```python
//! from typing import Generator, Iterator
//!
//! class A: ...
//! class B: ...
//!
//! def bad() -> Generator[A, None, None]:
//!     yield 3          # E: incompatible yield type
//!
//! def bad2() -> Iterator[A]:
//!     yield B()        # E: incompatible yield type
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, ReturnAnnotationKind, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0131",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0131",
};

/// Emits BSK-E0131 for generator yield/send/return type mismatches.
pub(crate) struct GeneratorTypeMismatch;

impl Rule for GeneratorTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a map of function name -> return annotation text for cross-referencing
        // yield-from targets.
        let func_return_annotations: HashMap<&str, &str> = module
            .functions
            .iter()
            .filter_map(|func| {
                let ann_span = func.return_annotation_span?;
                let ann_text = slice_span(&module.source, ann_span)?;
                Some((func.name.as_str(), ann_text.trim()))
            })
            .collect();

        for func in &module.functions {
            check_function(
                func,
                &module.source,
                &module.path,
                &func_return_annotations,
                diagnostics,
            );
        }
    }
}

/// Parsed generator return annotation.
#[expect(clippy::struct_field_names, reason = "field names intentionally mirror the type parameter names")]
struct GeneratorAnnotation {
    /// The yield type (first type parameter).
    yield_type: String,
    /// The send type (second type parameter), if present.
    send_type: Option<String>,
    /// The return type (third type parameter), if present.
    return_type: Option<String>,
}

/// Try to parse a return annotation as a generator-like type.
///
/// Recognizes: `Generator[Y, S, R]`, `Iterator[Y]`, `Iterable[Y]`.
fn parse_generator_annotation(ann: &str) -> Option<GeneratorAnnotation> {
    let ann = ann.trim();

    // Check for Generator[Y, S, R]
    if let Some(inner) = strip_generic_prefix(ann, "Generator") {
        let args = split_top_level_args(inner);
        if args.is_empty() {
            return None;
        }
        return Some(GeneratorAnnotation {
            yield_type: args[0].trim().to_owned(),
            send_type: args.get(1).map(|s| s.trim().to_owned()),
            return_type: args.get(2).map(|s| s.trim().to_owned()),
        });
    }

    // Check for Iterator[Y]
    if let Some(inner) = strip_generic_prefix(ann, "Iterator") {
        let args = split_top_level_args(inner);
        if args.is_empty() {
            return None;
        }
        return Some(GeneratorAnnotation {
            yield_type: args[0].trim().to_owned(),
            send_type: None,
            return_type: None,
        });
    }

    // Check for Iterable[Y]
    if let Some(inner) = strip_generic_prefix(ann, "Iterable") {
        let args = split_top_level_args(inner);
        if args.is_empty() {
            return None;
        }
        return Some(GeneratorAnnotation {
            yield_type: args[0].trim().to_owned(),
            send_type: None,
            return_type: None,
        });
    }

    None
}

/// Strip a generic prefix like `Generator[` and return the inner content (without trailing `]`).
fn strip_generic_prefix<'a>(ann: &'a str, prefix: &str) -> Option<&'a str> {
    let with_bracket = format!("{prefix}[");
    if !ann.starts_with(&with_bracket) {
        return None;
    }
    let inner_start = with_bracket.len();
    let inner_end = ann.rfind(']')?;
    if inner_end <= inner_start {
        return None;
    }
    Some(&ann[inner_start..inner_end])
}

/// Split comma-separated type arguments at the top level (respecting bracket nesting).
fn split_top_level_args(inner: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;

    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                args.push(&inner[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    args.push(&inner[start..]);
    args
}

/// A yield expression found in a function body.
struct YieldExpr {
    /// The byte offset of the `yield` keyword in the source.
    offset: u32,
    /// The text of the yielded expression (after `yield`).
    expr_text: String,
    /// Whether this is a `yield from` expression.
    is_yield_from: bool,
}

/// Find all yield expressions in a function body substring.
#[expect(clippy::cast_possible_truncation, reason = "byte offsets fit u32 for source files")]
fn find_yield_expressions(body: &str, body_offset: usize) -> Vec<YieldExpr> {
    let mut results = Vec::new();
    let bytes = body.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        // Skip string literals (single/double/triple quoted)
        if bytes[pos] == b'\'' || bytes[pos] == b'"' {
            pos = skip_string(body, pos);
            continue;
        }

        // Skip comments
        if bytes[pos] == b'#' {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }

        // Look for `yield` keyword
        if pos + 5 <= bytes.len() && &body[pos..pos + 5] == "yield" {
            // Make sure it's a standalone keyword (not part of a larger identifier)
            let before_ok = pos == 0 || !is_identifier_char(bytes[pos - 1]);
            let after_pos = pos + 5;

            if before_ok && after_pos <= bytes.len() {
                // Check for `yield from`
                let is_yield_from = after_pos + 5 <= bytes.len()
                    && &body[after_pos..after_pos + 5] == " from"
                    && (after_pos + 5 >= bytes.len() || !is_identifier_char(bytes[after_pos + 5]));

                if is_yield_from {
                    let expr_start = after_pos + 5;
                    let expr_text = extract_yield_expr(body, expr_start);
                    results.push(YieldExpr {
                        offset: (body_offset + pos) as u32,
                        expr_text,
                        is_yield_from: true,
                    });
                } else if after_pos < bytes.len()
                    && (bytes[after_pos] == b' ' || bytes[after_pos] == b'\n')
                    && !is_identifier_char(bytes[after_pos])
                {
                    let expr_text = extract_yield_expr(body, after_pos);
                    results.push(YieldExpr {
                        offset: (body_offset + pos) as u32,
                        expr_text,
                        is_yield_from: false,
                    });
                }
            }
        }

        pos += 1;
    }

    results
}

/// Extract the expression text after a yield keyword.
fn extract_yield_expr(body: &str, start: usize) -> String {
    let rest = body[start..].trim_start();
    // Find the end of the expression: newline, comment, or end of string
    let mut depth = 0i32;
    let mut end = rest.len();
    for (idx, ch) in rest.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    end = idx;
                    break;
                }
            }
            '#' | '\n' if depth == 0 => {
                end = idx;
                break;
            }
            _ => {}
        }
    }
    rest[..end].trim().to_owned()
}

fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn skip_string(body: &str, start: usize) -> usize {
    let bytes = body.as_bytes();
    let quote = bytes[start];

    // Check for triple quotes
    if start + 2 < bytes.len() && bytes[start + 1] == quote && bytes[start + 2] == quote {
        let mut pos = start + 3;
        while pos + 2 < bytes.len() {
            if bytes[pos] == quote && bytes[pos + 1] == quote && bytes[pos + 2] == quote {
                return pos + 3;
            }
            pos += 1;
        }
        return bytes.len();
    }

    // Single quoted string
    let mut pos = start + 1;
    while pos < bytes.len() {
        if bytes[pos] == b'\\' {
            pos += 2;
            continue;
        }
        if bytes[pos] == quote {
            return pos + 1;
        }
        if bytes[pos] == b'\n' {
            return pos;
        }
        pos += 1;
    }
    bytes.len()
}

/// Infer a simple type name from an expression text.
///
/// Returns the inferred type name for simple expressions:
/// - Integer literal (`3`, `-1`) -> `"int"`
/// - Float literal (`3.14`) -> `"float"`
/// - String literal (`"hello"`) -> `"str"`
/// - Boolean literal (`True`/`False`) -> `"bool"`
/// - `None` -> `"None"`
///
/// Returns `None` if the type cannot be inferred.
fn infer_expr_type(expr: &str) -> Option<&str> {
    let expr = expr.trim();

    if expr.is_empty() {
        return None;
    }

    if expr == "True" || expr == "False" {
        return Some("bool");
    }

    if expr == "None" {
        return Some("None");
    }

    // Integer literal
    if expr.chars().all(|c| c.is_ascii_digit())
        || (expr.starts_with('-')
            && expr.len() > 1
            && expr[1..].chars().all(|c| c.is_ascii_digit()))
    {
        return Some("int");
    }

    // Float literal
    if expr.contains('.')
        && expr
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        return Some("float");
    }

    // String literal
    if (expr.starts_with('"') && expr.ends_with('"'))
        || (expr.starts_with('\'') && expr.ends_with('\''))
    {
        return Some("str");
    }

    None
}

/// Get the constructor name from an expression like `ClassName(...)`.
fn get_constructor_name(expr: &str) -> Option<&str> {
    let expr = expr.trim();
    let paren_pos = expr.find('(')?;
    let name = expr[..paren_pos].trim();

    if name
        .chars()
        .next()
        .is_some_and(|c| c.is_uppercase() || c == '_')
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        Some(name)
    } else {
        None
    }
}

/// Check if a type name is compatible with an expected type.
///
/// This is a conservative check: it returns `true` (compatible) when
/// we cannot determine incompatibility.
fn is_type_compatible(actual: &str, expected: &str) -> bool {
    if expected == "Any" || actual == "Any" || expected == "object" || actual == expected {
        return true;
    }
    // int is compatible with float
    if expected == "float" && actual == "int" {
        return true;
    }
    // bool is compatible with int
    if expected == "int" && actual == "bool" {
        return true;
    }
    false
}

/// Extract the function name from a call expression like `generator17()`.
fn extract_call_name(expr: &str) -> Option<&str> {
    let expr = expr.trim();
    let paren_pos = expr.find('(')?;
    let name = expr[..paren_pos].trim();

    if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Some(name)
    } else {
        None
    }
}

/// Infer the element type of a list literal like `[1]`, `[1, 2, 3]`.
fn infer_list_element_type(expr: &str) -> Option<&str> {
    let expr = expr.trim();
    if !expr.starts_with('[') || !expr.ends_with(']') {
        return None;
    }
    let inner = expr[1..expr.len() - 1].trim();
    if inner.is_empty() {
        return None;
    }

    let first_elem = split_top_level_args(inner);
    if first_elem.is_empty() {
        return None;
    }
    infer_expr_type(first_elem[0].trim())
}

fn check_function(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    func_return_annotations: &HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if func.return_annotation == ReturnAnnotationKind::Missing {
        return;
    }

    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = slice_span(source, ann_span) else {
        return;
    };
    let ann_text = ann_text.trim();

    let Some(gen_ann) = parse_generator_annotation(ann_text) else {
        return;
    };

    // Find the function body.
    let def_start = func.def_span.start_usize();
    let Some(colon_rel) = source[def_start..].find(':') else {
        return;
    };
    let body_start = def_start + colon_rel + 1;

    let def_line_start = source[..def_start].rfind('\n').map_or(0, |idx| idx + 1);
    let def_indent = def_start - def_line_start;

    let body_end = find_body_end(source, body_start, def_indent);
    let body = &source[body_start..body_end];

    let yields = find_yield_expressions(body, body_start);

    for yield_expr in &yields {
        if yield_expr.is_yield_from {
            check_yield_from(
                yield_expr,
                &gen_ann,
                func,
                path,
                func_return_annotations,
                diagnostics,
            );
        } else {
            check_yield_value(yield_expr, &gen_ann, func, path, diagnostics);
        }
    }

    check_missing_generator_return(func, &gen_ann, &yields, path, diagnostics);
}

fn find_body_end(source: &str, body_start: usize, def_indent: usize) -> usize {
    let mut pos = body_start;
    let mut first_line = true;

    for line in source[body_start..].lines() {
        if first_line {
            first_line = false;
            pos += line.len() + 1;
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            pos += line.len() + 1;
            continue;
        }

        let line_indent = line.len() - trimmed.len();
        if line_indent <= def_indent
            && (trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("async def ")
                || trimmed.starts_with('@'))
        {
            return pos;
        }
        pos += line.len() + 1;
    }

    source.len()
}

fn check_yield_value(
    yield_expr: &YieldExpr,
    gen_ann: &GeneratorAnnotation,
    func: &FunctionInfo,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expected = &gen_ann.yield_type;
    let expr_text = &yield_expr.expr_text;

    if expected == "Any" {
        return;
    }
    if expected == "None" && expr_text.is_empty() {
        return;
    }

    let is_mismatch = if let Some(inferred) = infer_expr_type(expr_text) {
        !is_type_compatible(inferred, expected)
    } else if let Some(ctor_name) = get_constructor_name(expr_text) {
        !is_type_compatible(ctor_name, expected)
    } else {
        false
    };

    if is_mismatch {
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Incompatible yield type in `{}`: expected `{expected}`, got `{expr_text}`",
                func.name
            ),
            span: Span {
                start: yield_expr.offset,
                end: yield_expr.offset + 5,
            },
            path: path.to_owned(),
            help: Some(format!(
                "The generator `{}` is annotated to yield `{expected}`",
                func.name
            )),
            note: Some(
                "The yield expression must produce a value compatible with the declared yield type"
                    .to_owned(),
            ),
        });
    }
}

fn check_yield_from(
    yield_expr: &YieldExpr,
    gen_ann: &GeneratorAnnotation,
    func: &FunctionInfo,
    path: &str,
    func_return_annotations: &HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expr_text = &yield_expr.expr_text;
    let yield_from_span = Span {
        start: yield_expr.offset,
        end: yield_expr.offset + 10,
    };

    // Case 1: yield from function_call()
    if let Some(callee_name) = extract_call_name(expr_text) {
        if let Some(callee_ann) = func_return_annotations.get(callee_name) {
            if let Some(callee_gen) = parse_generator_annotation(callee_ann) {
                // Check yield type compatibility
                if !is_type_compatible(&callee_gen.yield_type, &gen_ann.yield_type) {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Incompatible `yield from` in `{}`: `{callee_name}` yields `{}` but `{}` expected",
                            func.name, callee_gen.yield_type, gen_ann.yield_type
                        ),
                        span: yield_from_span,
                        path: path.to_owned(),
                        help: Some(format!(
                            "The generator `{}` expects yield type `{}`",
                            func.name, gen_ann.yield_type
                        )),
                        note: Some(
                            "The delegated generator must yield values compatible with the outer generator's yield type"
                                .to_owned(),
                        ),
                    });
                }

                // Check send type compatibility
                if let (Some(outer_send), Some(inner_send)) =
                    (&gen_ann.send_type, &callee_gen.send_type)
                {
                    if !is_send_type_compatible(outer_send, inner_send) {
                        diagnostics.push(Diagnostic {
                            code: CODE.clone(),
                            severity: Severity::Error,
                            message: format!(
                                "Incompatible send type in `yield from` in `{}`: \
                                 `{callee_name}` expects send type `{inner_send}` but outer generator sends `{outer_send}`",
                                func.name
                            ),
                            span: yield_from_span,
                            path: path.to_owned(),
                            help: Some(format!(
                                "The generator `{}` sends `{outer_send}` which is not compatible with `{callee_name}`'s send type `{inner_send}`",
                                func.name
                            )),
                            note: Some(
                                "When using `yield from`, the outer generator's send type must be \
                                 compatible with the inner generator's send type"
                                    .to_owned(),
                            ),
                        });
                    }
                }
                return;
            }
        }
    }

    // Case 2: yield from [literal_list]
    if let Some(elem_type) = infer_list_element_type(expr_text) {
        if !is_type_compatible(elem_type, &gen_ann.yield_type) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Incompatible `yield from` in `{}`: list elements are `{elem_type}` but `{}` expected",
                    func.name, gen_ann.yield_type
                ),
                span: yield_from_span,
                path: path.to_owned(),
                help: Some(format!(
                    "The generator `{}` expects yield type `{}`",
                    func.name, gen_ann.yield_type
                )),
                note: Some(
                    "The iterable in `yield from` must produce values compatible with the generator's yield type"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Check send type compatibility.
///
/// For `yield from`, the outer generator's send type flows to the inner.
/// The outer's send type must be assignable to the inner's send type.
fn is_send_type_compatible(outer_send: &str, inner_send: &str) -> bool {
    if outer_send == inner_send {
        return true;
    }
    if inner_send == "None" || outer_send == "None" {
        return true;
    }
    // float accepts int
    if inner_send == "float" && (outer_send == "int" || outer_send == "float") {
        return true;
    }
    false
}

/// Check for missing return in generator with non-None return type.
///
/// If a function is annotated `Generator[Y, S, R]` where `R` is not `None` and not `Any`,
/// and the function has no return statements that could validly produce `R`, flag the def line.
fn check_missing_generator_return(
    func: &FunctionInfo,
    gen_ann: &GeneratorAnnotation,
    yields: &[YieldExpr],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if yields.is_empty() {
        return;
    }

    let Some(return_type) = &gen_ann.return_type else {
        return;
    };

    if return_type == "None" || return_type == "Any" {
        return;
    }

    // Check if any return statement could validly return the expected type.
    let has_valid_return = func.return_stmts.iter().any(|ret| {
        if !ret.has_value {
            return false;
        }
        // Without full type inference, conservatively assume call expressions
        // could return the right type.
        ret.value_is_call
    });

    if !has_valid_return && !func.return_stmts.is_empty() {
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Generator `{}` is annotated to return `{return_type}` but has no valid return path",
                func.name
            ),
            span: func.def_span,
            path: path.to_owned(),
            help: Some(format!(
                "Add a `return {return_type}(...)` statement or change the return type to `None`"
            )),
            note: Some(
                "A Generator[Y, S, R] function must have a `return` statement \
                 producing a value of type R"
                    .to_owned(),
            ),
        });
    }
}
