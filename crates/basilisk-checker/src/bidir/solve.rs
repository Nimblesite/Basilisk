//! Implements [TYPEINF-TARGET-CONSTRAINTS]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS
//! The subtype-constraint solver — stage two of the two-stage architecture.
//!
//! A worklist decomposes structural constraints (`list[A] <: list[B]` →
//! `A <: B`), routes variable constraints into bound updates on the
//! [`TyVarStore`], and discharges every ground leaf through the single
//! existing subtyping authority, [`InferredType::is_assignable_to`]
//! ([TYPEINF-SUBTYPING-IMPL]) — the solver never re-implements
//! assignability. Constraints whose shape still involves variables after
//! decomposition are re-verified on the resolved projections at the end.

use crate::types::InferredType;

use super::constraints::{Constraint, ConstraintReason};
use super::ty::Ty;
use super::tyvar::TyVarStore;
use ruff_text_size::TextRange;

/// Backstop against pathological propagation chains. Generous: real modules
/// produce constraint counts linear in expression count, far below this.
const SOLVE_FUEL: usize = 100_000;

/// One constraint the solver could not satisfy.
#[derive(Debug, Clone, PartialEq)]
pub struct SolveError {
    /// The unsatisfied obligation, projected to ground types for display.
    pub sub: InferredType,
    /// The expected side of the failed obligation.
    pub sup: InferredType,
    /// Source range of the offending expression.
    pub range: TextRange,
    /// Why the obligation existed.
    pub reason: ConstraintReason,
}

/// The outcome of a solve: the variable store with its accumulated bounds
/// (consulted for deferred generalization) plus any unsatisfied constraints.
#[derive(Debug)]
pub struct Solution {
    /// Variable bounds accumulated during solving.
    pub vars: TyVarStore,
    /// Constraints that could not be satisfied.
    pub errors: Vec<SolveError>,
}

/// Solve a constraint set against the variables the generation pass created.
///
/// Gradual-guarantee posture ([TYPEINF-TARGET-GRADUAL]): running out of fuel
/// or meeting an undecomposable-but-variable-bearing shape never invents an
/// error — residual obligations are re-checked on ground projections, and
/// anything still uncertain resolves through `Unknown`, which
/// [`InferredType::is_assignable_to`] treats as compatible.
#[must_use]
pub fn solve(mut vars: TyVarStore, constraints: Vec<Constraint>) -> Solution {
    let mut worklist: Vec<Constraint> = constraints;
    let mut residual: Vec<Constraint> = Vec::new();
    let mut errors: Vec<SolveError> = Vec::new();
    let mut fuel = SOLVE_FUEL;

    while let Some(constraint) = worklist.pop() {
        if fuel == 0 {
            residual.push(constraint);
            residual.append(&mut worklist);
            break;
        }
        fuel -= 1;
        step(
            constraint,
            &mut vars,
            &mut worklist,
            &mut residual,
            &mut errors,
        );
    }
    verify_residual(&vars, residual, &mut errors);
    Solution { vars, errors }
}

/// Process one constraint: update bounds, decompose, or leaf-check.
fn step(
    constraint: Constraint,
    vars: &mut TyVarStore,
    worklist: &mut Vec<Constraint>,
    residual: &mut Vec<Constraint>,
    errors: &mut Vec<SolveError>,
) {
    let Constraint {
        sub,
        sup,
        range,
        reason,
    } = constraint;
    match (sub, sup) {
        (Ty::Var(v), sup) => bound_var_above(v, &sup, range, reason, vars, worklist),
        (sub, Ty::Var(v)) => bound_var_below(v, &sub, range, reason, vars, worklist),
        (sub @ Ty::List(_), sup @ Ty::List(_))
        | (sub @ Ty::Set(_), sup @ Ty::Set(_))
        | (sub @ Ty::Dict(_, _), sup @ Ty::Dict(_, _))
        | (sub @ Ty::Tuple(_), sup @ Ty::Tuple(_)) => {
            decompose(sub, sup, range, reason, worklist, residual);
        }
        (sub, sup) if !sub.contains_var() && !sup.contains_var() => {
            leaf_check(&sub, &sup, range, reason, vars, errors);
        }
        (sub, sup) => decompose(sub, sup, range, reason, worklist, residual),
    }
}

