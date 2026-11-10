//! Implements [`annotations_generators`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Helper functions for `annotations_generators`: Generator return type and yield type
//! violations.
//!
//! This module provides type-parameter extraction and yield-from checking
//! utilities used by the rule to validate generator function annotations.
//! Which generator form an annotation denotes — `Generator`, `Iterator`,
//! `Iterable`, or the `Async*` variants — is decided by resolving its base
//! through the module's binding table ([ASTREBUILD-LAW]), never by matching
//! the annotation's source text against those spellings; the type parameters
//! are the subscript slice's AST nodes, never a comma-split of the source.

use basilisk_resolver::{
    assignable, FunctionInfo, ResolvedModule, RhsKind, Span, TypeNode, TypingForm, YieldExprInfo,
};
use ruff_python_ast::Expr;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::judge::TypeJudge;
use crate::rules::shared::ExprIndex;
use crate::span_util::{node_message_text, node_span, slice_span};
use crate::types::InferredType;

/// `annotations_generators` error code shared between this module and the rule.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "annotations_generators",
    docs_url: "https://www.basilisk-python.dev/errors/annotations_generators",
};

/// The yield-type argument of a return annotation that RESOLVES to a
/// generator protocol: `Generator[Y, S, R]` and `AsyncGenerator[Y, S]` carry
/// it first; `Iterator[Y]`, `Iterable[Y]`, `AsyncIterator[Y]`, and
/// `AsyncIterable[Y]` carry it as their single argument.
pub(super) fn generator_yield_type_expr<'ast>(
    module: &ResolvedModule,
    index: &ExprIndex<'ast>,
    ann_span: Span,
) -> Option<&'ast Expr> {
    let Expr::Subscript(sub) = index.expr(ann_span)? else {
        return None;
    };
    match module.bindings.form_of_with_builtins(&sub.value)? {
        TypingForm::Generator | TypingForm::AsyncGenerator => match sub.slice.as_ref() {
            Expr::Tuple(tuple) => tuple.elts.first(),
            _ => None,
        },
        TypingForm::Iterator
        | TypingForm::Iterable
        | TypingForm::AsyncIterator
        | TypingForm::AsyncIterable => match sub.slice.as_ref() {
            Expr::Tuple(_) => None,
            single => Some(single),
        },
        _ => None,
    }
}

/// The return-type argument (`R`) of a return annotation that RESOLVES to
/// `Generator[Y, S, R]` — the only generator form carrying one.
pub(super) fn generator_return_type_expr<'ast>(
    module: &ResolvedModule,
    index: &ExprIndex<'ast>,
    ann_span: Span,
) -> Option<&'ast Expr> {
    generator_positional_arg(module, index, ann_span, 2)
}

/// The send-type argument (`S`) of a return annotation resolving to
/// `Generator[Y, S, R]` or `AsyncGenerator[Y, S]`.
fn generator_send_type_expr<'ast>(
    module: &ResolvedModule,
    index: &ExprIndex<'ast>,
    ann_span: Span,
) -> Option<&'ast Expr> {
    generator_positional_arg(module, index, ann_span, 1)
}

/// The subscript argument at `position` of an annotation whose base resolves
/// to `Generator` (exactly three arguments) or `AsyncGenerator` (exactly
/// two). Malformed arities abstain — arity errors are another rule's job.
fn generator_positional_arg<'ast>(
    module: &ResolvedModule,
    index: &ExprIndex<'ast>,
    ann_span: Span,
    position: usize,
) -> Option<&'ast Expr> {
    let Expr::Subscript(sub) = index.expr(ann_span)? else {
        return None;
    };
    let arity = match module.bindings.form_of_with_builtins(&sub.value)? {
        TypingForm::Generator => 3,
        TypingForm::AsyncGenerator => 2,
        _ => return None,
    };
    let Expr::Tuple(tuple) = sub.slice.as_ref() else {
        return None;
    };
    if tuple.elts.len() != arity {
        return None;
    }
    tuple.elts.get(position)
}

/// Check a `yield from expr` against the outer generator's declared yield type.
pub(super) fn check_yield_from(
    func: &FunctionInfo,
    yield_expr: &YieldExprInfo,
    declared_yield_type: &InferredType,
    index: &ExprIndex<'_>,
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
                index,
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
    index: &ExprIndex<'_>,
    judge: &TypeJudge<'_, '_>,
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

    // Send-type compatibility is decidable from the two resolved annotations
    // alone, so it is checked whether or not the yield parameter resolves.
    check_send_type_compat(func, yield_expr, callee_ann_span, index, module, out);

    let Some(callee_yield_node) = generator_yield_type_expr(module, index, callee_ann_span) else {
        return;
    };
    let Some(callee_yield_text) = slice_span(&module.source, node_span(callee_yield_node)) else {
        return;
    };
    // The parameter is a type expression the CASCADE evaluates
    // ([TYPEINF-ANNOTATION-RESOLUTION]); an unresolvable one abstains.
    let callee_yield_type = judge
        .resolve_annotation_text(callee_yield_text.trim())
        .unwrap_or(InferredType::Unknown);
    if matches!(callee_yield_type, InferredType::Unknown | InferredType::Any) {
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
}

/// Check send type compatibility for `yield from` between two generator
/// annotations. The outer generator forwards its sends into the
/// sub-generator, so the outer send type must be assignable to the callee's
/// (<https://typing.python.org/en/latest/spec/generics.html> — `Generator`'s
/// send parameter is contravariant). Both send types are lowered through the
/// binding table and related with [`assignable`]; an undecidable pair
/// abstains. Source text appears in the MESSAGE only.
fn check_send_type_compat(
    func: &FunctionInfo,
    yield_expr: &YieldExprInfo,
    callee_ann_span: Span,
    index: &ExprIndex<'_>,
    module: &ResolvedModule,
    out: &mut Vec<Diagnostic>,
) {
    let Some(outer_ann_span) = func.return_annotation_span else {
        return;
    };
    let Some(outer_send_expr) = generator_send_type_expr(module, index, outer_ann_span) else {
        return;
    };
    let Some(callee_send_expr) = generator_send_type_expr(module, index, callee_ann_span) else {
        return;
    };
    let outer_send = TypeNode::lower(&module.bindings, outer_send_expr);
    let callee_send = TypeNode::lower(&module.bindings, callee_send_expr);
    if assignable(&outer_send, &callee_send) == Some(false) {
        let outer_text = node_message_text(&module.source, outer_send_expr);
        let callee_text = node_message_text(&module.source, callee_send_expr);
        out.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Incompatible `yield from` send type in `{}`: outer sends \
                 `{outer_text}` but sub-generator accepts `{callee_text}`",
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
