//! BSK-E0120: Generator return type and yield type violations.
//!
//! A generator function (one containing `yield` or `yield from`) must declare
//! a return type compatible with generator protocols:
//!
//! - Sync generators: `Generator`, `Iterator`, or `Iterable`
//! - Async generators: `AsyncGenerator`, `AsyncIterator`, or `AsyncIterable`
//!
//! Additionally, yield expressions must produce values assignable to the
//! declared yield type, and `yield from` sub-generators must have compatible
//! yield and send types.
//!
//! ```python
//! from typing import Generator, Iterator
//!
//! # BAD -- generator with non-generator return type
//! def bad() -> int:
//!     yield 1
//!
//! # GOOD
//! def good() -> Iterator[int]:
//!     yield 1
//! ```

use basilisk_resolver::scope::GeneratorViolationKind;
use basilisk_resolver::{FunctionInfo, ResolvedModule, RhsKind};

use super::Rule;
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::inference::infer_rhs;
use crate::types::InferredType;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0120",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0120",
};

/// Valid return type base names for synchronous generator functions.
const SYNC_GENERATOR_TYPES: &[&str] = &["Generator", "Iterator", "Iterable"];

/// Valid return type base names for asynchronous generator functions.
const ASYNC_GENERATOR_TYPES: &[&str] = &["AsyncGenerator", "AsyncIterator", "AsyncIterable"];

/// Emits BSK-E0120 for generator return type and yield type violations.
pub(crate) struct GeneratorReturnTypeViolation;

impl Rule for GeneratorReturnTypeViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Emit violations collected by the resolver (invalid return types).
        for violation in &module.generator_violations {
            let (message, help, note) = match &violation.kind {
                GeneratorViolationKind::InvalidReturnType {
                    func_name,
                    return_type,
                } => (
                    format!(
                        "Generator function `{func_name}` has incompatible return type \
                         `{return_type}`"
                    ),
                    Some(
                        "Use `Generator[YieldType, SendType, ReturnType]`, \
                         `Iterator[YieldType]`, or `Iterable[YieldType]` as the return type"
                            .to_owned(),
                    ),
                    Some(
                        "A function containing `yield` is a generator and must have a \
                         generator-compatible return type annotation"
                            .to_owned(),
                    ),
                ),
                GeneratorViolationKind::YieldTypeMismatch { expected, actual } => (
                    format!(
                        "Incompatible yield type: `{actual}` is not assignable to `{expected}`"
                    ),
                    Some(format!(
                        "Change the yielded value to match `{expected}`, or update the \
                         generator annotation"
                    )),
                    None,
                ),
                GeneratorViolationKind::YieldFromTypeMismatch { expected, actual } => (
                    format!(
                        "Incompatible `yield from` type: yields `{actual}` but expected \
                         `{expected}`"
                    ),
                    Some(
                        "Ensure the sub-iterator yields values compatible with the outer \
                         generator's yield type"
                            .to_owned(),
                    ),
                    None,
                ),
                GeneratorViolationKind::YieldFromSendTypeMismatch { expected, actual } => (
                    format!(
                        "Incompatible `yield from` send type: sub-generator accepts `{actual}` \
                         but outer generator sends `{expected}`"
                    ),
                    Some(
                        "Ensure the sub-generator's send type is compatible with the outer \
                         generator's send type"
                            .to_owned(),
                    ),
                    None,
                ),
            };

            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message,
                span: violation.span,
                path: module.path.clone(),
                help,
                note,
            });
        }

        // Check yield type mismatches for generator functions with valid return types.
        for func in &module.functions {
            if !func.is_generator || func.yield_exprs.is_empty() {
                continue;
            }
            check_yield_types(func, module, diagnostics);
            check_return_in_generator(func, module, diagnostics);
        }
    }
}