/// `Var <: sup`: record the upper bound and propagate to existing lowers.
fn bound_var_above(
    var: super::tyvar::TyVarId,
    sup: &Ty,
    range: TextRange,
    reason: ConstraintReason,
    vars: &mut TyVarStore,
    worklist: &mut Vec<Constraint>,
) {
    let lowers: Vec<Ty> = vars
        .get(var)
        .map(|data| data.lower.clone())
        .unwrap_or_default();
    vars.add_upper(var, sup.clone());
    for lower in lowers {
        worklist.push(Constraint {
            sub: lower,
            sup: sup.clone(),
            range,
            reason,
        });
    }
}

/// `sub <: Var`: record the lower bound and propagate to existing uppers.
fn bound_var_below(
    var: super::tyvar::TyVarId,
    sub: &Ty,
    range: TextRange,
    reason: ConstraintReason,
    vars: &mut TyVarStore,
    worklist: &mut Vec<Constraint>,
) {
    let uppers: Vec<Ty> = vars
        .get(var)
        .map(|data| data.upper.clone())
        .unwrap_or_default();
    vars.add_lower(var, sub.clone());
    for upper in uppers {
        worklist.push(Constraint {
            sub: sub.clone(),
            sup: upper,
            range,
            reason,
        });
    }
}

/// Split a structural pair into element constraints; shapes the solver does
/// not decompose defer to the residual ground re-check.
fn decompose(
    sub: Ty,
    sup: Ty,
    range: TextRange,
    reason: ConstraintReason,
    worklist: &mut Vec<Constraint>,
    residual: &mut Vec<Constraint>,
) {
    match (sub, sup) {
        (Ty::List(a), Ty::List(b)) | (Ty::Set(a), Ty::Set(b)) => {
            worklist.push(Constraint {
                sub: *a,
                sup: *b,
                range,
                reason,
            });
        }
        (Ty::Dict(ka, va), Ty::Dict(kb, vb)) => {
            worklist.push(Constraint {
                sub: *ka,
                sup: *kb,
                range,
                reason,
            });
            worklist.push(Constraint {
                sub: *va,
                sup: *vb,
                range,
                reason,
            });
        }
        (Ty::Tuple(given), Ty::Tuple(wanted)) if given.len() == wanted.len() => {
            for (a, b) in given.into_iter().zip(wanted) {
                worklist.push(Constraint {
                    sub: a,
                    sup: b,
                    range,
                    reason,
                });
            }
        }
        (Ty::Union(alts), sup) => {
            for alt in alts {
                worklist.push(Constraint {
                    sub: alt,
                    sup: sup.clone(),
                    range,
                    reason,
                });
            }
        }
        (sub, sup) => push_callable_or_residual(sub, sup, range, reason, worklist, residual),
    }
}

/// Callable variance split; anything else joins the residual set.
fn push_callable_or_residual(
    sub: Ty,
    sup: Ty,
    range: TextRange,
    reason: ConstraintReason,
    worklist: &mut Vec<Constraint>,
    residual: &mut Vec<Constraint>,
) {
    match (sub, sup) {
        (Ty::Callable(given_params, given_ret), Ty::Callable(wanted_params, wanted_ret))
            if given_params.len() == wanted_params.len()
                || given_params.is_empty()
                || wanted_params.is_empty() =>
        {
            // Contravariant parameters — skipped entirely when either side is
            // the gradual `Callable[..., R]` (empty parameter list), matching
            // `is_assignable_to`'s treatment.
            for (given, wanted) in given_params.into_iter().zip(wanted_params) {
                worklist.push(Constraint {
                    sub: wanted,
                    sup: given,
                    range,
                    reason,
                });
            }
            // Covariant return.
            worklist.push(Constraint {
                sub: *given_ret,
                sup: *wanted_ret,
                range,
                reason,
            });
        }
        (sub, sup) => residual.push(Constraint {
            sub,
            sup,
            range,
            reason,
        }),
    }
}

/// Ground leaf: delegate to the single subtyping authority.
fn leaf_check(
    sub: &Ty,
    sup: &Ty,
    range: TextRange,
    reason: ConstraintReason,
    vars: &TyVarStore,
    errors: &mut Vec<SolveError>,
) {
    let actual = sub.to_inferred(vars);
    let expected = sup.to_inferred(vars);
    if !actual.is_assignable_to(&expected) {
        errors.push(SolveError {
            sub: actual,
            sup: expected,
            range,
            reason,
        });
    }
}

