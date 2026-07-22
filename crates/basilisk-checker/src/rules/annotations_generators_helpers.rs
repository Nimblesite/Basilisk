//! Implements [`annotations_generators`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Helper functions for `annotations_generators`: Generator return type and yield type
//! violations.
//!
//! This module provides type extraction and yield-from checking utilities used
//! by the rule to validate generator function annotations.

use basilisk_resolver::{FunctionInfo, ResolvedModule, RhsKind, YieldExprInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::inference::infer_rhs;
use crate::rules::shared::split_top_level_commas;
use crate::span_util::slice_span;
use crate::types::InferredType;

/// `annotations_generators` error code shared between this module and the rule.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "annotations_generators",
    docs_url: "https://www.basilisk-python.dev/errors/annotations_generators",
};

/// Valid return type base names for synchronous generator functions.
pub(super) const SYNC_GENERATOR_TYPES: &[&str] = &["Generator", "Iterator", "Iterable"];

/// Valid return type base names for asynchronous generator functions.
pub(super) const ASYNC_GENERATOR_TYPES: &[&str] =
    &["AsyncGenerator", "AsyncIterator", "AsyncIterable"];

/// Infer the type of a yield expression value.
pub(super) fn infer_yield_type(
    rhs: &RhsKind,
    call_name: Option<&String>,
    module: &ResolvedModule,
) -> InferredType {
    if matches!(rhs, RhsKind::CallExpr) {
        return call_name.map_or(InferredType::Unknown, |name| {
            infer_call_result(name, module)
        });
    }
    infer_rhs(rhs)
}

/// The result type of a direct call `name(...)`: a module-level function's
/// declared return type, a local class's instance, or a builtin constructor
/// (`str(...)`, `int(...)`). `Unknown` for anything unresolvable — a callee's
/// bare name is never itself the yielded type (GitHub #281 inferred `get` as
/// a type from `yield NAME_SYNONYMS.get(name, name)`).
fn infer_call_result(name: &str, module: &ResolvedModule) -> InferredType {
    // A module-level (or nested) function: its declared return type is the
    // call's type. Methods are excluded — a direct call never targets one.
    if let Some(callee) = module
        .functions
        .iter()
        .find(|f| f.name == name && f.class_name.is_none())
    {
        return callee
            .return_annotation_span
            .and_then(|span| slice_span(&module.source, span))
            .map_or(InferredType::Unknown, |ann| {
                InferredType::from_annotation(ann.trim())
            });
    }
    // A locally-defined class: constructing it yields an instance of it.
    if module.classes.iter().any(|c| c.name == name) {
        return InferredType::from_annotation(name);
    }
    // A builtin constructor parses to a concrete type; any other bare name
    // parses to `Named(...)`, which is not evidence of the call's type.
    match InferredType::from_annotation(name) {
        InferredType::Named(_) => InferredType::Unknown,
        builtin => builtin,
    }
}

/// Extract the base type name (before `[`).
pub(super) fn base_type_name(annotation: &str) -> &str {
    annotation
        .find('[')
        .and_then(|idx| annotation.get(..idx))
        .unwrap_or(annotation)
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
pub(super) fn extract_yield_type(annotation: &str, base: &str) -> Option<String> {
    let bracket_pos = annotation.find('[')?;
    let inner = annotation.get(bracket_pos + 1..annotation.len().checked_sub(1)?)?;

    match base {
        "Generator" | "AsyncGenerator" => {
            // First type parameter is the yield type.
            let first_arg = split_top_level_commas(inner).into_iter().next()?;
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
pub(super) fn extract_return_type_from_generator(annotation: &str) -> Option<String> {
    let bracket_pos = annotation.find('[')?;
    let inner = annotation.get(bracket_pos + 1..annotation.len().checked_sub(1)?)?;
    let args = split_top_level_commas(inner);
    args.get(2).map(|arg| arg.trim().to_owned())
}

/// Check a `yield from expr` against the outer generator's declared yield type.
pub(super) fn check_yield_from(
    func: &FunctionInfo,
    yield_expr: &YieldExprInfo,
    declared_yield_type: &InferredType,
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
    yield_expr: &YieldExprInfo,
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
            out.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Incompatible `yield from` in `{}`: list element type `{elem_type}` \
                     is not assignable to yield type `{declared_yield_type}`",
                    func.name
                ),
                yield_expr.span,
                &module.path,
                Some(
                    "Ensure the sub-iterator yields values compatible with the outer \
                     generator's yield type"
                        .to_owned(),
                ),
                None,
            ));
            return; // One diagnostic per yield-from is enough.
        }
    }
}

/// Check `yield from callee()` against the declared yield and send types.
fn check_yield_from_call(
    func: &FunctionInfo,
    yield_expr: &YieldExprInfo,
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
    let Some(callee_ann) = slice_span(&module.source, callee_ann_span) else {
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
        out.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Incompatible `yield from` in `{}`: sub-generator yields \
                 `{callee_yield_type}` but `{declared_yield_type}` is expected",
                func.name
            ),
            yield_expr.span,
            &module.path,
            Some(
                "Ensure the sub-iterator yields values compatible with the outer \
                 generator's yield type"
                    .to_owned(),
            ),
            None,
        ));
    }

    // Check send type compatibility between Generator types.
    check_send_type_compat(
        func,
        yield_expr,
        outer_ann,
        outer_base,
        callee_ann,
        callee_base,
        module,
        out,
    );
}

/// Check send type compatibility for `yield from` between two `Generator` types.
#[expect(
    clippy::too_many_arguments,
    reason = "send type check requires both generator annotations"
)]
pub(super) fn check_send_type_compat(
    func: &FunctionInfo,
    yield_expr: &YieldExprInfo,
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

    let outer_bracket = outer_ann.find('[').unwrap_or(0);
    let outer_inner = outer_ann
        .get(outer_bracket + 1..outer_ann.len().saturating_sub(1))
        .unwrap_or_default();
    let callee_bracket = callee_ann.find('[').unwrap_or(0);
    let callee_inner = callee_ann
        .get(callee_bracket + 1..callee_ann.len().saturating_sub(1))
        .unwrap_or_default();

    let outer_args = split_top_level_commas(outer_inner);
    let callee_args = split_top_level_commas(callee_inner);

    let Some(outer_send_str) = outer_args.get(1) else {
        return;
    };
    let Some(callee_send_str) = callee_args.get(1) else {
        return;
    };

    let outer_send = InferredType::from_annotation(outer_send_str.trim());
    let callee_send = InferredType::from_annotation(callee_send_str.trim());

    if matches!(outer_send, InferredType::Unknown | InferredType::Any)
        || matches!(callee_send, InferredType::Unknown | InferredType::Any)
    {
        return;
    }

    if !outer_send.is_assignable_to(&callee_send) {
        out.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Incompatible `yield from` send type in `{}`: outer sends \
                 `{outer_send}` but sub-generator accepts `{callee_send}`",
                func.name
            ),
            yield_expr.span,
            &module.path,
            Some(
                "Ensure the sub-generator's send type is compatible with the \
                 outer generator's send type"
                    .to_owned(),
            ),
            None,
        ));
    }
}
