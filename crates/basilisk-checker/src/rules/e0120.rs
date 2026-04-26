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
use basilisk_resolver::{FunctionInfo, ResolvedModule};

use super::e0120_helpers::{
    base_type_name, check_yield_from, extract_return_type_from_generator, extract_yield_type,
    infer_yield_type, ASYNC_GENERATOR_TYPES, CODE, SYNC_GENERATOR_TYPES,
};
use super::Rule;
use crate::diagnostic::{Diagnostic, Severity};
use crate::inference::infer_rhs;
use crate::span_util::slice_span;
use crate::types::InferredType;

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
                provenance: None,
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
    let Some(ann_text) = slice_span(&module.source, ann_span) else {
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
                provenance: None,
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
    let Some(ann_text) = slice_span(&module.source, ann_span) else {
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
            provenance: None,
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
                provenance: None,
            });
        }
    }
}
