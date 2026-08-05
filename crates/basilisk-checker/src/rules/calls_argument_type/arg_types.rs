//! Implements [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! Type-directed argument resolution for bound built-in method calls.
//!
//! The resolver classifies a call argument by the *syntactic shape* of its
//! expression (`RhsKind`): a name is `Other` whatever it was declared to be,
//! and a display element is `Other` even when its declared type is known. A
//! rule that matches on those shapes cannot tell a valid `[*p]` (`p: list[str]`)
//! from an invalid `[1]`, so it must either reject both or accept both
//! (GitHub #356).
//!
//! This module answers the question the shape cannot: the *type* of the
//! argument expression, resolved through the declared types visible at that
//! point in the module. Anything it cannot resolve is [`InferredType::Unknown`],
//! which every compatibility predicate here accepts — an unresolved expression
//! never manufactures a diagnostic ([CHKARCH-CONFORMANCE-MODE]).

use crate::types::{InferredType, LiteralValue};

/// The type produced by iterating `container`; `Unknown` when unknowable.
fn iterated_type(container: &InferredType) -> InferredType {
    match container {
        InferredType::List(element) | InferredType::Set(element) => element.as_ref().clone(),
        InferredType::Dict(key, _) => key.as_ref().clone(),
        InferredType::Tuple(elements) => elements
            .iter()
            .cloned()
            .fold(InferredType::Never, InferredType::union),
        InferredType::Str | InferredType::LiteralString => InferredType::Str,
        InferredType::Generator(yielded, _, _) => yielded.as_ref().clone(),
        _ => InferredType::Unknown,
    }
}

/// The type of a numeric literal.
fn number_type(number: &ruff_python_ast::Number) -> InferredType {
    match number {
        ruff_python_ast::Number::Int(_) => InferredType::Int,
        ruff_python_ast::Number::Float(_) => InferredType::Float,
        ruff_python_ast::Number::Complex { .. } => InferredType::Named("complex".to_owned()),
    }
}

/// Does `argument` satisfy an `Iterable[str]` / `Iterable[LiteralString]`
/// parameter such as `str.join`'s?
///
/// `str` itself qualifies — iterating a string yields strings. Everything this
/// module could not resolve qualifies too; only a positively-known mismatch is
/// rejected.
pub(crate) fn satisfies_str_iterable(argument: &InferredType) -> bool {
    match argument {
        InferredType::Literal(value) => matches!(value, LiteralValue::Str(_)),
        InferredType::Int
        | InferredType::Float
        | InferredType::Bool
        | InferredType::Bytes
        | InferredType::None_ => false,
        InferredType::List(element) | InferredType::Set(element) => may_be_str(element),
        InferredType::Dict(key, _) => may_be_str(key),
        InferredType::Tuple(elements) => elements.iter().all(may_be_str),
        InferredType::Generator(yielded, _, _) => may_be_str(yielded),
        // A union member that qualifies is enough: this check sees declared
        // types, not narrowed ones, so `list[str] | None` inside an
        // `if values is not None:` guard must not be rejected.
        InferredType::Union(members) => members.iter().any(satisfies_str_iterable),
        InferredType::Optional(inner) => satisfies_str_iterable(inner),
        // `str`/`LiteralString` iterate as `Iterable[str]`; everything else
        // reaching here is unresolved (`Unknown`, `Any`, a named class) and is
        // accepted rather than guessed at.
        _ => true,
    }
}

/// Could a value of this type be a `str`? `true` unless it is positively known
/// to be something else.
fn may_be_str(element: &InferredType) -> bool {
    match element {
        InferredType::Int
        | InferredType::Float
        | InferredType::Bool
        | InferredType::Bytes
        | InferredType::None_
        | InferredType::List(_)
        | InferredType::Set(_)
        | InferredType::Dict(_, _)
        | InferredType::Tuple(_) => false,
        InferredType::Literal(value) => matches!(value, LiteralValue::Str(_)),
        InferredType::Union(members) => members.iter().all(may_be_str),
        InferredType::Optional(inner) => may_be_str(inner),
        _ => true,
    }
}
