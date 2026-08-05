//! Implements [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! Type-level argument predicates for bound built-in method calls.
//!
//! Arguments arrive here already typed by the module's bidirectional engine
//! ([NARROWPLAN-INTEGRATION] Step 3); these predicates decide what those
//! types satisfy. Anything the engine could not resolve is
//! [`InferredType::Unknown`], which every predicate here accepts — an
//! unresolved expression never manufactures a diagnostic
//! ([CHKARCH-CONFORMANCE-MODE]).

use crate::types::{InferredType, LiteralValue};

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
pub(super) fn may_be_str(element: &InferredType) -> bool {
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
