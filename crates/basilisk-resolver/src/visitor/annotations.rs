//! Annotations visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAnnAssign};
use ruff_text_size::Ranged;

use crate::scope::{
    InvalidStringAnnotation, InvalidStringAnnotationKind, RhsKind, Span, VariableInfo,
};

use super::class_info_ext::expr_simple_name;
use super::core::{classify_rhs, text_range_to_span};

pub(super) fn annotation_flags(expr: &Expr) -> (bool, bool, bool) {
    match expr {
        Expr::Name(name) => {
            let s = name.id.as_str();
            (s == "Any", s == "None", false)
        }
        Expr::Attribute(attr) => {
            let s = attr.attr.as_str();
            (s == "Any", s == "None", false)
        }
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

pub(super) fn annotation_is_kw_only(ann: &Expr) -> bool {
    matches!(ann, Expr::Name(n) if n.id.as_str() == "KW_ONLY")
}

/// Returns `true` when the annotation expression is `InitVar[T]`.
///
/// Matches both `InitVar[T]` and `dataclasses.InitVar[T]`.
pub(super) fn annotation_is_init_var(ann: &Expr) -> bool {
    match ann {
        Expr::Subscript(sub) => {
            // Check if the base is "InitVar" or "dataclasses.InitVar"
            match sub.value.as_ref() {
                Expr::Name(n) => n.id.as_str() == "InitVar",
                Expr::Attribute(attr) => attr.attr.as_str() == "InitVar",
                _ => false,
            }
        }
        _ => false,
    }
}

/// For a field value expression, returns `Some(true)` when it is `field(kw_only=True, ...)`,
/// `Some(false)` when it is `field(kw_only=False, ...)`, and `None` otherwise.
pub(super) fn count_unbounded_in_tuple_slice(slice: &Expr) -> usize {
    let elements: &[Expr] = match slice {
        Expr::Tuple(t) => &t.elts,
        // Single-element tuple slice — check just this element
        other => return usize::from(is_unbounded_component(other)),
    };
    elements
        .iter()
        .filter(|e| is_unbounded_component(e))
        .count()
}

/// Returns `true` if this expression is an unbounded tuple component:
/// - `*tuple[T, ...]` — starred subscript with an ellipsis last element
/// - `*Name` — starred name (`TypeVarTuple` unpack)
/// - `Unpack[tuple[T, ...]]` — legacy form
pub(super) fn is_unbounded_component(expr: &Expr) -> bool {
    match expr {
        Expr::Starred(starred) => match starred.value.as_ref() {
            // `*tuple[T, ...]` or `*tuple[str, *tuple[str, ...]]`
            Expr::Subscript(sub) => {
                if expr_simple_name(&sub.value).as_deref() != Some("tuple") {
                    return false;
                }
                inner_tuple_is_unbounded(&sub.slice)
            }
            // `*Ts` — TypeVarTuple unpack
            Expr::Name(_) => true,
            _ => false,
        },
        // `Unpack[tuple[T, ...]]` — legacy unpack form
        Expr::Subscript(sub) if expr_simple_name(&sub.value).as_deref() == Some("Unpack") => {
            match sub.slice.as_ref() {
                Expr::Subscript(inner_sub)
                    if expr_simple_name(&inner_sub.value).as_deref() == Some("tuple") =>
                {
                    inner_tuple_is_unbounded(&inner_sub.slice)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// Returns `true` when the slice of a `tuple[...]` represents an unbounded tuple
/// (i.e. the tuple contains an ellipsis: `tuple[T, ...]`).
pub(super) fn inner_tuple_is_unbounded(slice: &Expr) -> bool {
    match slice {
        Expr::Tuple(t) => t.elts.last().is_some_and(|e| {
            matches!(e, Expr::EllipsisLiteral(_)) || is_unbounded_component(e) // nested unbounded: `*tuple[str, ...]`
        }),
        Expr::EllipsisLiteral(_) => true,
        // Single element that is itself an unbounded starred expr
        other => is_unbounded_component(other),
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
pub(super) fn annotation_has_multiple_unbounded(expr: &Expr) -> bool {
    let Expr::Subscript(sub) = expr else {
        return false;
    };
    if expr_simple_name(&sub.value).as_deref() != Some("tuple") {
        return false;
    }
    // Check for multiple unbounded components (original rule)
    if count_unbounded_in_tuple_slice(&sub.slice) >= 2 {
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
pub(super) fn collect_multiple_unbounded_tuple_spans(stmts: &[Stmt]) -> Vec<Span> {
    let mut out = Vec::new();
    collect_multi_unbounded_from_stmts(stmts, &mut out);
    out
}

pub(super) fn collect_multi_unbounded_from_stmts(stmts: &[Stmt], out: &mut Vec<Span>) {
    for stmt in stmts {
        collect_multi_unbounded_from_stmt(stmt, out);
    }
}

pub(super) fn collect_multi_unbounded_from_stmt(stmt: &Stmt, out: &mut Vec<Span>) {
    match stmt {
        Stmt::AnnAssign(ann) => {
            if annotation_has_multiple_unbounded(&ann.annotation) {
                out.push(text_range_to_span(ann.annotation.range()));
            }
        }
        Stmt::FunctionDef(func) => {
            // Check parameter annotations
            let all_params = func
                .parameters
                .posonlyargs
                .iter()
                .chain(func.parameters.args.iter())
                .chain(func.parameters.kwonlyargs.iter());
            for param in all_params {
                if let Some(ann) = param.parameter.annotation.as_ref() {
                    if annotation_has_multiple_unbounded(ann) {
                        out.push(text_range_to_span(ann.range()));
                    }
                }
            }
            if let Some(vararg) = &func.parameters.vararg {
                if let Some(ann) = vararg.annotation.as_ref() {
                    if annotation_has_multiple_unbounded(ann) {
                        out.push(text_range_to_span(ann.range()));
                    }
                }
            }
            if let Some(kwarg) = &func.parameters.kwarg {
                if let Some(ann) = kwarg.annotation.as_ref() {
                    if annotation_has_multiple_unbounded(ann) {
                        out.push(text_range_to_span(ann.range()));
                    }
                }
            }
            // Check return annotation
            if let Some(ret) = func.returns.as_ref() {
                if annotation_has_multiple_unbounded(ret) {
                    out.push(text_range_to_span(ret.range()));
                }
            }
            // Recurse into function body
            collect_multi_unbounded_from_stmts(&func.body, out);
        }
        Stmt::ClassDef(cls) => {
            collect_multi_unbounded_from_stmts(&cls.body, out);
        }
        Stmt::If(if_stmt) => {
            collect_multi_unbounded_from_stmts(&if_stmt.body, out);
            collect_multi_unbounded_from_stmts(
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

// ---------------------------------------------------------------------------
// assert_type type-aware collection
// ---------------------------------------------------------------------------

/// Strip `Annotated[T, ...]` wrapper, returning just `T`.
pub(super) fn strip_annotated_wrapper(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    if !ann.starts_with("Annotated[") {
        return None;
    }
    let inner_start = "Annotated[".len();
    let inner = &ann[inner_start..];
    // Find the end of the first argument (handle nested brackets).
    let mut depth = 0i32;
    let mut end = inner.len();
    for (i, ch) in inner.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => {
                if depth == 0 {
                    // Hit closing ] of Annotated without a comma — whole inner is one arg.
                    end = i;
                    break;
                }
                depth -= 1;
            }
            ',' if depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
    }
    Some(inner[..end].trim())
}

/// Returns `true` when `actual` type is a user-defined type alias that cannot be
/// resolved without a full type engine. These produce false positives because we
/// compare annotation text without expanding aliases.
///
/// A type is a user-defined alias when its base identifier (the part before `[`)
/// starts with an uppercase letter and is not a known typing special form.
pub(super) fn annotation_contains_readonly_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Name(name) => name.id.as_str() == "ReadOnly",
        Expr::Attribute(attr) => attr.attr.as_str() == "ReadOnly",
        Expr::Subscript(sub) => {
            annotation_contains_readonly_expr(&sub.value)
                || annotation_contains_readonly_expr(&sub.slice)
        }
        Expr::BinOp(bin) => {
            annotation_contains_readonly_expr(&bin.left)
                || annotation_contains_readonly_expr(&bin.right)
        }
        Expr::Tuple(tuple) => tuple.elts.iter().any(annotation_contains_readonly_expr),
        _ => false,
    }
}

/// Extract `ReadOnly` field names from a functional `TypedDict(...)` call's dict literal.
///
/// Returns a set of field names that have `ReadOnly[...]` annotations.
pub(super) fn ann_text_is_final(text: &str) -> bool {
    let t = text.trim();
    t == "Final" || t.starts_with("Final[") || t == "typing.Final" || t.starts_with("typing.Final[")
}

/// Collect the names of module-level `Final`-annotated variables from a statement list.
pub(super) fn is_classvar_annotation(expr: &Expr) -> bool {
    match expr {
        Expr::Name(name) => name.id.as_str() == "ClassVar",
        Expr::Subscript(sub) => {
            matches!(&*sub.value, Expr::Name(name) if name.id.as_str() == "ClassVar")
        }
        _ => false,
    }
}

/// Recursively walk statements finding call expressions that instantiate
/// protocols or abstract classes.
pub(super) fn collect_invalid_annotations(stmts: &[Stmt]) -> Vec<InvalidStringAnnotation> {
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
                        check_annotation_for_invalid_patterns(ann, &mut out);
                    }
                }
                // Check return annotation
                if let Some(ret) = &func.returns {
                    check_annotation_for_invalid_patterns(ret, &mut out);
                }
                out.extend(collect_invalid_annotations(&func.body));
            }
            Stmt::AnnAssign(ann) => {
                check_annotation_for_invalid_patterns(&ann.annotation, &mut out);
            }
            Stmt::ClassDef(cls) => out.extend(collect_invalid_annotations(&cls.body)),
            _ => {}
        }
    }
    out
}

/// Check a single annotation expression for invalid patterns like `tuple[...]`.
pub(super) fn check_annotation_for_invalid_patterns(
    expr: &Expr,
    out: &mut Vec<InvalidStringAnnotation>,
) {
    if let Expr::Subscript(sub) = expr {
        // Check for `tuple[...]` — bare ellipsis as only argument
        let is_tuple = matches!(sub.value.as_ref(), Expr::Name(n) if n.id.as_str() == "tuple");
        if is_tuple {
            if let Expr::EllipsisLiteral(_) = sub.slice.as_ref() {
                out.push(InvalidStringAnnotation {
                    kind: InvalidStringAnnotationKind::NonTypeExpression,
                    span: text_range_to_span(sub.range()),
                });
            }
        }
    }
}
