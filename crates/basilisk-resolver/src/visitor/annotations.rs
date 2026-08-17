//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Annotations visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAnnAssign};
use ruff_text_size::Ranged;

use crate::canonical::{BindingTable, TypingForm};
use crate::scope::{
    InvalidStringAnnotation, InvalidStringAnnotationKind, PrimitiveKind, RhsKind, Span,
    VariableInfo,
};

use super::class_info_ext::expr_simple_name;
use super::core::{classify_rhs, text_range_to_span};

// ---------------------------------------------------------------------------
// Annotation qualifiers
//
// Each of these asks the binding table which DEFINITION an annotation refers
// to. None of them looks at the characters at the use site, so an aliased
// import (`from dataclasses import InitVar as IV`), a module-qualified use
// (`dataclasses.InitVar[int]`), and a locally shadowed name are all answered
// correctly. Implements [RESOLV-CANONICAL-BINDING].
// ---------------------------------------------------------------------------

/// Whether an annotation denotes `form`, unwrapping a PEP 484 quoted
/// forward reference: `"Final[int]"` means exactly what `Final[int]` means,
/// resolved against the module's final namespace
/// (<https://peps.python.org/pep-0484/#forward-references>).
fn annotation_form_is(bindings: &BindingTable, ann: &Expr, form: TypingForm) -> bool {
    match ann {
        Expr::StringLiteral(lit) => {
            bindings.form_of_quoted_annotation(lit.value.to_str()) == Some(form)
        }
        _ => bindings.is_form(ann, form),
    }
}

/// Whether an annotation is the dataclass keyword-only sentinel.
pub(super) fn annotation_is_kw_only(bindings: &BindingTable, ann: &Expr) -> bool {
    annotation_form_is(bindings, ann, TypingForm::KwOnlySentinel)
}

/// Whether an annotation is `InitVar` or `InitVar[T]`.
pub(super) fn annotation_is_init_var(bindings: &BindingTable, ann: &Expr) -> bool {
    annotation_form_is(bindings, ann, TypingForm::InitVar)
}

/// Whether an annotation is the `Final` qualifier, bare or subscripted.
pub(super) fn annotation_is_final(bindings: &BindingTable, ann: &Expr) -> bool {
    annotation_form_is(bindings, ann, TypingForm::FinalQualifier)
}

/// Whether an annotation is the `ClassVar` qualifier, bare or subscripted.
pub(super) fn annotation_is_class_var(bindings: &BindingTable, ann: &Expr) -> bool {
    annotation_form_is(bindings, ann, TypingForm::ClassVar)
}

/// The explicit `Required`/`NotRequired` marking of a `TypedDict` field
/// annotation: `Some(true)` for `Required[...]`, `Some(false)` for
/// `NotRequired[...]`, `None` when neither is written (the declaring class's
/// `total=` then decides, PEP 655).
///
/// The qualifier may interleave with the other wrappers PEP 655 and PEP 705
/// permit — `Annotated[NotRequired[int], meta]`, `ReadOnly[Required[str]]` —
/// so the walk unwraps exactly those, resolving every head through the
/// module's bindings. A quoted annotation is parsed and resolved against the
/// module's final namespace (PEP 484 forward references), never inspected as
/// text. The predecessor of this predicate case-folded the rendered
/// annotation and searched it for `"notrequired"`, so a field annotated with
/// a user-defined `NotRequiredData` class was silently optional and an
/// aliased qualifier was invisible.
pub(super) fn annotation_requiredness(bindings: &BindingTable, ann: &Expr) -> Option<bool> {
    if let Expr::StringLiteral(lit) = ann {
        let parsed = ruff_python_parser::parse_expression(lit.value.to_str()).ok()?;
        return requiredness_walk(
            &|expr| bindings.deferred_form_of(expr),
            &parsed.into_syntax().body,
        );
    }
    requiredness_walk(&|expr| bindings.form_of(expr), ann)
}

/// [`annotation_requiredness`]'s wrapper-unwrapping walk, parameterised over
/// positional (in-tree annotation) and final-namespace (parsed forward
/// reference) resolution.
fn requiredness_walk(form_of: &dyn Fn(&Expr) -> Option<TypingForm>, expr: &Expr) -> Option<bool> {
    let Expr::Subscript(sub) = expr else {
        return None;
    };
    match form_of(&sub.value)? {
        TypingForm::Required => Some(true),
        TypingForm::NotRequired => Some(false),
        TypingForm::ReadOnly => requiredness_walk(form_of, &sub.slice),
        TypingForm::Annotated => {
            // `Annotated[T, metadata...]`: the type is the first element.
            let first = match sub.slice.as_ref() {
                Expr::Tuple(tuple) => tuple.elts.first()?,
                other => other,
            };
            requiredness_walk(form_of, first)
        }
        _ => None,
    }
}

