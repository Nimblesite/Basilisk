//! Implements [TYPEINF-ANNOTATION-RESOLUTION] — the typing special forms.
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//!
//! Subscripted forms whose meaning is fixed by the typing spec — containers,
//! `Literal`, `Callable`, `Generator`, `Optional`/`Union`, `Annotated`,
//! `Final`, `TypeForm` — evaluated from their **argument expressions**, each
//! resolved by the same cascade. A form this module does not model is gradual
//! (`Unknown`), never a guess.

use ruff_python_ast::{Expr, UnaryOp};

use crate::types::{CallableInfo, InferredType, LiteralValue};

use super::{AnnotationResolver, Frame};

/// Evaluate a subscripted special form. `None` means "not a special form" —
/// the caller continues the cascade with aliases and classes.
pub(super) fn special_form(
    resolver: &AnnotationResolver<'_>,
    head: &str,
    args: &[&Expr],
    frame: &Frame,
) -> Option<InferredType> {
    let resolve = |expr: &Expr| resolver.eval(expr, frame);
    match head {
        "literal" => Some(literal_union(args)),
        "optional" => Some(InferredType::Optional(Box::new(first_type(args, &resolve)))),
        "union" => Some(InferredType::Union(args.iter().map(|a| resolve(a)).collect())),
        // `Annotated[T, ..]` and `Final[T]` are transparent wrappers.
        "annotated" | "final" => Some(first_type(args, &resolve)),
        "typeform" => Some(InferredType::TypeForm(Box::new(first_type(args, &resolve)))),
        "list" => Some(InferredType::List(Box::new(first_type(args, &resolve)))),
        "set" | "frozenset" => Some(InferredType::Set(Box::new(first_type(args, &resolve)))),
        "dict" => Some(dict_type(args, &resolve)),
        "tuple" => Some(tuple_type(args, &resolve)),
        "callable" => Some(callable_type(args, &resolve)),
        "generator" => Some(generator_type(args, &resolve)),
        // `type[X]` needs class-object modelling the cascade does not yet do:
        // gradual, so no rule invents a verdict from it.
        "type" => Some(InferredType::Unknown),
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

/// `Callable[[P..], R]`, `Callable[..., R]`, and `Callable[P, R]` for a
/// `ParamSpec` `P` (whose parameter list is unknown — the arbitrary form).
fn callable_type(args: &[&Expr], resolve: &dyn Fn(&Expr) -> InferredType) -> InferredType {
    let [params, ret] = args else {
        return InferredType::Unknown;
    };
    let param_types = match params {
        Expr::List(list) => list.elts.iter().map(resolve).collect(),
        // `...` and a `ParamSpec` both mean "parameters not constrained here".
        _ => Vec::new(),
    };
    InferredType::Callable(CallableInfo {
        param_types,
        return_type: Box::new(resolve(ret)),
    })
}

/// `Generator[Yield, Send, Return]`; any other arity is gradual.
fn generator_type(args: &[&Expr], resolve: &dyn Fn(&Expr) -> InferredType) -> InferredType {
    match args {
        [yielded, sent, returned] => InferredType::Generator(
            Box::new(resolve(yielded)),
            Box::new(resolve(sent)),
            Box::new(resolve(returned)),
        ),
        _ => InferredType::Unknown,
    }
}

/// `Literal[a, b, ..]` — a union of the literal values, read from the AST
/// literal nodes themselves rather than from annotation text, so a value's
/// case and radix survive (`Literal[0x14]` is `Literal[20]`).
fn literal_union(args: &[&Expr]) -> InferredType {
    match args {
        [] => InferredType::Unknown,
        [single] => literal_value(single),
        many => InferredType::Union(many.iter().map(|arg| literal_value(arg)).collect()),
    }
}

/// One `Literal[..]` argument.
fn literal_value(expr: &Expr) -> InferredType {
    match expr {
        Expr::NumberLiteral(number) => number_literal(&number.value),
        Expr::StringLiteral(text) => {
            InferredType::Literal(LiteralValue::Str(text.value.to_str().to_owned()))
        }
        Expr::BytesLiteral(_) => InferredType::Bytes,
        Expr::BooleanLiteral(flag) => InferredType::Literal(LiteralValue::Bool(flag.value)),
        Expr::NoneLiteral(_) => InferredType::None_,
        Expr::UnaryOp(unary) if unary.op == UnaryOp::USub => negate(literal_value(&unary.operand)),
        // An enum member (`Color.RED`) or a name: nominal, kept for display and
        // base-name comparison.
        other => super::tables::dotted_name(other)
            .map_or(InferredType::Unknown, InferredType::Named),
    }
}

/// An integer literal keeps its value; other numeric literals keep their kind.
fn number_literal(number: &ruff_python_ast::Number) -> InferredType {
    match number {
        ruff_python_ast::Number::Int(value) => value
            .as_i64()
            .map_or(InferredType::Int, |int| {
                InferredType::Literal(LiteralValue::Int(int))
            }),
        ruff_python_ast::Number::Float(_) | ruff_python_ast::Number::Complex { .. } => {
            InferredType::Float
        }
    }
}

/// `Literal[-1]` — the parser sees unary minus applied to `1`.
fn negate(ty: InferredType) -> InferredType {
    match ty {
        InferredType::Literal(LiteralValue::Int(value)) => {
            InferredType::Literal(LiteralValue::Int(-value))
        }
        other => other,
    }
}

/// Render a resolved element type back into the unpacked-tuple marker the
/// PEP 646 matcher reads (`*tuple[int, ...]`, `*Ts`).
///
/// The marker is a rendering of an **already-resolved type**, not a slice of
/// source text; it exists because [`InferredType`] has no unpacked-tuple
/// variant yet. That variant is owed by
/// [NARROWPLAN-INTEGRATION](../../../../docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION),
/// and this bridge dies with it.
pub(super) fn unpacked_marker(element: &InferredType) -> InferredType {
    InferredType::Named(format!("*{element}"))
}