/// Re-verify undecomposed constraints on their ground projections after all
/// bounds settled — variables resolve through deferred generalization first.
fn verify_residual(vars: &TyVarStore, residual: Vec<Constraint>, errors: &mut Vec<SolveError>) {
    for constraint in residual {
        let actual = constraint.sub.to_inferred(vars);
        let expected = constraint.sup.to_inferred(vars);
        if !actual.is_assignable_to(&expected) {
            errors.push(SolveError {
                sub: actual,
                sup: expected,
                range: constraint.range,
                reason: constraint.reason,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tyvar::Polarity;
    use super::*;
    use crate::types::LiteralValue;

    fn range() -> TextRange {
        TextRange::default()
    }

    /// [TYPEINF-TARGET-CONSTRAINTS]: `list[Literal[1]] <: list[int]`
    /// decomposes and passes through the numeric tower.
    #[test]
    fn structural_decomposition_reaches_ground_leaves() {
        let mut set = super::super::constraints::ConstraintSet::default();
        set.push(
            Ty::List(Box::new(Ty::Ground(InferredType::Literal(
                LiteralValue::Int(1),
            )))),
            Ty::List(Box::new(Ty::Ground(InferredType::Int))),
            range(),
            ConstraintReason::ExpectedType,
        );
        let solution = solve(TyVarStore::default(), set.into_vec());
        assert!(solution.errors.is_empty(), "{:?}", solution.errors);
    }

    /// An impossible ground constraint surfaces exactly one error.
    #[test]
    fn ground_mismatch_is_reported() {
        let mut set = super::super::constraints::ConstraintSet::default();
        set.push(
            Ty::Ground(InferredType::Str),
            Ty::Ground(InferredType::Int),
            range(),
            ConstraintReason::ExpectedType,
        );
        let solution = solve(TyVarStore::default(), set.into_vec());
        assert_eq!(solution.errors.len(), 1);
    }

    /// `Literal[1] <: Var` then `Var <: int` propagates: the literal bound is
    /// checked against the later upper bound, and the var still resolves to
    /// the precise `Literal[1]` (deferred generalization).
    #[test]
    fn var_bounds_propagate_and_stay_precise() {
        let mut vars = TyVarStore::default();
        let var = vars.fresh(Polarity::Output);
        let mut set = super::super::constraints::ConstraintSet::default();
        set.push(
            Ty::Ground(InferredType::Literal(LiteralValue::Int(1))),
            Ty::Var(var),
            range(),
            ConstraintReason::CollectionElement,
        );
        set.push(
            Ty::Var(var),
            Ty::Ground(InferredType::Int),
            range(),
            ConstraintReason::ExpectedType,
        );
        let solution = solve(vars, set.into_vec());
        assert!(solution.errors.is_empty(), "{:?}", solution.errors);
        assert_eq!(
            solution.vars.resolve(var),
            InferredType::Literal(LiteralValue::Int(1)),
            "generalization stays deferred: Literal[1] is not widened to int"
        );
    }

    /// A var bounded below by `str` and above by `int` is unsatisfiable.
    #[test]
    fn conflicting_var_bounds_error() {
        let mut vars = TyVarStore::default();
        let var = vars.fresh(Polarity::Output);
        let mut set = super::super::constraints::ConstraintSet::default();
        set.push(
            Ty::Ground(InferredType::Str),
            Ty::Var(var),
            range(),
            ConstraintReason::CollectionElement,
        );
        set.push(
            Ty::Var(var),
            Ty::Ground(InferredType::Int),
            range(),
            ConstraintReason::ExpectedType,
        );
        let solution = solve(vars, set.into_vec());
        assert_eq!(solution.errors.len(), 1, "{:?}", solution.errors);
    }

    /// Callable parameters check contravariantly, returns covariantly.
    #[test]
    fn callable_variance() {
        let sub = Ty::Callable(
            vec![Ty::Ground(InferredType::Float)],
            Box::new(Ty::Ground(InferredType::Bool)),
        );
        let sup = Ty::Callable(
            vec![Ty::Ground(InferredType::Int)],
            Box::new(Ty::Ground(InferredType::Int)),
        );
        let mut set = super::super::constraints::ConstraintSet::default();
        set.push(sub, sup, range(), ConstraintReason::CallArgument);
        let solution = solve(TyVarStore::default(), set.into_vec());
        assert!(
            solution.errors.is_empty(),
            "(float)->bool <: (int)->int must hold: {:?}",
            solution.errors
        );
    }
}
