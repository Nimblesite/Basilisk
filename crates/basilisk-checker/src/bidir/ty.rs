//! Implements [TYPEINF-TARGET-CONSTRAINTS]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS
//! The solver-internal type language for the bidirectional engine.
//!
//! [`Ty`] is the constraint solver's view of a type: the existing
//! [`InferredType`] with structure lifted (so containers decompose during
//! solving) plus type variables ([`Ty::Var`]). Ground leaves stay as
//! [`InferredType`] so every leaf subtyping decision delegates to the single
//! existing authority, [`InferredType::is_assignable_to`] — the solver never
//! re-implements assignability.

use crate::types::{CallableInfo, InferredType};

use super::tyvar::{TyVarId, TyVarStore};

/// A type as seen by the constraint solver.
///
/// Structural containers are explicit so `check`/`solve` can decompose them
/// against expected types with variables inside; everything without inner
/// structure stays a [`Ty::Ground`] leaf.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    /// A fully-known type with no solver structure and no variables inside.
    Ground(InferredType),
    /// A bounded, polar type variable — see [`super::tyvar::TyVarStore`].
    Var(TyVarId),
    /// `list[T]` with a solver-visible element type.
    List(Box<Ty>),
    /// `set[T]` with a solver-visible element type.
    Set(Box<Ty>),
    /// `dict[K, V]` with solver-visible key/value types.
    Dict(Box<Ty>, Box<Ty>),
    /// `tuple[T1, ..., Tn]` with solver-visible element types.
    Tuple(Vec<Ty>),
    /// A union of alternatives.
    Union(Vec<Ty>),
    /// `Callable[[P1..Pn], R]`; an empty parameter list means the gradual
    /// `Callable[..., R]` form, matching [`CallableInfo`].
    Callable(Vec<Ty>, Box<Ty>),
    /// `Generator[Yield, Send, Return]` with solver-visible positions.
    Generator(Box<Ty>, Box<Ty>, Box<Ty>),
}

impl Ty {
    /// The `Any` escape hatch as a ground leaf.
    #[must_use]
    pub const fn any() -> Self {
        Ty::Ground(InferredType::Any)
    }

    /// The conservative `Unknown` ([TYPEINF-EXCEEDS-NOUNKNOWN]) ground leaf.
    #[must_use]
    pub const fn unknown() -> Self {
        Ty::Ground(InferredType::Unknown)
    }

    /// Lift an [`InferredType`] into the solver language, exposing container
    /// structure so constraints can decompose it. `Optional[T]` lifts to
    /// `Union[T, None]` so the solver has one union form to reason about.
    #[must_use]
    pub fn from_inferred(inferred: &InferredType) -> Self {
        match inferred {
            InferredType::List(elem) => Ty::List(Box::new(Self::from_inferred(elem))),
            InferredType::Set(elem) => Ty::Set(Box::new(Self::from_inferred(elem))),
            InferredType::Dict(key, value) => Ty::Dict(
                Box::new(Self::from_inferred(key)),
                Box::new(Self::from_inferred(value)),
            ),
            InferredType::Tuple(elems) => {
                Ty::Tuple(elems.iter().map(Self::from_inferred).collect())
            }
            InferredType::Union(alts) => Ty::Union(alts.iter().map(Self::from_inferred).collect()),
            InferredType::Optional(inner) => Ty::Union(vec![
                Self::from_inferred(inner),
                Ty::Ground(InferredType::None_),
            ]),
            InferredType::Callable(info) => Ty::Callable(
                info.param_types.iter().map(Self::from_inferred).collect(),
                Box::new(Self::from_inferred(&info.return_type)),
            ),
            InferredType::Generator(yield_type, send_type, return_type) => Ty::Generator(
                Box::new(Self::from_inferred(yield_type)),
                Box::new(Self::from_inferred(send_type)),
                Box::new(Self::from_inferred(return_type)),
            ),
            ground => Ty::Ground(ground.clone()),
        }
    }

