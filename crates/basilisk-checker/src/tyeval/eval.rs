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
            TypeTerm::Apply(head, apply_args) => self.eval_apply(env, head, apply_args, args, depth),
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
    fn eval_cond(&mut self, env: &AliasEnv, cond: &CondTerm, args: &[TypeTerm], depth: u32) -> Eval {
        let scrutinee = match self.eval_at(env, &cond.scrutinee, args, depth + 1) {
            Eval::Value(ty) => ty,
            Eval::Divergent => return Eval::Divergent,
        };
        if let InferredType::Union(members) = scrutinee {
            return self.distribute_cond(env, cond, members, args, depth);
        }
        let against = match self.eval_at(env, &cond.against, args, depth + 1) {
            Eval::Value(ty) => ty,
            Eval::Divergent => return Eval::Divergent,
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
mod tests {
    use super::super::term::AliasDef;
    use super::*;

    fn int() -> TypeTerm {
        TypeTerm::Ground(InferredType::Int)
    }

    fn str_ty() -> TypeTerm {
        TypeTerm::Ground(InferredType::Str)
    }

    /// A mapped-type operator (`type Pair[T] = tuple[T, T]`) applies lazily.
    #[test]
    fn mapped_alias_applies_arguments() {
        let mut env = AliasEnv::default();
        assert!(env.insert(
            "pair",
            AliasDef {
                arity: 1,
                body: TypeTerm::Tuple(vec![TypeTerm::Param(0), TypeTerm::Param(0)]),
            },
        ));
        let mut evaluator = Evaluator::new();
        let result = evaluator.evaluate(&env, &TypeTerm::Alias("pair".to_owned(), vec![int()]));
        assert_eq!(
            result,
            Eval::Value(InferredType::Tuple(vec![
                InferredType::Int,
                InferredType::Int
            ]))
        );
    }

    /// A guarded recursive alias (`type Json = int | list[Json]`) evaluates
    /// to whnf — the recursive arm normalizes under fuel without expanding
    /// forever.
    #[test]
    fn guarded_recursion_reaches_whnf() {
        let mut env = AliasEnv::default();
        assert!(env.insert(
            "json",
            AliasDef {
                arity: 0,
                body: TypeTerm::Union(vec![
                    int(),
                    TypeTerm::List(Box::new(TypeTerm::Alias("json".to_owned(), Vec::new()))),
                ]),
            },
        ));
        let mut evaluator = Evaluator::new();
        let result = evaluator
            .evaluate(&env, &TypeTerm::Alias("json".to_owned(), Vec::new()))
            .into_inferred();
        // The head is a union of int and list[...]; the recursive interior
        // bottoms out gradually rather than diverging.
        assert!(InferredType::Int.is_assignable_to(&result));
        assert!(
            InferredType::List(Box::new(InferredType::Unknown)).is_assignable_to(&result),
            "list arm must be present: {result:?}"
        );
    }

    /// The guardedness acceptance condition rejects `type X = X` up front.
    #[test]
    fn unguarded_recursion_is_rejected() {
        let mut env = AliasEnv::default();
        assert!(!env.insert(
            "x",
            AliasDef {
                arity: 0,
                body: TypeTerm::Alias("x".to_owned(), Vec::new()),
            },
        ));
        // Union arms do not guard either: `type X = int | X`.
        assert!(!env.insert(
            "x",
            AliasDef {
                arity: 0,
                body: TypeTerm::Union(vec![int(), TypeTerm::Alias("x".to_owned(), Vec::new())]),
            },
        ));
    }

    /// Unknown aliases and fuel exhaustion produce `Divergent`, which
    /// projects to the gradual `Unknown` — never an error
    /// ([TYPEINF-TARGET-GRADUAL]).
    #[test]
    fn divergence_projects_to_unknown() {
        let env = AliasEnv::default();
        let mut evaluator = Evaluator::new();
        let result = evaluator.evaluate(&env, &TypeTerm::Alias("missing".to_owned(), Vec::new()));
        assert_eq!(result, Eval::Divergent);
        assert_eq!(result.into_inferred(), InferredType::Unknown);
    }

    /// Memoization: re-evaluating the same application does not spend fuel
    /// again (the second call is a cache hit even with zero fuel left).
    #[test]
    fn applications_are_memoized() {
        let mut env = AliasEnv::default();
        assert!(env.insert(
            "wrap",
            AliasDef {
                arity: 1,
                body: TypeTerm::List(Box::new(TypeTerm::Param(0))),
            },
        ));
        let mut evaluator = Evaluator::new();
        let term = TypeTerm::Alias("wrap".to_owned(), vec![int()]);
        let first = evaluator.evaluate(&env, &term);
        evaluator.fuel = 0;
        let second = evaluator.evaluate(&env, &term);
        assert_eq!(first, second, "memo hit must not need fuel");
    }

    /// An escape-hatch alias (`insert_undecidable`) runs under fuel and
    /// truncates to the gradual `Unknown` instead of looping — the
    /// gradual guarantee on truncated evaluation.
    #[test]
    fn undecidable_alias_truncates_gradually() {
        let mut env = AliasEnv::default();
        env.insert_undecidable(
            "x",
            AliasDef {
                arity: 0,
                body: TypeTerm::Alias("x".to_owned(), Vec::new()),
            },
        );
        let result = Evaluator::new().evaluate(&env, &TypeTerm::Alias("x".to_owned(), Vec::new()));
        assert_eq!(result, Eval::Divergent);
        assert_eq!(result.into_inferred(), InferredType::Unknown);
    }

    /// Dict/Set constructors normalize their components.
    #[test]
    fn dict_and_set_constructors_normalize() {
        let mut env = AliasEnv::default();
        assert!(env.insert(
            "m",
            AliasDef {
                arity: 0,
                body: TypeTerm::Dict(Box::new(str_ty()), Box::new(TypeTerm::Set(Box::new(int())))),
            },
        ));
        let result = Evaluator::new()
            .evaluate(&env, &TypeTerm::Alias("m".to_owned(), Vec::new()))
            .into_inferred();
        assert_eq!(
            result,
            InferredType::Dict(
                Box::new(InferredType::Str),
                Box::new(InferredType::Set(Box::new(InferredType::Int)))
            )
        );
    }

    /// Conditional types rewrite on assignability and are call-by-need:
    /// the untaken arm is a divergent (unknown) alias and is never forced.
    #[test]
    fn conditional_rewrites_lazily() {
        let env = AliasEnv::default();
        let divergent_arm = TypeTerm::Alias("missing".to_owned(), Vec::new());
        let taken = TypeTerm::Cond(Box::new(CondTerm {
            scrutinee: int(),
            against: int(),
            then_arm: str_ty(),
            else_arm: divergent_arm.clone(),
        }));
        assert_eq!(
            Evaluator::new().evaluate(&env, &taken),
            Eval::Value(InferredType::Str),
            "then-arm taken; divergent else-arm must never be forced"
        );

        let not_taken = TypeTerm::Cond(Box::new(CondTerm {
            scrutinee: str_ty(),
            against: int(),
            then_arm: divergent_arm,
            else_arm: int(),
        }));
        assert_eq!(
            Evaluator::new().evaluate(&env, &not_taken),
            Eval::Value(InferredType::Int),
            "else-arm taken; divergent then-arm must never be forced"
        );
    }

    /// An `Unknown` scrutinee cannot decide the rewrite: the conditional
    /// is gradual (`Divergent` → `Unknown`), never a guessed branch.
    #[test]
    fn conditional_on_unknown_scrutinee_is_gradual() {
        let env = AliasEnv::default();
        let cond = TypeTerm::Cond(Box::new(CondTerm {
            scrutinee: TypeTerm::Ground(InferredType::Unknown),
            against: int(),
            then_arm: int(),
            else_arm: str_ty(),
        }));
        assert_eq!(Evaluator::new().evaluate(&env, &cond), Eval::Divergent);
    }

    /// A union scrutinee distributes: `(int | str) extends int ? A : B`
    /// rewrites each member independently and unions the results.
    #[test]
    fn conditional_distributes_over_union_scrutinee() {
        let env = AliasEnv::default();
        let cond = TypeTerm::Cond(Box::new(CondTerm {
            scrutinee: TypeTerm::Union(vec![int(), str_ty()]),
            against: int(),
            then_arm: TypeTerm::Ground(InferredType::Bool),
            else_arm: TypeTerm::Ground(InferredType::None_),
        }));
        let result = Evaluator::new().evaluate(&env, &cond).into_inferred();
        assert!(InferredType::Bool.is_assignable_to(&result), "{result:?}");
        assert!(InferredType::None_.is_assignable_to(&result), "{result:?}");
    }

    /// Mapped types are first-class `Type → Type` operators: an operator
    /// passed as an argument applies through `Apply` (higher-order).
    #[test]
    fn operator_argument_applies_higher_order() {
        let mut env = AliasEnv::default();
        assert!(env.insert(
            "wrap",
            AliasDef {
                arity: 1,
                body: TypeTerm::List(Box::new(TypeTerm::Param(0))),
            },
        ));
        // type ApplyToInt[F] = F[int]  — F is an operator-kinded parameter.
        assert!(env.insert(
            "apply_to_int",
            AliasDef {
                arity: 1,
                body: TypeTerm::Apply(Box::new(TypeTerm::Param(0)), vec![int()]),
            },
        ));
        let term = TypeTerm::Alias(
            "apply_to_int".to_owned(),
            vec![TypeTerm::Op("wrap".to_owned())],
        );
        assert_eq!(
            Evaluator::new().evaluate(&env, &term),
            Eval::Value(InferredType::List(Box::new(InferredType::Int)))
        );
    }

    /// Kind errors are gradual: applying a proper type, or applying an
    /// operator at the wrong arity, yields `Divergent` → `Unknown`,
    /// never an invented error.
    #[test]
    fn ill_kinded_applications_are_gradual() {
        let mut env = AliasEnv::default();
        assert!(env.insert(
            "wrap",
            AliasDef {
                arity: 1,
                body: TypeTerm::List(Box::new(TypeTerm::Param(0))),
            },
        ));
        let wrong_arity = TypeTerm::Alias("wrap".to_owned(), vec![int(), int()]);
        assert_eq!(Evaluator::new().evaluate(&env, &wrong_arity), Eval::Divergent);

        let apply_ground = TypeTerm::Apply(Box::new(int()), vec![int()]);
        assert_eq!(Evaluator::new().evaluate(&env, &apply_ground), Eval::Divergent);

        let unapplied_operator = TypeTerm::Op("wrap".to_owned());
        assert_eq!(
            Evaluator::new().evaluate(&env, &unapplied_operator),
            Eval::Divergent,
            "an unapplied Type → Type operator is not a proper type"
        );
    }
}
