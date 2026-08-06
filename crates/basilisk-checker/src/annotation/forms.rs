//! Implements [TYPEINF-ANNOTATION-RESOLUTION] — subscripted builtin types.
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//!
//! Subscripted builtin containers, evaluated from their **argument
//! expressions**, each resolved by the same cascade. A head this module does
//! not model is gradual (`Unknown`), never a guess.
//!
//! The forms that require an import to name are NOT recognised here and must
//! not be reintroduced by spelling: identifying them is a question about
//! definitions, and the mechanism that answers it lawfully does not exist yet.

use ruff_python_ast::Expr;

use crate::types::InferredType;

use super::{AnnotationResolver, Frame};

/// Evaluate a subscripted builtin. `None` means "not modelled here" — the
/// caller continues the cascade with aliases and classes.
pub(super) fn special_form(
    resolver: &AnnotationResolver<'_>,
    head: &str,
    args: &[&Expr],
    frame: &Frame,
) -> Option<InferredType> {
    let resolve = |expr: &Expr| resolver.eval(expr, frame);
    match head {
        "list" => Some(InferredType::List(Box::new(first_type(args, &resolve)))),
        "set" | "frozenset" => Some(InferredType::Set(Box::new(first_type(args, &resolve)))),
        "dict" => Some(dict_type(args, &resolve)),
        "tuple" => Some(tuple_type(args, &resolve)),
        // `type[X]` is a CLASS OBJECT: the nominal `type` leaf keeps "a value
        // provably an instance (`None`, `3`) is no class object" enforceable,
        // while WHICH class stays gradual — `X` is not modelled yet:
        // gradual, so no rule invents a verdict from it.
        "type" => Some(InferredType::Named("type".to_owned())),
        _ => None,
    }
}

/// The first argument's type, or gradual when there is none.
fn first_type(args: &[&Expr], resolve: &dyn Fn(&Expr) -> InferredType) -> InferredType {
    args.first()
        .map_or(InferredType::Unknown, |expr| resolve(expr))
}

/// `dict[K, V]`; any other arity is gradual.
fn dict_type(args: &[&Expr], resolve: &dyn Fn(&Expr) -> InferredType) -> InferredType {
    match args {
        [key, value] => InferredType::Dict(Box::new(resolve(key)), Box::new(resolve(value))),
        _ => InferredType::Unknown,
    }
}

/// `tuple[X, Y]`, `tuple[X, ...]`, and the PEP 484 empty form `tuple[()]`.
fn tuple_type(args: &[&Expr], resolve: &dyn Fn(&Expr) -> InferredType) -> InferredType {
    if let [Expr::Tuple(empty)] = args {
        if empty.elts.is_empty() {
            return InferredType::Tuple(Vec::new());
        }
    }
    InferredType::Tuple(args.iter().map(|arg| resolve(arg)).collect())
}

/// Render a resolved element type back into the unpacked-tuple marker the
/// PEP 646 matcher reads (`*tuple[int, ...]`).
///
/// The marker is a rendering of an **already-resolved type**, not a slice of
/// source text; it exists because [`InferredType`] has no unpacked-tuple
/// variant yet. That variant is owed by
/// [NARROWPLAN-INTEGRATION](../../../../docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION),
/// and this bridge dies with it.
pub(super) fn unpacked_marker(element: &InferredType) -> InferredType {
    InferredType::Named(format!("*{element}"))
}