/// The primitive classes an annotation accepts, when every member of the
/// (possibly union) type resolves to one — `None` is abstention.
///
/// Each member resolves through the module's bindings with the builtin
/// fallback, so `int`, `builtins.int`, and an aliased import answer alike and
/// a module-local `class int` answers not at all. The walk unwraps the
/// `Required`/`NotRequired`/`ReadOnly`/`Annotated` qualifier interleavings,
/// descends `X | Y` unions, `Optional[T]`, and `Union[...]`, and reads `None`
/// from its own AST node. A quoted annotation is parsed and resolved against
/// the module's final namespace (PEP 484), never inspected as text.
pub(super) fn annotation_accepted_primitives(
    bindings: &BindingTable,
    ann: &Expr,
) -> Option<Vec<PrimitiveKind>> {
    let mut out = Vec::new();
    let complete = if let Expr::StringLiteral(lit) = ann {
        let parsed = ruff_python_parser::parse_expression(lit.value.to_str()).ok()?;
        accepted_primitives_walk(
            &|expr| bindings.deferred_form_of(expr),
            &parsed.into_syntax().body,
            &mut out,
        )
    } else {
        accepted_primitives_walk(&|expr| bindings.form_of_with_builtins(expr), ann, &mut out)
    };
    complete.then_some(out)
}

/// [`annotation_accepted_primitives`]'s member walk. Returns `false` — the
/// caller then abstains — the moment any member fails to resolve to a
/// judgeable primitive.
fn accepted_primitives_walk(
    form_of: &dyn Fn(&Expr) -> Option<TypingForm>,
    expr: &Expr,
    out: &mut Vec<PrimitiveKind>,
) -> bool {
    if let Expr::NoneLiteral(_) = expr {
        out.push(PrimitiveKind::NoneType);
        return true;
    }
    if let Expr::BinOp(bin) = expr {
        if bin.op == ruff_python_ast::Operator::BitOr {
            return accepted_primitives_walk(form_of, &bin.left, out)
                && accepted_primitives_walk(form_of, &bin.right, out);
        }
        return false;
    }
    if let Expr::Subscript(sub) = expr {
        return match form_of(&sub.value) {
            Some(TypingForm::Required | TypingForm::NotRequired | TypingForm::ReadOnly) => {
                accepted_primitives_walk(form_of, &sub.slice, out)
            }
            Some(TypingForm::Optional) => {
                out.push(PrimitiveKind::NoneType);
                accepted_primitives_walk(form_of, &sub.slice, out)
            }
            Some(TypingForm::Annotated) => {
                // `Annotated[T, metadata...]`: the type is the first element.
                let Expr::Tuple(tuple) = sub.slice.as_ref() else {
                    return accepted_primitives_walk(form_of, &sub.slice, out);
                };
                let Some(first) = tuple.elts.first() else {
                    return false;
                };
                accepted_primitives_walk(form_of, first, out)
            }
            Some(TypingForm::Union) => match sub.slice.as_ref() {
                Expr::Tuple(tuple) => tuple
                    .elts
                    .iter()
                    .all(|member| accepted_primitives_walk(form_of, member, out)),
                single => accepted_primitives_walk(form_of, single, out),
            },
            _ => false,
        };
    }
    let Some(kind) = form_of(expr).and_then(primitive_of_form) else {
        return false;
    };
    out.push(kind);
    true
}

/// The judgeable primitive a resolved form denotes, if any.
fn primitive_of_form(form: TypingForm) -> Option<PrimitiveKind> {
    match form {
        TypingForm::StrClass => Some(PrimitiveKind::Str),
        TypingForm::BytesClass => Some(PrimitiveKind::Bytes),
        TypingForm::IntClass => Some(PrimitiveKind::Int),
        TypingForm::FloatClass => Some(PrimitiveKind::Float),
        TypingForm::ComplexClass => Some(PrimitiveKind::Complex),
        TypingForm::BoolClass => Some(PrimitiveKind::Bool),
        TypingForm::NoneTypeClass => Some(PrimitiveKind::NoneType),
        _ => None,
    }
}

