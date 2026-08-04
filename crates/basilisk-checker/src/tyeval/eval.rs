//! Implements [TYPEINF-TARGET-TYPELEVEL] — the bounded, memoized,
//! call-by-need evaluator to weak head normal form.
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-TYPELEVEL
//!
//! - **Laziness**: aliases unfold only until an outermost constructor
//!   appears; conditional types normalize the scrutinee, decide the
//!   rewrite by assignability, and evaluate ONLY the taken arm — an
//!   untaken divergent arm never runs (call-by-need).
//! - **Fuel and depth bounds** with **memoization** of normalized results
//!   per application (TypeScript's instantiation-depth model).
//! - **The `Divergent` fallback**: running out of fuel/depth, an unknown
//!   alias, or an ill-kinded application yields [`Eval::Divergent`], which
//!   projects to the gradual `Unknown` — evaluation failure NEVER invents
//!   an error ([TYPEINF-TARGET-GRADUAL]).

use std::collections::HashMap;

use crate::types::InferredType;

use super::term::{AliasEnv, CondTerm, TypeTerm};

/// Fuel: total alias unfoldings one evaluation may perform.
const EVAL_FUEL: u32 = 256;
/// Depth: maximum nesting of constructors descended while normalizing.
const EVAL_DEPTH: u32 = 64;

/// A weak-head-normal-form outcome.
#[derive(Debug, Clone, PartialEq)]
pub enum Eval {
    /// Normalized to a head constructor (projected to [`InferredType`],
    /// with unevaluated sub-terms projected conservatively).
    Value(InferredType),
    /// Fuel/depth exhausted, unguarded shape, unknown alias, or ill-kinded
    /// application — the divergent sentinel. Projects to `Unknown`: never
    /// an invented error ([TYPEINF-TARGET-GRADUAL]).
    Divergent,
}

impl Eval {
    /// Project to the checker's type lattice.
    #[must_use]
    pub fn into_inferred(self) -> InferredType {
        match self {
            Eval::Value(ty) => ty,
            Eval::Divergent => InferredType::Unknown,
        }
    }
}

/// Evaluator state: fuel plus the `(alias, args)` application memo.
#[derive(Debug, Default)]
pub struct Evaluator {
    pub(super) fuel: u32,
    memo: HashMap<(String, String), Eval>,
}

