//! Implements [TYPEINF-TARGET-CONSTRAINTS]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS
//! Bounded, polar type variables with deferred generalization.
//!
//! Each variable carries explicit lower/upper bounds (Pyright's type-interval
//! idea, Pyrefly's `Var`) and an input/output polarity borrowed from Dolan's
//! algebraic subtyping and Parreaux's Simple-sub — deliberately **without**
//! full biunification. Generalization is deferred: a list literal `[1]`
//! synthesizes `list[Var{lower=Literal[1]}]`, and the variable settles only
//! when [`TyVarStore::resolve`] is forced by a constraining use, preserving
//! `Literal`/generic precision instead of eagerly widening `Literal[1] → int`.

use crate::types::InferredType;

use super::ty::Ty;

/// Identifier of a type variable inside one [`TyVarStore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyVarId(usize);

impl TyVarId {
    /// The variable's index into its store.
    #[must_use]
    pub fn index(self) -> usize {
        self.0
    }
}

/// The position a variable was introduced in, deciding how it resolves.
///
/// An **output** (positive/covariant) variable resolves to the union of its
/// lower bounds — what flowed *into* it. An **input** (negative/contravariant)
/// variable resolves to its upper bounds — what is *demanded* of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    /// Positive position: results, container elements being produced.
    Output,
    /// Negative position: parameters, values being consumed.
    Input,
}

/// One variable's accumulated solving state.
#[derive(Debug, Clone)]
pub struct TyVarData {
    /// Types constrained to flow into the variable (`τ <: Var`).
    pub lower: Vec<Ty>,
    /// Types the variable must flow into (`Var <: τ`).
    pub upper: Vec<Ty>,
    /// Introduction polarity — see [`Polarity`].
    pub polarity: Polarity,
}

/// Allocation and resolution of type variables for one inference run.
#[derive(Debug, Clone, Default)]
pub struct TyVarStore {
    vars: Vec<TyVarData>,
}

impl TyVarStore {
    /// Allocate a fresh, unbounded variable with the given polarity.
    pub fn fresh(&mut self, polarity: Polarity) -> TyVarId {
        let id = self.vars.len();
        self.vars.push(TyVarData {
            lower: Vec::new(),
            upper: Vec::new(),
            polarity,
        });
        TyVarId(id)
    }

    /// Record `ty <: var` — a new lower bound. Duplicates are dropped.
    pub fn add_lower(&mut self, id: TyVarId, ty: Ty) {
        if let Some(data) = self.vars.get_mut(id.index()) {
            if !data.lower.contains(&ty) {
                data.lower.push(ty);
            }
        }
    }

    /// Record `var <: ty` — a new upper bound. Duplicates are dropped.
    pub fn add_upper(&mut self, id: TyVarId, ty: Ty) {
        if let Some(data) = self.vars.get_mut(id.index()) {
            if !data.upper.contains(&ty) {
                data.upper.push(ty);
            }
        }
    }

    /// The variable's current state, if it exists in this store.
    #[must_use]
    pub fn get(&self, id: TyVarId) -> Option<&TyVarData> {
        self.vars.get(id.index())
    }

    /// Number of variables allocated so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// Whether no variables have been allocated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// Resolve a variable to a concrete [`InferredType`] — the **deferred
    /// generalization** point ([TYPEINF-TARGET-CONSTRAINTS]).
    ///
    /// - Output polarity: the union of the lower bounds (what was produced) —
    ///   `Var{lower=Literal[1]}` resolves to `Literal[1]`, not `int`.
    /// - Input polarity: the first upper bound (what is demanded), since an
    ///   unconstrained parameter accepts anything.
    /// - No bounds at all: the conservative `Unknown`
    ///   ([TYPEINF-EXCEEDS-NOUNKNOWN]) so downstream rules stay silent —
    ///   never a guess.
    ///
    /// Cycles between variables (a bound mentioning the variable being
    /// resolved) collapse to `Unknown`, the Stage 0 stand-in for the
    /// divergent sentinel planned in [TYPEINF-TARGET-INCREMENTAL].
    #[must_use]
    pub fn resolve(&self, id: TyVarId) -> InferredType {
        self.resolve_guarded(id, &mut Vec::new())
    }

    fn resolve_guarded(&self, id: TyVarId, visiting: &mut Vec<TyVarId>) -> InferredType {
        if visiting.contains(&id) {
            return InferredType::Unknown;
        }
        let Some(data) = self.vars.get(id.index()) else {
            return InferredType::Unknown;
        };
        visiting.push(id);
        let resolved = match data.polarity {
            Polarity::Output => self.union_of(&data.lower, visiting),
            Polarity::Input => self.first_bound(&data.upper, visiting),
        };
        let _ = visiting.pop();
        resolved
    }