/// Check yield expression types against the declared yield type parameter.
fn check_yield_types(func: &FunctionInfo, module: &ResolvedModule, out: &mut Vec<Diagnostic>) {
    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = module
        .source
        .get(ann_span.start as usize..ann_span.end as usize)
    else {
        return;
    };

    let base = base_type_name(ann_text);
    let valid_types = if func.is_async {
        ASYNC_GENERATOR_TYPES
    } else {
        SYNC_GENERATOR_TYPES
    };

    // Only check yield types if this is a valid generator type.
    if !valid_types.iter().any(|t| base.eq_ignore_ascii_case(t)) {
        return;
    }

    // Extract the yield type parameter from the annotation.
    let Some(yield_type_str) = extract_yield_type(ann_text, base) else {
        return;
    };

    let declared_yield_type = InferredType::from_annotation(&yield_type_str);

    // Skip if the declared yield type is Unknown/Any - can't check.
    if matches!(
        declared_yield_type,
        InferredType::Unknown | InferredType::Any
    ) {
        return;
    }

    for yield_expr in &func.yield_exprs {
        if yield_expr.is_yield_from {
            check_yield_from(
                func,
                yield_expr,
                &declared_yield_type,
                &yield_type_str,
                ann_text,
                base,
                module,
                out,
            );
            continue;
        }

        let inferred = infer_yield_type(&yield_expr.rhs_kind, yield_expr.call_name.as_ref());

        // Skip Unknown types - we can't prove incompatibility.
        if matches!(inferred, InferredType::Unknown) {
            continue;
        }

        if !inferred.is_assignable_to(&declared_yield_type) {
            out.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Incompatible yield type in `{}`: `{inferred}` is not assignable to \
                     `{declared_yield_type}`",
                    func.name
                ),
                span: yield_expr.span,
                path: module.path.clone(),
                help: Some(format!(
                    "Change the yielded value to match `{declared_yield_type}`, or update \
                     the generator annotation"
                )),
                note: None,
            });
        }
    }
}

/// Check return statements in generator functions against the `ReturnType` parameter.
fn check_return_in_generator(
    func: &FunctionInfo,
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    let Some(ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(ann_text) = module
        .source
        .get(ann_span.start as usize..ann_span.end as usize)
    else {
        return;
    };

    let base = base_type_name(ann_text);

    // Only Generator[Y, S, R] has a return type parameter.
    // Iterator and Iterable don't have a return type param.
    if !base.eq_ignore_ascii_case("Generator") {
        return;
    }

    let Some(return_type_str) = extract_return_type_from_generator(ann_text) else {
        return;
    };

    let declared_return_type = InferredType::from_annotation(&return_type_str);

    if matches!(
        declared_return_type,
        InferredType::Unknown | InferredType::Any
    ) {
        return;
    }

    // If R is not None, the function must have an unconditional return on all paths.
    // When the last top-level statement is NOT a return (and not a raise/noreturn call),
    // the function can fall through without returning R → "missing return".
    if !matches!(declared_return_type, InferredType::None_)
        && !func.body_ends_with_return
        && !func.body_last_stmt_terminates
    {
        out.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Generator function `{}` is missing a return statement; declared return \
                 type is `{declared_return_type}`",
                func.name
            ),
            span: func.def_span,
            path: module.path.clone(),
            help: Some(format!(
                "Add a `return` statement that produces a value of type `{declared_return_type}` \
                 on all code paths"
            )),
            note: Some(
                "In `Generator[Y, S, R]`, the third parameter `R` is the return type; \
                 the function must return `R` on every code path"
                    .to_owned(),
            ),
        });
    }

    for ret_stmt in &func.return_stmts {
        if !ret_stmt.has_value {
            continue;
        }
        // Skip call expressions - can't prove type.
        if ret_stmt.value_is_call {
            continue;
        }

        let inferred = infer_rhs(&ret_stmt.rhs_kind);

        if matches!(inferred, InferredType::Unknown) {
            continue;
        }

        if !inferred.is_assignable_to(&declared_return_type) {
            out.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Incompatible return type in generator `{}`: `{inferred}` is not assignable \
                     to `{declared_return_type}`",
                    func.name
                ),
                span: ret_stmt.span,
                path: module.path.clone(),
                help: Some(format!(
                    "Change the return value to match `{declared_return_type}`, or update \
                     the Generator return type parameter"
                )),
                note: Some(
                    "In `Generator[Y, S, R]`, the third parameter `R` is the return type"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Check a `yield from expr` against the outer generator's declared yield type.
#[allow(clippy::too_many_arguments)]
fn check_yield_from(
    func: &FunctionInfo,
    yield_expr: &basilisk_resolver::YieldExprInfo,
    declared_yield_type: &InferredType,
    _yield_type_str: &str,
    outer_ann: &str,
    outer_base: &str,
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    match &yield_expr.rhs_kind {
        RhsKind::List(elements) => {
            check_yield_from_list(func, yield_expr, declared_yield_type, elements, module, out);
        }
        RhsKind::CallExpr => {
            check_yield_from_call(
                func,
                yield_expr,
                declared_yield_type,
                outer_ann,
                outer_base,
                module,
                out,
            );
        }
        _ => {}
    }
}

/// Check `yield from [literal_list]` against the declared yield type.
fn check_yield_from_list(
    func: &FunctionInfo,
    yield_expr: &basilisk_resolver::YieldExprInfo,
    declared_yield_type: &InferredType,
    elements: &[RhsKind],
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    for elem_rhs in elements {
        let elem_type = infer_rhs(elem_rhs);
        if matches!(elem_type, InferredType::Unknown) {
            continue;
        }
        if !elem_type.is_assignable_to(declared_yield_type) {
            out.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Incompatible `yield from` in `{}`: list element type `{elem_type}` \
                     is not assignable to yield type `{declared_yield_type}`",
                    func.name
                ),
                span: yield_expr.span,
                path: module.path.clone(),
                help: Some(
                    "Ensure the sub-iterator yields values compatible with the outer \
                     generator's yield type"
                        .to_owned(),
                ),
                note: None,
            });
            return; // One diagnostic per yield-from is enough.
        }
    }
}