impl Evaluator {
    /// A fresh evaluator with full fuel.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fuel: EVAL_FUEL,
            memo: HashMap::new(),
        }
    }

    /// Evaluate `term` to weak head normal form under `env`.
    pub fn evaluate(&mut self, env: &AliasEnv, term: &TypeTerm) -> Eval {
        self.eval_at(env, term, &[], 0)
    }

    /// Core: lazy unfolding with parameter substitution from `args`.
    fn eval_at(&mut self, env: &AliasEnv, term: &TypeTerm, args: &[TypeTerm], depth: u32) -> Eval {
        if depth > EVAL_DEPTH {
            return Eval::Divergent;
        }
        match term {
            TypeTerm::Ground(ty) => Eval::Value(ty.clone()),
            TypeTerm::Param(index) => match args.get(*index) {
                Some(argument) => self.eval_at(env, &argument.clone(), &[], depth + 1),
                None => Eval::Divergent,
            },
            TypeTerm::Alias(name, alias_args) => {
                self.eval_application(env, name, alias_args, args, depth)
            }
            // An unapplied operator is not a proper type: as a whnf demand
            // it is ill-kinded (`Kind::Operator`, not `Kind::Type`) unless
            // nullary, in which case it is an ordinary alias reference.
            TypeTerm::Op(name) => match env.get(name) {
                Some(def) if def.arity == 0 => self.eval_application(env, name, &[], args, depth),
                _ => Eval::Divergent,
            },
            TypeTerm::Apply(head, apply_args) => {
                self.eval_apply(env, head, apply_args, args, depth)
            }
            TypeTerm::Cond(cond) => self.eval_cond(env, cond, args, depth),
            TypeTerm::List(inner) => {
                let element = self.eval_at(env, inner, args, depth + 1).into_inferred();
                Eval::Value(InferredType::List(Box::new(element)))
            }
            TypeTerm::Set(inner) => {
                let element = self.eval_at(env, inner, args, depth + 1).into_inferred();
                Eval::Value(InferredType::Set(Box::new(element)))
            }
            TypeTerm::Dict(key, value) => {
                let key_ty = self.eval_at(env, key, args, depth + 1).into_inferred();
                let value_ty = self.eval_at(env, value, args, depth + 1).into_inferred();
                Eval::Value(InferredType::Dict(Box::new(key_ty), Box::new(value_ty)))
            }
            TypeTerm::Tuple(items) => {
                let elements = items
                    .iter()
                    .map(|item| self.eval_at(env, item, args, depth + 1).into_inferred())
                    .collect();
                Eval::Value(InferredType::Tuple(elements))
            }
            TypeTerm::Named(name, items) => {
                // A named constructor is already a whnf head; its arguments
                // project conservatively for display/assignability use.
                let _ = items;
                Eval::Value(InferredType::Named(name.clone()))
            }
            TypeTerm::Union(arms) => {
                let union = arms
                    .iter()
                    .map(|arm| self.eval_at(env, arm, args, depth + 1).into_inferred())
                    .fold(InferredType::Never, InferredType::union);
                Eval::Value(union)
            }
        }
    }

    /// Higher-order application: normalize the head to an operator value
    /// (a [`TypeTerm::Op`], possibly reached through a parameter), then
    /// unfold it. Applying a non-operator or mismatching the kind's arity
    /// is ill-kinded → [`Eval::Divergent`] (gradual, never an error).
    fn eval_apply(
        &mut self,
        env: &AliasEnv,
        head: &TypeTerm,
        apply_args: &[TypeTerm],
        outer_args: &[TypeTerm],
        depth: u32,
    ) -> Eval {
        let resolved_head = match head {
            TypeTerm::Param(index) => match outer_args.get(*index) {
                Some(bound) => bound.clone(),
                None => return Eval::Divergent,
            },
            other => other.clone(),
        };
        match resolved_head {
            TypeTerm::Op(name) | TypeTerm::Alias(name, _) => {
                self.eval_application(env, &name, apply_args, outer_args, depth)
            }
            _ => Eval::Divergent,
        }
    }

    /// A conditional type: force the scrutinee to whnf, decide
    /// `scrutinee <: against`, then evaluate ONLY the taken arm
    /// (call-by-need). A union scrutinee distributes over its arms — the
    /// TypeScript/PEP 827 distribution rule — each arm rewritten lazily.
    /// An undecidable scrutinee (gradual `Unknown`) makes the whole
    /// conditional gradual rather than guessing a branch.
    fn eval_cond(
        &mut self,
        env: &AliasEnv,
        cond: &CondTerm,
        args: &[TypeTerm],
        depth: u32,
    ) -> Eval {
        let Some(scrutinee) = self.force_value(env, &cond.scrutinee, args, depth) else {
            return Eval::Divergent;
        };
        if let InferredType::Union(members) = scrutinee {
            return self.distribute_cond(env, cond, members, args, depth);
        }
        let Some(against) = self.force_value(env, &cond.against, args, depth) else {
            return Eval::Divergent;
        };
        if matches!(scrutinee, InferredType::Unknown) {
            // Cannot decide the rewrite gradually — do not guess a branch.
            return Eval::Divergent;
        }
        let arm = if scrutinee.is_assignable_to(&against) {
            &cond.then_arm
        } else {
            &cond.else_arm
        };
        self.eval_at(env, arm, args, depth + 1)
    }

    /// Force a subterm one level deeper to a whnf value; `None` signals
    /// divergence for the caller to short-circuit.
    fn force_value(
        &mut self,
        env: &AliasEnv,
        term: &TypeTerm,
        args: &[TypeTerm],
        depth: u32,
    ) -> Option<InferredType> {
        match self.eval_at(env, term, args, depth + 1) {
            Eval::Value(ty) => Some(ty),
            Eval::Divergent => None,
        }
    }

    /// Distribution of a conditional over a union scrutinee: rewrite each
    /// member independently and union the results.
    fn distribute_cond(
        &mut self,
        env: &AliasEnv,
        cond: &CondTerm,
        members: Vec<InferredType>,
        args: &[TypeTerm],
        depth: u32,
    ) -> Eval {
        let mut result = InferredType::Never;
        for member in members {
            let member_cond = CondTerm {
                scrutinee: TypeTerm::Ground(member),
                against: cond.against.clone(),
                then_arm: cond.then_arm.clone(),
                else_arm: cond.else_arm.clone(),
            };
            match self.eval_cond(env, &member_cond, args, depth) {
                Eval::Value(ty) => result = InferredType::union(result, ty),
                Eval::Divergent => return Eval::Divergent,
            }
        }
        Eval::Value(result)
    }

    /// Unfold one alias application, memoized per `(alias, args)`.
    fn eval_application(
        &mut self,
        env: &AliasEnv,
        name: &str,
        alias_args: &[TypeTerm],
        outer_args: &[TypeTerm],
        depth: u32,
    ) -> Eval {
        let key = (name.to_owned(), format!("{alias_args:?}|{outer_args:?}"));
        if let Some(cached) = self.memo.get(&key) {
            return cached.clone();
        }
        if self.fuel == 0 {
            return Eval::Divergent;
        }
        self.fuel -= 1;

        let Some(def) = env.get(name) else {
            return Eval::Divergent;
        };
        // Kind check: the application must saturate the operator exactly.
        if def.arity != alias_args.len() {
            return Eval::Divergent;
        }
        // Substitute the application's arguments (resolving any outer
        // parameters lazily) and unfold the body one step.
        let substituted: Vec<TypeTerm> = alias_args
            .iter()
            .map(|arg| substitute(arg, outer_args))
            .collect();
        let body = def.body.clone();
        let result = self.eval_at(env, &body, &substituted, depth + 1);
        let _ = self.memo.insert(key, result.clone());
        result
    }
}

