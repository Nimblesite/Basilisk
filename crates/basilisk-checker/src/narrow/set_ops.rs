//! Implements [TYPEINF-TARGET-NARROWING]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING
//! Occurrence-typing set operations over [`InferredType`] — the
//! intersection-and-negation core the narrowing environment applies.
//!
//! Both operations delegate every atomic overlap decision to the single
//! subtyping authority, [`InferredType::is_assignable_to`]
//! ([TYPEINF-SUBTYPING-IMPL]), and stay deliberately conservative in the
//! gradual directions: narrowing `Any`/`Unknown` trusts the guard, and a
//! subtraction that cannot be proven leaves the type unchanged — never a
//! fabricated `Never` ([TYPEINF-TARGET-GRADUAL]).

use crate::types::InferredType;

/// Narrow `declared` by a positive guard for `guard` — the intersection
/// `declared ∧ guard` on our lattice.
///
/// - Gradual left side (`Any`/`Unknown`): the guard's type wins (that is the
///   entire point of an `isinstance` check on untyped data).
/// - Union/Optional left side: keep the members that overlap the guard; when
///   a guarded subtype relation holds the more precise side survives.
/// - Disjoint atoms: `Never` (the branch is unreachable for that variable).
#[must_use]
pub fn intersect(declared: &InferredType, guard: &InferredType) -> InferredType {
    if matches!(declared, InferredType::Any | InferredType::Unknown) {
        return guard.clone();
    }
    match declared {
        InferredType::Union(members) => members
            .iter()
            .map(|member| intersect(member, guard))
            .fold(InferredType::Never, InferredType::union),
        InferredType::Optional(inner) => {
            let inner_part = intersect(inner, guard);
            let none_part = intersect(&InferredType::None_, guard);
            InferredType::union(inner_part, none_part)
        }
        atom => intersect_atom(atom, guard),
    }
}

/// Intersection of a non-union declared atom with the guard type.
fn intersect_atom(atom: &InferredType, guard: &InferredType) -> InferredType {
    match guard {
        InferredType::Any | InferredType::Unknown => atom.clone(),
        // A union guard distributes member-wise: `int ∧ (bool | str)` keeps
        // the overlapping member `bool` — a whole-union assignability check
        // would wrongly collapse the intersection to `Never`.
        InferredType::Union(members) => members
            .iter()
            .map(|member| intersect_atom(atom, member))
            .fold(InferredType::Never, InferredType::union),
        InferredType::Optional(inner) => InferredType::union(
            intersect_atom(atom, inner),
            intersect_atom(atom, &InferredType::None_),
        ),
        _ if atom.is_assignable_to(guard) => {
            // The declared side is at least as precise — keep it (`Literal[1]`
            // narrowed by `int` stays `Literal[1]`).
            atom.clone()
        }
        _ if guard.is_assignable_to(atom) => {
            // The guard is more precise (`int | str` member `int` guarded by
            // `bool` becomes `bool`).
            guard.clone()
        }
        _ => InferredType::Never,
    }
}

/// The complement branch: remove from `declared` everything that would have
/// satisfied the positive guard — `declared \ guard`.
///
/// Conservative by construction: only provable-full-overlap members are
/// removed. A gradual declared type stays gradual (we cannot enumerate what
/// to subtract from `Any`), so the negative branch of a guard over untyped
/// data never invents precision ([TYPEINF-TARGET-GRADUAL]).
#[must_use]
pub fn subtract(declared: &InferredType, guard: &InferredType) -> InferredType {
    match declared {
        InferredType::Any | InferredType::Unknown => declared.clone(),
        InferredType::Union(members) => members
            .iter()
            .filter(|member| !member.is_assignable_to(guard))
            .cloned()
            .fold(InferredType::Never, InferredType::union),
        InferredType::Optional(inner) => {
            let none_removed = InferredType::None_.is_assignable_to(guard);
            let inner_removed = inner.is_assignable_to(guard);
            match (inner_removed, none_removed) {
                (true, true) => InferredType::Never,
                (true, false) => InferredType::None_,
                (false, true) => (**inner).clone(),
                (false, false) => declared.clone(),
            }
        }
        atom if atom.is_assignable_to(guard) => InferredType::Never,
        atom => atom.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LiteralValue;

    /// [TYPEINF-TARGET-NARROWING]: `int | str` narrowed by `isinstance(x, int)`.
    #[test]
    fn union_intersects_to_the_guarded_member() {
        let declared = InferredType::Union(vec![InferredType::Int, InferredType::Str]);
        assert_eq!(intersect(&declared, &InferredType::Int), InferredType::Int);
        assert_eq!(subtract(&declared, &InferredType::Int), InferredType::Str);
    }

    /// `Optional[int]` under `is None` / `is not None`.
    #[test]
    fn optional_narrows_on_none_checks() {
        let declared = InferredType::Optional(Box::new(InferredType::Int));
        assert_eq!(
            intersect(&declared, &InferredType::None_),
            InferredType::None_
        );
        assert_eq!(subtract(&declared, &InferredType::None_), InferredType::Int);
    }

    /// Gradual declared types: positive trusts the guard, negative stays
    /// gradual — never fabricated precision ([TYPEINF-TARGET-GRADUAL]).
    #[test]
    fn gradual_sides_stay_safe() {
        assert_eq!(
            intersect(&InferredType::Unknown, &InferredType::Int),
            InferredType::Int
        );
        assert_eq!(
            subtract(&InferredType::Unknown, &InferredType::Int),
            InferredType::Unknown
        );
        assert_eq!(
            subtract(&InferredType::Any, &InferredType::Str),
            InferredType::Any
        );
    }

    /// Literal precision survives a broader guard.
    #[test]
    fn literal_survives_broader_guard() {
        let literal = InferredType::Literal(LiteralValue::Int(3));
        assert_eq!(intersect(&literal, &InferredType::Int), literal);
    }

    /// A UNION guard over an ATOMIC declared type distributes member-wise:
    /// `int ∧ (bool | str)` is `bool`, never a collapsed `Never`
    /// (`isinstance(x, (bool, str))` with `x: int`).
    #[test]
    fn union_guard_over_atom_distributes_memberwise() {
        let guard = InferredType::Union(vec![InferredType::Bool, InferredType::Str]);
        assert_eq!(intersect(&InferredType::Int, &guard), InferredType::Bool);

        let literals = InferredType::Union(vec![
            InferredType::Literal(LiteralValue::Int(1)),
            InferredType::Literal(LiteralValue::Str("a".to_owned())),
        ]);
        assert_eq!(
            intersect(&InferredType::Int, &literals),
            InferredType::Literal(LiteralValue::Int(1)),
            "`x in (1, \"a\")` with x: int keeps exactly the int literal"
        );
    }

    /// An `Optional` guard over an atom keeps the overlapping side.
    #[test]
    fn optional_guard_over_atom_keeps_the_overlap() {
        let guard = InferredType::Optional(Box::new(InferredType::Int));
        assert_eq!(intersect(&InferredType::Int, &guard), InferredType::Int);
        assert_eq!(intersect(&InferredType::None_, &guard), InferredType::None_);
        assert_eq!(intersect(&InferredType::Str, &guard), InferredType::Never);
    }

    /// Disjoint atoms narrow to `Never` positively and stay put negatively.
    #[test]
    fn disjoint_atoms() {
        assert_eq!(
            intersect(&InferredType::Str, &InferredType::Int),
            InferredType::Never
        );
        assert_eq!(
            subtract(&InferredType::Str, &InferredType::Int),
            InferredType::Str
        );
    }
}