/// Check `yield from callee()` against the declared yield and send types.
#[allow(clippy::too_many_arguments)]
fn check_yield_from_call(
    func: &FunctionInfo,
    yield_expr: &basilisk_resolver::YieldExprInfo,
    declared_yield_type: &InferredType,
    outer_ann: &str,
    outer_base: &str,
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    let Some(callee_name) = &yield_expr.call_name else {
        return;
    };
    let Some(callee_func) = module.functions.iter().find(|f| f.name == *callee_name) else {
        return;
    };
    let Some(callee_ann_span) = callee_func.return_annotation_span else {
        return;
    };
    let Some(callee_ann) = module
        .source
        .get(callee_ann_span.start as usize..callee_ann_span.end as usize)
    else {
        return;
    };

    let callee_base = base_type_name(callee_ann);
    let callee_yield_type_str = extract_yield_type(callee_ann, callee_base).unwrap_or_default();
    if callee_yield_type_str.is_empty() {
        return;
    }
    let callee_yield_type = InferredType::from_annotation(&callee_yield_type_str);
    if matches!(callee_yield_type, InferredType::Unknown) {
        return;
    }

    // Check yield type compatibility.
    if !callee_yield_type.is_assignable_to(declared_yield_type) {
        out.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Incompatible `yield from` in `{}`: sub-generator yields \
                 `{callee_yield_type}` but `{declared_yield_type}` is expected",
                func.name
            ),
            span: yield_expr.span,
            path: module.path.clone(),
            help: Some(
                "Ensure the sub-iterator yields values compatible with the outer \
                 generator's yield type"
                    .to_owned(),
            ),
            note: None,
        });
    }

    // Check send type compatibility between Generator types.
    check_send_type_compat(func, yield_expr, outer_ann, outer_base, callee_ann, callee_base, module, out);
}