    /// Project back to an [`InferredType`], resolving every variable through
    /// `vars` (deferred generalization happens here — see
    /// [`TyVarStore::resolve`]).
    #[must_use]
    pub fn to_inferred(&self, vars: &TyVarStore) -> InferredType {
        match self {
            Ty::Ground(ground) => ground.clone(),
            Ty::Var(id) => vars.resolve(*id),
            Ty::List(elem) => InferredType::List(Box::new(elem.to_inferred(vars))),
            Ty::Set(elem) => InferredType::Set(Box::new(elem.to_inferred(vars))),
            Ty::Dict(key, value) => InferredType::Dict(
                Box::new(key.to_inferred(vars)),
                Box::new(value.to_inferred(vars)),
            ),
            Ty::Tuple(elems) => {
                InferredType::Tuple(elems.iter().map(|e| e.to_inferred(vars)).collect())
            }
            Ty::Union(alts) => alts
                .iter()
                .map(|a| a.to_inferred(vars))
                .fold(InferredType::Never, InferredType::union),
            Ty::Callable(params, ret) => InferredType::Callable(CallableInfo {
                param_types: params.iter().map(|p| p.to_inferred(vars)).collect(),
                return_type: Box::new(ret.to_inferred(vars)),
            }),
            Ty::Generator(yield_type, send_type, return_type) => InferredType::Generator(
                Box::new(yield_type.to_inferred(vars)),
                Box::new(send_type.to_inferred(vars)),
                Box::new(return_type.to_inferred(vars)),
            ),
        }
    }

    /// Whether any [`Ty::Var`] occurs anywhere in this type.
    #[must_use]
    pub fn contains_var(&self) -> bool {
        match self {
            Ty::Ground(_) => false,
            Ty::Var(_) => true,
            Ty::List(elem) | Ty::Set(elem) => elem.contains_var(),
            Ty::Dict(key, value) => key.contains_var() || value.contains_var(),
            Ty::Tuple(elems) | Ty::Union(elems) => elems.iter().any(Ty::contains_var),
            Ty::Callable(params, ret) => params.iter().any(Ty::contains_var) || ret.contains_var(),
            Ty::Generator(yield_type, send_type, return_type) => {
                yield_type.contains_var() || send_type.contains_var() || return_type.contains_var()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LiteralValue;

    /// [TYPEINF-TARGET-CONSTRAINTS]: lifting exposes container structure and
    /// projection inverts it exactly for var-free types.
    #[test]
    fn lift_then_project_round_trips_var_free_types() {
        let cases = [
            InferredType::Int,
            InferredType::List(Box::new(InferredType::Str)),
            InferredType::Dict(Box::new(InferredType::Str), Box::new(InferredType::Int)),
            InferredType::Tuple(vec![InferredType::Int, InferredType::Bool]),
            InferredType::Union(vec![InferredType::Int, InferredType::Str]),
            InferredType::Literal(LiteralValue::Int(1)),
            InferredType::Callable(CallableInfo {
                param_types: vec![InferredType::Int],
                return_type: Box::new(InferredType::Bool),
            }),
        ];
        let vars = TyVarStore::default();
        for inferred in cases {
            let lifted = Ty::from_inferred(&inferred);
            assert!(!lifted.contains_var(), "{inferred:?} lifts without vars");
            assert_eq!(lifted.to_inferred(&vars), inferred, "round trip");
        }
    }

    /// `Optional[T]` lifts to the solver's single union form and projects back
    /// to a union containing `None`.
    #[test]
    fn optional_lifts_to_union_with_none() {
        let lifted = Ty::from_inferred(&InferredType::Optional(Box::new(InferredType::Int)));
        assert!(
            matches!(&lifted, Ty::Union(alts) if alts.len() == 2),
            "Optional must lift to a two-alternative Ty::Union, got {lifted:?}"
        );
        let vars = TyVarStore::default();
        let projected = lifted.to_inferred(&vars);
        assert!(
            projected.is_assignable_to(&InferredType::Optional(Box::new(InferredType::Int))),
            "projection stays Optional-compatible"
        );
    }
}