    /// Union of the given bounds, or `Unknown` when there are none.
    fn union_of(&self, bounds: &[Ty], visiting: &mut Vec<TyVarId>) -> InferredType {
        if bounds.is_empty() {
            return InferredType::Unknown;
        }
        bounds
            .iter()
            .map(|bound| self.project(bound, visiting))
            .fold(InferredType::Never, InferredType::union)
    }

    /// First demanded bound, or `Unknown` when nothing is demanded.
    fn first_bound(&self, bounds: &[Ty], visiting: &mut Vec<TyVarId>) -> InferredType {
        bounds
            .first()
            .map_or(InferredType::Unknown, |bound| self.project(bound, visiting))
    }

    /// Project a bound to ground, resolving nested variables under the cycle
    /// guard rather than through [`Ty::to_inferred`] (which would restart an
    /// unguarded resolution).
    fn project(&self, bound: &Ty, visiting: &mut Vec<TyVarId>) -> InferredType {
        match bound {
            Ty::Var(inner) => self.resolve_guarded(*inner, visiting),
            Ty::List(elem) => InferredType::List(Box::new(self.project(elem, visiting))),
            Ty::Set(elem) => InferredType::Set(Box::new(self.project(elem, visiting))),
            Ty::Dict(key, value) => InferredType::Dict(
                Box::new(self.project(key, visiting)),
                Box::new(self.project(value, visiting)),
            ),
            Ty::Tuple(elems) => {
                InferredType::Tuple(elems.iter().map(|e| self.project(e, visiting)).collect())
            }
            Ty::Union(alts) => alts
                .iter()
                .map(|a| self.project(a, visiting))
                .fold(InferredType::Never, InferredType::union),
            Ty::Callable(params, ret) => InferredType::Callable(crate::types::CallableInfo {
                param_types: params.iter().map(|p| self.project(p, visiting)).collect(),
                return_type: Box::new(self.project(ret, visiting)),
            }),
            Ty::Generator(yield_type, send_type, return_type) => InferredType::Generator(
                Box::new(self.project(yield_type, visiting)),
                Box::new(self.project(send_type, visiting)),
                Box::new(self.project(return_type, visiting)),
            ),
            Ty::Ground(ground) => ground.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LiteralValue;

    /// [TYPEINF-TARGET-CONSTRAINTS]: deferred generalization — an output var
    /// whose only lower bound is `Literal[1]` resolves to `Literal[1]`, never
    /// eagerly widened to `int`.
    #[test]
    fn output_var_preserves_literal_precision() {
        let mut store = TyVarStore::default();
        let var = store.fresh(Polarity::Output);
        store.add_lower(var, Ty::Ground(InferredType::Literal(LiteralValue::Int(1))));
        assert_eq!(
            store.resolve(var),
            InferredType::Literal(LiteralValue::Int(1))
        );
    }

    /// Multiple lower bounds union (`[1, "a"]` → `Literal[1] | Literal["a"]`).
    #[test]
    fn output_var_unions_lower_bounds() {
        let mut store = TyVarStore::default();
        let var = store.fresh(Polarity::Output);
        store.add_lower(var, Ty::Ground(InferredType::Int));
        store.add_lower(var, Ty::Ground(InferredType::Str));
        let resolved = store.resolve(var);
        assert!(InferredType::Int.is_assignable_to(&resolved));
        assert!(InferredType::Str.is_assignable_to(&resolved));
    }

    /// An input var resolves to what is demanded of it; unconstrained vars of
    /// either polarity stay `Unknown` — no guessing
    /// ([TYPEINF-EXCEEDS-NOUNKNOWN]).
    #[test]
    fn input_var_takes_upper_bound_and_unbounded_stays_unknown() {
        let mut store = TyVarStore::default();
        let input = store.fresh(Polarity::Input);
        store.add_upper(input, Ty::Ground(InferredType::Int));
        assert_eq!(store.resolve(input), InferredType::Int);

        let unbounded = store.fresh(Polarity::Output);
        assert_eq!(store.resolve(unbounded), InferredType::Unknown);
    }

    /// Mutually-referential variables terminate and collapse to `Unknown`
    /// instead of recursing forever.
    #[test]
    fn cyclic_bounds_resolve_to_unknown() {
        let mut store = TyVarStore::default();
        let a = store.fresh(Polarity::Output);
        let b = store.fresh(Polarity::Output);
        store.add_lower(a, Ty::Var(b));
        store.add_lower(b, Ty::Var(a));
        assert_eq!(store.resolve(a), InferredType::Unknown);
    }

    /// A structural bound (`list[Var]`) resolves through the nested variable.
    #[test]
    fn structural_bounds_project_nested_vars() {
        let mut store = TyVarStore::default();
        let elem = store.fresh(Polarity::Output);
        store.add_lower(elem, Ty::Ground(InferredType::Bool));
        let outer = store.fresh(Polarity::Output);
        store.add_lower(outer, Ty::List(Box::new(Ty::Var(elem))));
        assert_eq!(
            store.resolve(outer),
            InferredType::List(Box::new(InferredType::Bool))
        );
    }
}
