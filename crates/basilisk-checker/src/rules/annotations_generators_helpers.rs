//! Implements [`annotations_generators`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Helper functions for `annotations_generators`: Generator return type and yield type
//! violations.
//!
//! This module provides type extraction and yield-from checking utilities used
//! by the rule to validate generator function annotations.

use basilisk_resolver::{FunctionInfo, ResolvedModule, RhsKind, YieldExprInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::judge::TypeJudge;
use crate::rules::shared::split_top_level_commas;
use crate::span_util::slice_span;
use crate::types::InferredType;

/// `annotations_generators` error code shared between this module and the rule.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "annotations_generators",
    docs_url: "https://www.basilisk-python.dev/errors/annotations_generators",
};

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
/// Which subscript argument carries the yield type depends on WHICH generator
/// form the annotation denotes — and every one of those forms
/// (`Generator`, `Iterator`, `Iterable`, and the `Async*` variants) requires an
/// import. Deciding that by matching the annotation's source text against those
/// spellings is not import resolution, so that recognition is deleted. Rebuild
/// it on the annotation cascade ([TYPEINF-ANNOTATION-RESOLUTION]).
pub(super) fn extract_yield_type(_annotation: &str, _base: &str) -> Option<String> {
    None
}

/// Extract the return type (3rd parameter) from `Generator[Y, S, R]`.
pub(super) fn extract_return_type_from_generator(annotation: &str) -> Option<String> {
    let bracket_pos = annotation.find('[')?;
    let inner = annotation.get(bracket_pos + 1..annotation.len().checked_sub(1)?)?;
    let args = split_top_level_commas(inner);
    args.get(2).map(|arg| arg.trim().to_owned())
}

/// The outer generator's declared annotation, as the call branch renders it.
pub(super) struct OuterAnnotation<'a> {
    /// The full annotation text (`Generator[int, None, None]`).
    pub(super) text: &'a str,
    /// Its base name (`Generator`, `Iterator`, …).
    pub(super) base: &'a str,
}

/// Check a `yield from expr` against the outer generator's declared yield type.
pub(super) fn check_yield_from(
    func: &FunctionInfo,
    yield_expr: &YieldExprInfo,
    declared_yield_type: &InferredType,
    outer: &OuterAnnotation<'_>,
    judge: &TypeJudge<'_, '_>,
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    match &yield_expr.rhs_kind {
        RhsKind::List(_) => {
            check_yield_from_list(func, yield_expr, declared_yield_type, judge, module, out);
        }
        RhsKind::CallExpr => {
            check_yield_from_call(
                func,
                yield_expr,
                declared_yield_type,
                outer,
                judge,
                module,
                out,
            );
        }
        _ => {}
    }
}

/// Check `yield from [literal_list]` against the declared yield type — the
/// iterated element type comes from the engine's synthesis of the sub-iterator
/// expression ([NARROWPLAN-INTEGRATION] Step 2).
fn check_yield_from_list(
    func: &FunctionInfo,
    yield_expr: &YieldExprInfo,
    declared_yield_type: &InferredType,
    judge: &TypeJudge<'_, '_>,
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    let (InferredType::List(element) | InferredType::Set(element)) =
        judge.inferred(yield_expr.value_span)
    else {
        return;
    };
    let elem_type = *element;
    if !crate::expr_type::is_fully_known(&elem_type)
        || elem_type.is_assignable_to(declared_yield_type)
    {
        return;
    }
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
}

/// Check `yield from callee()` against the declared yield and send types.
fn check_yield_from_call(
    func: &FunctionInfo,
    yield_expr: &YieldExprInfo,
    declared_yield_type: &InferredType,
    outer: &OuterAnnotation<'_>,
    judge: &TypeJudge<'_, '_>,
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    let (outer_ann, outer_base) = (outer.text, outer.base);
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
    // The parameter is a type expression the CASCADE evaluates — the legacy
    // parser folded class case (`A` → `a`), which can never equal the
    // properly-cased declared side.
    let callee_yield_type = judge
        .resolve_annotation_text(&callee_yield_type_str)
        .unwrap_or(InferredType::Unknown);
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
        judge,
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
    judge: &TypeJudge<'_, '_>,
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    // The `Generator`-spelling gate that stood here compared annotation source
    // text against an import-requiring name; deleted pending cascade-based
    // recognition ([TYPEINF-ANNOTATION-RESOLUTION]).
    let _ = (outer_base, callee_base);

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

    // A send type is a type expression the cascade evaluates — never a
    // string this rule case-folds ([NARROWPLAN-INTEGRATION] Step 7,
    // [#379](https://github.com/Nimblesite/Basilisk/issues/379)). An
    // unresolvable one abstains, exactly as the gradual leaves below do.
    let (Some(outer_send), Some(callee_send)) = (
        judge.resolve_annotation_text(outer_send_str.trim()),
        judge.resolve_annotation_text(callee_send_str.trim()),
    ) else {
        return;
    };

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