/// Replace [`TypeTerm::Param`] references with `args` (lazy: nested alias
/// applications keep their own bodies unexpanded).
fn substitute(term: &TypeTerm, args: &[TypeTerm]) -> TypeTerm {
    match term {
        TypeTerm::Param(index) => args
            .get(*index)
            .cloned()
            .unwrap_or(TypeTerm::Ground(InferredType::Unknown)),
        TypeTerm::Alias(name, alias_args) => TypeTerm::Alias(
            name.clone(),
            alias_args.iter().map(|a| substitute(a, args)).collect(),
        ),
        TypeTerm::Op(name) => TypeTerm::Op(name.clone()),
        TypeTerm::Apply(head, apply_args) => TypeTerm::Apply(
            Box::new(substitute(head, args)),
            apply_args.iter().map(|a| substitute(a, args)).collect(),
        ),
        TypeTerm::Cond(cond) => TypeTerm::Cond(Box::new(CondTerm {
            scrutinee: substitute(&cond.scrutinee, args),
            against: substitute(&cond.against, args),
            then_arm: substitute(&cond.then_arm, args),
            else_arm: substitute(&cond.else_arm, args),
        })),
        TypeTerm::List(inner) => TypeTerm::List(Box::new(substitute(inner, args))),
        TypeTerm::Set(inner) => TypeTerm::Set(Box::new(substitute(inner, args))),
        TypeTerm::Dict(key, value) => TypeTerm::Dict(
            Box::new(substitute(key, args)),
            Box::new(substitute(value, args)),
        ),
        TypeTerm::Tuple(items) => {
            TypeTerm::Tuple(items.iter().map(|i| substitute(i, args)).collect())
        }
        TypeTerm::Named(name, items) => TypeTerm::Named(
            name.clone(),
            items.iter().map(|i| substitute(i, args)).collect(),
        ),
        TypeTerm::Union(arms) => {
            TypeTerm::Union(arms.iter().map(|a| substitute(a, args)).collect())
        }
        TypeTerm::Ground(_) => term.clone(),
    }
}

#[cfg(test)]
#[path = "eval/tests.rs"]
mod tests;