/// Check send type compatibility for `yield from` between two `Generator` types.
#[allow(clippy::too_many_arguments)]
fn check_send_type_compat(
    func: &FunctionInfo,
    yield_expr: &basilisk_resolver::YieldExprInfo,
    outer_ann: &str,
    outer_base: &str,
    callee_ann: &str,
    callee_base: &str,
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    if !outer_base.eq_ignore_ascii_case("Generator")
        || !callee_base.eq_ignore_ascii_case("Generator")
    {
        return;
    }

    let outer_inner = outer_ann
        .get(outer_ann.find('[').unwrap_or(0) + 1..outer_ann.len() - 1)
        .unwrap_or("");
    let callee_inner = callee_ann
        .get(callee_ann.find('[').unwrap_or(0) + 1..callee_ann.len() - 1)
        .unwrap_or("");

    let outer_args = split_top_level_comma(outer_inner);
    let callee_args = split_top_level_comma(callee_inner);

    if outer_args.len() < 2 || callee_args.len() < 2 {
        return;
    }

    let outer_send = InferredType::from_annotation(outer_args[1].trim());
    let callee_send = InferredType::from_annotation(callee_args[1].trim());

    if matches!(outer_send, InferredType::Unknown | InferredType::Any)
        || matches!(callee_send, InferredType::Unknown | InferredType::Any)
    {
        return;
    }

    if !outer_send.is_assignable_to(&callee_send) {
        out.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Incompatible `yield from` send type in `{}`: outer sends \
                 `{outer_send}` but sub-generator accepts `{callee_send}`",
                func.name
            ),
            span: yield_expr.span,
            path: module.path.clone(),
            help: Some(
                "Ensure the sub-generator's send type is compatible with the \
                 outer generator's send type"
                    .to_owned(),
            ),
            note: None,
        });
    }
}

/// Infer the type of a yield expression value.
fn infer_yield_type(rhs: &RhsKind, call_name: Option<&String>) -> InferredType {
    // If it's a call expression with a known name, use that as a Named type.
    if matches!(rhs, RhsKind::CallExpr) {
        if let Some(name) = call_name {
            return InferredType::Named(name.to_ascii_lowercase());
        }
        return InferredType::Unknown;
    }
    infer_rhs(rhs)
}

/// Extract the base type name (before `[`).
fn base_type_name(annotation: &str) -> &str {
    annotation
        .find('[')
        .map_or(annotation, |idx| &annotation[..idx])
        .trim()
}

/// Extract the yield type parameter from a generator annotation.
///
/// - `Generator[A, B, C]` -> `Some("A")`
/// - `Iterator[A]` -> `Some("A")`
/// - `Iterable[A]` -> `Some("A")`
/// - `AsyncGenerator[A, B]` -> `Some("A")`
/// - `AsyncIterator[A]` -> `Some("A")`
/// - `AsyncIterable[A]` -> `Some("A")`
fn extract_yield_type(annotation: &str, base: &str) -> Option<String> {
    let bracket_pos = annotation.find('[')?;
    let inner = annotation.get(bracket_pos + 1..annotation.len() - 1)?;

    match base {
        "Generator" | "AsyncGenerator" => {
            // First type parameter is the yield type.
            let first_arg = split_top_level_comma(inner).into_iter().next()?;
            Some(first_arg.trim().to_owned())
        }
        "Iterator" | "Iterable" | "AsyncIterator" | "AsyncIterable" => {
            // Single type parameter is the yield type.
            Some(inner.trim().to_owned())
        }
        _ => None,
    }
}

/// Extract the return type (3rd parameter) from `Generator[Y, S, R]`.
fn extract_return_type_from_generator(annotation: &str) -> Option<String> {
    let bracket_pos = annotation.find('[')?;
    let inner = annotation.get(bracket_pos + 1..annotation.len() - 1)?;
    let args = split_top_level_comma(inner);
    if args.len() >= 3 {
        Some(args[2].trim().to_owned())
    } else {
        None
    }
}

/// Split a string by top-level commas (respecting bracket nesting).
fn split_top_level_comma(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut start = 0;

    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&inner[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    let remainder = &inner[start..];
    if !remainder.trim().is_empty() {
        parts.push(remainder);
    }
    parts
}