/// Whether a `ReadOnly` qualifier appears anywhere within an annotation.
///
/// Recurses through the composition forms an item type can be built from, so
/// `Required[ReadOnly[int]]` and `ReadOnly[int] | None` are both found — and
/// a quoted annotation is searched through its parsed forward-reference
/// expression (PEP 484), never its characters.
pub(super) fn annotation_contains_readonly_expr(bindings: &BindingTable, expr: &Expr) -> bool {
    if let Expr::StringLiteral(lit) = expr {
        return bindings.quoted_annotation_mentions(lit.value.to_str(), TypingForm::ReadOnly);
    }
    if bindings.is_form(expr, TypingForm::ReadOnly) {
        return true;
    }
    match expr {
        Expr::Subscript(sub) => annotation_contains_readonly_expr(bindings, &sub.slice),
        Expr::BinOp(bin) => {
            annotation_contains_readonly_expr(bindings, &bin.left)
                || annotation_contains_readonly_expr(bindings, &bin.right)
        }
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .any(|element| annotation_contains_readonly_expr(bindings, element)),
        _ => false,
    }
}

pub(super) fn annotation_flags(expr: &Expr) -> (bool, bool, bool) {
    match expr {
        Expr::Name(name) => (false, name.id.as_str() == "None", false),
        Expr::Attribute(attr) => (false, attr.attr.as_str() == "None", false),
        Expr::NoneLiteral(_) => (false, true, false),
        Expr::NumberLiteral(_) | Expr::BooleanLiteral(_) => (false, false, true),
        _ => (false, false, false),
    }
}

// ---------------------------------------------------------------------------
// Import info
// ---------------------------------------------------------------------------

pub(super) fn ann_assign_info_from(node: &StmtAnnAssign) -> Option<VariableInfo> {
    let name = expr_simple_name(&node.target)?;
    let rhs_kind = node.value.as_deref().map_or(RhsKind::Other, classify_rhs);
    let annotation_span = Some(text_range_to_span(node.annotation.range()));
    let rhs_span = node.value.as_deref().map(|v| text_range_to_span(v.range()));
    Some(VariableInfo {
        name,
        name_span: text_range_to_span(node.target.range()),
        has_annotation: true,
        rhs_kind,
        annotation_span,
        rhs_span,
    })
}

pub(super) fn count_unbounded_in_tuple_slice(bindings: &BindingTable, slice: &Expr) -> usize {
    let elements: &[Expr] = match slice {
        Expr::Tuple(t) => &t.elts,
        // Single-element tuple slice — check just this element
        other => return usize::from(is_unbounded_component(bindings, other)),
    };
    elements
        .iter()
        .filter(|e| is_unbounded_component(bindings, e))
        .count()
}

/// Whether the subscript head denotes the builtin `tuple` class.
///
/// REBUILT from `expr_simple_name(...) == "tuple"`, which granted builtin
/// meaning to the final token of any expression: `typing.Tuple[...]` and
/// `from builtins import tuple as T; T[...]` were missed, and a module that
/// declares its own `class tuple` had that class mistaken for the builtin.
/// Resolution goes through the module's bindings, so both PEP 585 spellings
/// answer alike and a shadowed name answers not at all
/// ([RESOLV-CANONICAL-BINDING]).
fn head_is_builtin_tuple(bindings: &BindingTable, head: &Expr) -> bool {
    matches!(
        bindings.form_of_with_builtins(head),
        Some(TypingForm::TupleClass | TypingForm::TupleAlias)
    )
}

/// Returns `true` if this expression is an unbounded tuple component:
/// - `*tuple[T, ...]` — starred subscript with an ellipsis last element
/// - `*Ts` — starred name (type-variable-tuple unpack)
///
/// [PEP 646](https://peps.python.org/pep-0646/) gives the unpack two
/// spellings: the `*` prefix and `Unpack[...]`, which "are equivalent". Both
/// are accepted here, with `Unpack` recognised by RESOLVING the subscript head
/// through the module's bindings — so `from typing import Unpack as Splat` and
/// `typing.Unpack` behave identically, and a local `class Unpack` does not.
pub(super) fn is_unbounded_component(bindings: &BindingTable, expr: &Expr) -> bool {
    let Some(unpacked) = unpacked_operand(bindings, expr) else {
        return false;
    };
    match unpacked {
        // `*tuple[T, ...]` or `*tuple[str, *tuple[str, ...]]`
        Expr::Subscript(sub) => {
            head_is_builtin_tuple(bindings, &sub.value)
                && inner_tuple_is_unbounded(bindings, &sub.slice)
        }
        // `*Ts` — type-variable-tuple unpack
        Expr::Name(_) => true,
        _ => false,
    }
}

/// The operand of an unpack, written either way, or `None` if this expression
/// is not an unpack at all.
fn unpacked_operand<'e>(bindings: &BindingTable, expr: &'e Expr) -> Option<&'e Expr> {
    match expr {
        Expr::Starred(starred) => Some(starred.value.as_ref()),
        Expr::Subscript(sub)
            if bindings.form_of_with_builtins(&sub.value) == Some(TypingForm::Unpack) =>
        {
            Some(sub.slice.as_ref())
        }
        _ => None,
    }
}

