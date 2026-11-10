//! Implements [TYPEINF-COLLECTIONS-TUPLES]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-COLLECTIONS
//! Homogeneous (`tuple[X, ...]`) and PEP 646 unpacked (`*tuple[...]`/`*Ts`)
//! tuple matching, used by [`InferredType::is_assignable_to`]
//! ([TYPEINF-SUBTYPING-IMPL]).
//!
//! The annotation parser ([`crate::types_parsing`]) stores the `...`
//! terminator as `Named("...")` and unpacked segments as `Named` text starting
//! with `*`, so these helpers re-read that text to decompose tuple shapes.

use crate::types::InferredType;

/// Returns the element type `X` when `elems` is the homogeneous variable-length
/// tuple form `tuple[X, ...]`.
///
/// The annotation parser represents the `...` terminator as `Named("...")`, so
/// `tuple[str, ...]` becomes `Tuple([Str, Named("...")])`. Distinguishing this
/// from a fixed-length tuple is what lets a literal `(a, b, c)` widen to
/// `tuple[X, ...]` (PEP 484).
pub(crate) fn homogeneous_tuple_elem(elems: &[InferredType]) -> Option<&InferredType> {
    match elems {
        [elem, InferredType::Named(terminator)] if terminator == "..." => Some(elem),
        _ => None,
    }
}

/// Returns `true` when a tuple element is an unpacked variadic segment — either
/// `*tuple[...]` or a `*Ts` `TypeVarTuple`. The annotation parser stores these as
/// `Named` text beginning with `*`.
pub(crate) fn is_unpacked_tuple_elem(elem: &InferredType) -> bool {
    matches!(elem, InferredType::Named(name) if name.starts_with('*'))
}

/// What an unpacked `*tuple[...]` / `*Ts` segment consumes from a source tuple.
enum StarSegment {
    /// `*tuple[X, ...]` or `*Ts` — zero or more elements, each assignable to the
    /// element type (`None` ⇒ any, for `*Ts` / `*tuple[Any, ...]`).
    Variadic(Option<InferredType>),
    /// `*tuple[X, Y]` — a fixed run of elements consumed positionally.
    Fixed(Vec<InferredType>),
}

/// Parse an unpacked tuple element (`Named("*tuple[...]")` / `Named("*ts")`).
fn parse_star_segment(name: &str) -> StarSegment {
    let Some(inner) = name
        .strip_prefix("*tuple[")
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        // `*Ts` (a TypeVarTuple) — any number of elements of any type.
        return StarSegment::Variadic(None);
    };
    let parts = crate::types_parsing::split_type_params(inner);
    if matches!(parts.last(), Some(last) if last.trim() == "...") {
        // `*tuple[X, ...]` — homogeneous, zero or more of `X`.
        let elem = parts
            .first()
            .map(|p| InferredType::from_annotation(p.trim()));
        return StarSegment::Variadic(elem);
    }
    StarSegment::Fixed(
        parts
            .iter()
            .map(|p| InferredType::from_annotation(p.trim()))
            .collect(),
    )
}

/// Match a fixed-length source tuple against a target tuple that contains a
/// single unpacked `*tuple[...]` / `*Ts` segment (PEP 646), using
/// prefix/middle/suffix decomposition.
pub(crate) fn tuple_assignable_with_star(source: &[InferredType], target: &[InferredType]) -> bool {
    // Unhandled shapes return `true` (assignable) rather than `false`: this is a
    // best-effort matcher and must never manufacture a false positive. Only a
    // pattern we actually decompose may return `false` (a real mismatch).
    //
    // A variadic source, multiple unpacked segments, or a `*Ts` we can't read
    // all need full PEP 646 unification — be permissive.
    if source.iter().any(is_unpacked_tuple_elem) {
        return true;
    }
    // A homogeneous variadic source (`tuple[X, ...]`) has unknown length —
    // prefix/middle/suffix decomposition cannot prove a mismatch.
    if homogeneous_tuple_elem(source).is_some() {
        return true;
    }
    let Some(star_idx) = target.iter().position(is_unpacked_tuple_elem) else {
        return true;
    };
    let (prefix, rest) = target.split_at(star_idx);
    let Some((star_elem, suffix)) = rest.split_first() else {
        return true;
    };
    // Only one unpacked segment is supported.
    if suffix.iter().any(is_unpacked_tuple_elem) {
        return true;
    }
    let InferredType::Named(star_name) = star_elem else {
        return true;
    };

    match parse_star_segment(star_name) {
        StarSegment::Variadic(elem) => {
            let Some(middle_len) = source.len().checked_sub(prefix.len() + suffix.len()) else {
                return false;
            };
            prefix_suffix_match(source, prefix, suffix)
                && match elem {
                    None => true,
                    Some(elem_ty) => source
                        .iter()
                        .skip(prefix.len())
                        .take(middle_len)
                        .all(|s| s.is_assignable_to(&elem_ty)),
                }
        }
        StarSegment::Fixed(middle) => {
            if source.len() != prefix.len() + middle.len() + suffix.len() {
                return false;
            }
            prefix_suffix_match(source, prefix, suffix)
                && source
                    .iter()
                    .skip(prefix.len())
                    .take(middle.len())
                    .zip(middle.iter())
                    .all(|(s, m)| s.is_assignable_to(m))
        }
    }
}

/// Check that a source tuple's leading elements match `prefix` and trailing
/// elements match `suffix` (both fixed, non-starred). Callers guarantee
/// `source.len() >= prefix.len() + suffix.len()`.
fn prefix_suffix_match(
    source: &[InferredType],
    prefix: &[InferredType],
    suffix: &[InferredType],
) -> bool {
    source
        .iter()
        .zip(prefix.iter())
        .all(|(s, p)| s.is_assignable_to(p))
        && source
            .iter()
            .rev()
            .zip(suffix.iter().rev())
            .all(|(s, q)| s.is_assignable_to(q))
}
