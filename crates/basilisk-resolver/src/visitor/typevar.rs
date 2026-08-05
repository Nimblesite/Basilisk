//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Typevar visitor functions.

use ruff_python_ast::Expr;
use ruff_text_size::Ranged;

use crate::scope::{Pep695BoundViolation, Pep695BoundViolationKind};

use super::core::text_range_to_span;

pub(super) fn check_typevar_bound_expr(
    bound: &Expr,
    class_name: &str,
    type_param: &str,
    bare_names: &std::collections::HashSet<String>,
    current_typeparams: &std::collections::HashSet<String>,
    outer_typeparams: &std::collections::HashSet<String>,
    out: &mut Vec<Pep695BoundViolation>,
) {
    let make =
        |kind: Pep695BoundViolationKind, range: ruff_text_size::TextRange| Pep695BoundViolation {
            kind,
            class_name: class_name.to_owned(),
            type_param_name: type_param.to_owned(),
            span: text_range_to_span(range),
        };

    match bound {
        Expr::List(list) => {
            out.push(make(
                Pep695BoundViolationKind::ListLiteralBound,
                list.range(),
            ));
        }
        Expr::Tuple(tup) => {
            if tup.elts.is_empty() {
                out.push(make(Pep695BoundViolationKind::EmptyTuple, tup.range()));
            } else if tup.elts.len() == 1 {
                out.push(make(
                    Pep695BoundViolationKind::SingleElementTuple,
                    tup.range(),
                ));
            } else {
                // Check for invalid elements and outer-scope TypeVar references.
                let mut emitted = false;
                for elt in &tup.elts {
                    if !is_valid_constraint_element(elt) {
                        out.push(make(
                            Pep695BoundViolationKind::InvalidConstraintElement,
                            elt.range(),
                        ));
                        emitted = true;
                        break;
                    }
                }
                if !emitted {
                    for elt in &tup.elts {
                        if bound_refs_outer_typeparam(elt, current_typeparams, outer_typeparams) {
                            out.push(make(
                                Pep695BoundViolationKind::OuterScopeTypeVarInBound,
                                elt.range(),
                            ));
                            break;
                        }
                    }
                }
            }
        }
        Expr::Name(name) if bare_names.contains(name.id.as_str()) => {
            out.push(make(
                Pep695BoundViolationKind::NonLiteralConstraint,
                name.range(),
            ));
        }
        // Check if the bound itself references an outer-scope TypeVar (e.g. `T: dict[str, V]`).
        bound_expr
            if bound_refs_outer_typeparam(bound_expr, current_typeparams, outer_typeparams) =>
        {
            out.push(make(
                Pep695BoundViolationKind::OuterScopeTypeVarInBound,
                bound_expr.range(),
            ));
        }
        _ => {}
    }
}

/// Returns `true` if the expression references an outer-scope `TypeParam` or a
/// TypeVar-like name that is not in the current class's `TypeParam` set.
///
/// Used to detect cases like `class Nested[T: dict[str, V]]` where `V` is from
/// an outer class, or `class Foo[T: (list[S], str)]` where `S` is unresolved.
pub(super) fn bound_refs_outer_typeparam(
    expr: &Expr,
    current_typeparams: &std::collections::HashSet<String>,
    outer_typeparams: &std::collections::HashSet<String>,
) -> bool {
    match expr {
        Expr::Name(name) => {
            let n = name.id.as_str();
            // Explicitly an outer TypeVar, or a TypeVar-like single-letter uppercase name
            // not in the current class's TypeParam set.
            outer_typeparams.contains(n)
                || (is_typevar_like_name(n) && !current_typeparams.contains(n))
        }
        Expr::Subscript(sub) => {
            // Check the type arguments of a generic type expression, not the base type.
            // e.g. for `list[S]`, we check `S` not `list`.
            bound_refs_outer_typeparam(&sub.slice, current_typeparams, outer_typeparams)
        }
        Expr::Tuple(t) => t
            .elts
            .iter()
            .any(|e| bound_refs_outer_typeparam(e, current_typeparams, outer_typeparams)),
        Expr::BinOp(bin) => {
            bound_refs_outer_typeparam(&bin.left, current_typeparams, outer_typeparams)
                || bound_refs_outer_typeparam(&bin.right, current_typeparams, outer_typeparams)
        }
        _ => false,
    }
}

/// Returns `true` if the name looks like a `TypeVar` by the single-letter uppercase convention.
///
/// Single-letter uppercase names (e.g. `T`, `S`, `V`) are almost universally `TypeVars`.
/// Multi-letter names could be concrete types (e.g. `str`, `int`, `ForwardReference`).
pub(super) fn is_typevar_like_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 1
        && bytes
            .first()
            .copied()
            .is_some_and(|b| b.is_ascii_uppercase())
}

/// Returns `false` if this expression is not a valid constraint tuple element.
///
/// Valid elements are type expressions: names, subscripts, binary ops, string
/// literals (forward references), etc.
/// Invalid elements include numeric and bytes literals (not types).
pub(super) fn is_valid_constraint_element(expr: &Expr) -> bool {
    !matches!(expr, Expr::NumberLiteral(_) | Expr::BytesLiteral(_))
}