/// Returns `true` when the slice of a `tuple[...]` represents an unbounded tuple
/// (i.e. the tuple contains an ellipsis: `tuple[T, ...]`).
pub(super) fn inner_tuple_is_unbounded(bindings: &BindingTable, slice: &Expr) -> bool {
    match slice {
        Expr::Tuple(t) => t.elts.last().is_some_and(|e| {
            matches!(e, Expr::EllipsisLiteral(_)) || is_unbounded_component(bindings, e)
            // nested unbounded: `*tuple[str, ...]`
        }),
        Expr::EllipsisLiteral(_) => true,
        // Single element that is itself an unbounded starred expr
        other => is_unbounded_component(bindings, other),
    }
}

/// Returns `true` if the annotation expression is a `tuple[...]` with an invalid form.
///
/// Invalid forms include:
/// - Multiple unbounded components: `tuple[*tuple[T, ...], *Ts]`
/// - Bare ellipsis as the only element: `tuple[...]`
/// - Ellipsis not at the second position (with exactly one preceding type): `tuple[..., int]`,
///   `tuple[int, ..., int]`
/// - More than one non-ellipsis type before the ellipsis: `tuple[int, int, ...]`
/// - Non-variadic starred expression paired with ellipsis: `tuple[*tuple[str], ...]`
pub(super) fn annotation_has_multiple_unbounded(bindings: &BindingTable, expr: &Expr) -> bool {
    let Expr::Subscript(sub) = expr else {
        return false;
    };
    if !head_is_builtin_tuple(bindings, &sub.value) {
        return false;
    }
    // Check for multiple unbounded components (original rule)
    if count_unbounded_in_tuple_slice(bindings, &sub.slice) >= 2 {
        return true;
    }
    // Check for invalid ellipsis forms
    tuple_slice_has_invalid_ellipsis(&sub.slice)
}

/// Returns `true` when a `tuple[...]` slice has an invalid ellipsis placement.
///
/// Valid: `tuple[T, ...]` — exactly two elements, first is a type, second is `...`
///   (and the first must not be a non-variadic starred expression)
/// Everything else with a bare `...` is invalid.
pub(super) fn tuple_slice_has_invalid_ellipsis(slice: &Expr) -> bool {
    match slice {
        // Single `...` element: `tuple[...]` — invalid
        Expr::EllipsisLiteral(_) => true,
        Expr::Tuple(t) => {
            let elts = &t.elts;
            // Find all bare EllipsisLiteral positions
            let ellipsis_count = elts
                .iter()
                .filter(|e| matches!(e, Expr::EllipsisLiteral(_)))
                .count();
            if ellipsis_count == 0 {
                return false; // No bare ellipsis — nothing to validate here
            }
            // Valid form: exactly 2 elements, first is NOT ellipsis, second IS ellipsis
            if elts.len() == 2
                && elts
                    .get(1)
                    .is_some_and(|e| matches!(e, Expr::EllipsisLiteral(_)))
            {
                // `tuple[T, ...]` is valid only if T is not itself a starred expression.
                // Both `tuple[*tuple[str], ...]` (non-variadic) and
                // `tuple[*tuple[str, ...], ...]` (variadic) are invalid.
                return elts.first().is_some_and(|e| matches!(e, Expr::Starred(_)));
            }
            // Any other placement of bare `...` is invalid:
            // - More than one ellipsis
            // - Ellipsis not at position 1 (e.g. `tuple[..., int]`)
            // - More than 2 elements with ellipsis at end (e.g. `tuple[int, int, ...]`)
            true
        }
        _ => false,
    }
}

/// Collect all annotation spans that contain invalid multiple-unbounded-tuple patterns.
pub(super) fn collect_multiple_unbounded_tuple_spans(
    bindings: &BindingTable,
    stmts: &[Stmt],
) -> Vec<Span> {
    let mut out = Vec::new();
    collect_multi_unbounded_from_stmts(bindings, stmts, &mut out);
    out
}

pub(super) fn collect_multi_unbounded_from_stmts(
    bindings: &BindingTable,
    stmts: &[Stmt],
    out: &mut Vec<Span>,
) {
    for stmt in stmts {
        collect_multi_unbounded_from_stmt(bindings, stmt, out);
    }
}

