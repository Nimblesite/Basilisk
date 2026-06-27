//! Implements [BSK-E0131] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Type checking helpers for BSK-E0131.
//!
//! Contains compatibility checks for yield/send/return types and the
//! diagnostic-emitting functions for individual yield expressions.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::{is_type_compatible, split_top_level_commas};

use super::annotation::{parse_generator_annotation, GeneratorAnnotation};
use super::yield_scan::YieldExpr;
use super::CODE;

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
pub(super) fn infer_expr_type(expr: &str) -> Option<&str> {
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
            && expr
                .get(1..)
                .unwrap_or("")
                .chars()
                .all(|c| c.is_ascii_digit()))
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
pub(super) fn get_constructor_name(expr: &str) -> Option<&str> {
    let expr = expr.trim();
    let paren_pos = expr.find('(')?;
    let name = expr.get(..paren_pos)?.trim();

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

/// Extract the function name from a call expression like `generator17()`.
pub(super) fn extract_call_name(expr: &str) -> Option<&str> {
    let expr = expr.trim();
    let paren_pos = expr.find('(')?;
    let name = expr.get(..paren_pos)?.trim();

    if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Some(name)
    } else {
        None
    }
}

/// Infer the element type of a list literal like `[1]`, `[1, 2, 3]`.
pub(super) fn infer_list_element_type(expr: &str) -> Option<&str> {
    let expr = expr.trim();
    if !expr.starts_with('[') || !expr.ends_with(']') {
        return None;
    }
    let inner = expr.get(1..expr.len().saturating_sub(1))?.trim();
    if inner.is_empty() {
        return None;
    }

    let first_elem = split_top_level_commas(inner);
    infer_expr_type(first_elem.first()?.trim())
}

/// Check a single `yield` expression for type compatibility with the generator annotation.
pub(super) fn check_yield_value(
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
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Incompatible yield type in `{}`: expected `{expected}`, got `{expr_text}`",
                func.name
            ),
            Span {
                start: yield_expr.offset,
                end: yield_expr.offset + 5,
            },
            path,
            Some(format!(
                "The generator `{}` is annotated to yield `{expected}`",
                func.name
            )),
            Some(
                "The yield expression must produce a value compatible with the declared yield type"
                    .to_owned(),
            ),
        ));
    }
}

/// Check a `yield from` expression for type compatibility with the generator annotation.
pub(super) fn check_yield_from(
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
                    diagnostics.push(error_diagnostic_owned(
                        CODE.clone(),
                        format!(
                            "Incompatible `yield from` in `{}`: `{callee_name}` yields `{}` but `{}` expected",
                            func.name, callee_gen.yield_type, gen_ann.yield_type
                        ),
                        yield_from_span,
                        path,
                        Some(format!(
                            "The generator `{}` expects yield type `{}`",
                            func.name, gen_ann.yield_type
                        )),
                        Some(
                            "The delegated generator must yield values compatible with the outer generator's yield type"
                                .to_owned(),
                        ),
                    ));
                }

                // Check send type compatibility
                if let (Some(outer_send), Some(inner_send)) =
                    (&gen_ann.send_type, &callee_gen.send_type)
                {
                    if !is_send_type_compatible(outer_send, inner_send) {
                        diagnostics.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!(
                                "Incompatible send type in `yield from` in `{}`: \
                                 `{callee_name}` expects send type `{inner_send}` but outer generator sends `{outer_send}`",
                                func.name
                            ),
                            yield_from_span,
                            path,
                            Some(format!(
                                "The generator `{}` sends `{outer_send}` which is not compatible with `{callee_name}`'s send type `{inner_send}`",
                                func.name
                            )),
                            Some(
                                "When using `yield from`, the outer generator's send type must be \
                                 compatible with the inner generator's send type"
                                    .to_owned(),
                            ),
                        ));
                    }
                }
                return;
            }
        }
    }

    // Case 2: yield from [literal_list]
    if let Some(elem_type) = infer_list_element_type(expr_text) {
        if !is_type_compatible(elem_type, &gen_ann.yield_type) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Incompatible `yield from` in `{}`: list elements are `{elem_type}` but `{}` expected",
                    func.name, gen_ann.yield_type
                ),
                yield_from_span,
                path,
                Some(format!(
                    "The generator `{}` expects yield type `{}`",
                    func.name, gen_ann.yield_type
                )),
                Some(
                    "The iterable in `yield from` must produce values compatible with the generator's yield type"
                        .to_owned(),
                ),
            ));
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
pub(super) fn check_missing_generator_return(
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
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Generator `{}` is annotated to return `{return_type}` but has no valid return path",
                func.name
            ),
            func.def_span,
            path,
            Some(format!(
                "Add a `return {return_type}(...)` statement or change the return type to `None`"
            )),
            Some(
                "A Generator[Y, S, R] function must have a `return` statement \
                 producing a value of type R"
                    .to_owned(),
            ),
        ));
    }
}