pub(super) fn collect_multi_unbounded_from_stmt(
    bindings: &BindingTable,
    stmt: &Stmt,
    out: &mut Vec<Span>,
) {
    match stmt {
        Stmt::AnnAssign(ann) if annotation_has_multiple_unbounded(bindings, &ann.annotation) => {
            out.push(text_range_to_span(ann.annotation.range()));
        }
        Stmt::FunctionDef(func) => {
            // Check parameter annotations
            for param in super::walks::iter_all_params(&func.parameters) {
                if let Some(ann) = param.parameter.annotation.as_ref() {
                    if annotation_has_multiple_unbounded(bindings, ann) {
                        out.push(text_range_to_span(ann.range()));
                    }
                }
            }
            if let Some(vararg) = &func.parameters.vararg {
                if let Some(ann) = vararg.annotation.as_ref() {
                    if annotation_has_multiple_unbounded(bindings, ann) {
                        out.push(text_range_to_span(ann.range()));
                    }
                }
            }
            if let Some(kwarg) = &func.parameters.kwarg {
                if let Some(ann) = kwarg.annotation.as_ref() {
                    if annotation_has_multiple_unbounded(bindings, ann) {
                        out.push(text_range_to_span(ann.range()));
                    }
                }
            }
            // Check return annotation
            if let Some(ret) = func.returns.as_ref() {
                if annotation_has_multiple_unbounded(bindings, ret) {
                    out.push(text_range_to_span(ret.range()));
                }
            }
            // Recurse into function body
            collect_multi_unbounded_from_stmts(bindings, &func.body, out);
        }
        Stmt::ClassDef(cls) => {
            collect_multi_unbounded_from_stmts(bindings, &cls.body, out);
        }
        Stmt::If(if_stmt) => {
            collect_multi_unbounded_from_stmts(bindings, &if_stmt.body, out);
            collect_multi_unbounded_from_stmts(
                bindings,
                &if_stmt
                    .elif_else_clauses
                    .iter()
                    .flat_map(|c| c.body.iter())
                    .cloned()
                    .collect::<Vec<_>>(),
                out,
            );
        }
        _ => {}
    }
}

/// Recursively walk statements collecting invalid annotation patterns.
pub(super) fn collect_invalid_annotations(
    bindings: &BindingTable,
    stmts: &[Stmt],
) -> Vec<InvalidStringAnnotation> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(func) => {
                // Check parameter annotations
                for param in func
                    .parameters
                    .args
                    .iter()
                    .chain(func.parameters.posonlyargs.iter())
                    .chain(func.parameters.kwonlyargs.iter())
                {
                    if let Some(ann) = &param.parameter.annotation {
                        check_annotation_for_invalid_patterns(bindings, ann, &mut out);
                    }
                }
                // Check return annotation
                if let Some(ret) = &func.returns {
                    check_annotation_for_invalid_patterns(bindings, ret, &mut out);
                }
                out.extend(collect_invalid_annotations(bindings, &func.body));
            }
            Stmt::AnnAssign(ann) => {
                check_annotation_for_invalid_patterns(bindings, &ann.annotation, &mut out);
            }
            Stmt::ClassDef(cls) => out.extend(collect_invalid_annotations(bindings, &cls.body)),
            _ => {}
        }
    }
    out
}

/// Record an annotation that is not a type expression.
///
/// One shape today: `tuple[...]`, a bare ellipsis as the only type argument.
/// The typing spec allows `tuple[int, ...]` — the homogeneous variadic tuple —
/// and does not allow the ellipsis to stand alone
/// (<https://typing.python.org/en/latest/spec/tuples.html>).
///
/// REBUILT from the deleted `n.id.as_str() == "tuple"` test. The subscript
/// head is resolved through the module's bindings, so `builtins.tuple[...]`,
/// `typing.Tuple[...]` (the PEP 585 alias) and `from builtins import tuple as
/// T; T[...]` are all the same class and all reported, while a module that
/// defines its own `class tuple` is a different class and is not.
/// Implements [RESOLV-CANONICAL-BINDING].
pub(super) fn check_annotation_for_invalid_patterns(
    bindings: &BindingTable,
    expr: &Expr,
    out: &mut Vec<InvalidStringAnnotation>,
) {
    let Expr::Subscript(sub) = expr else {
        return;
    };
    if !matches!(
        bindings.form_of_with_builtins(&sub.value),
        Some(TypingForm::TupleClass | TypingForm::TupleAlias)
    ) {
        return;
    }
    if matches!(sub.slice.as_ref(), Expr::EllipsisLiteral(_)) {
        out.push(InvalidStringAnnotation {
            kind: InvalidStringAnnotationKind::NonTypeExpression,
            span: text_range_to_span(sub.range()),
        });
    }
}
