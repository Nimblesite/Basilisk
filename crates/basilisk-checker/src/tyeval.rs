//! Implements [TYPEINF-TARGET] and [TYPEINF-TARGET-TYPELEVEL] Stage 3
//! groundwork. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST
//! ("Stage 3 — type-level evaluation groundwork").
//!
//! Python's type-hint sublanguage is Turing-complete (Roth,
//! <https://arxiv.org/abs/2208.14755>), so recursive/parameterised type
//! aliases must be *evaluated*, not expanded eagerly. This module is the
//! normalization-by-evaluation core:
//!
//! - [`TypeTerm`] — the type-level term language: ground types, alias
//!   references with arguments (kind `Type → Type` operators — PEP 695
//!   `type Pair[T] = tuple[T, T]`), and parameter references;
//! - [`evaluate`] — lazy unfolding to **weak head normal form**: aliases
//!   unfold only until an outermost constructor appears; arguments
//!   substitute lazily (mapped-type applications rewrite on demand);
//! - **fuel and depth bounds** with **memoization** of normalized results
//!   per `(alias, argument)` application;
//! - the **`Divergent` fallback**: running out of fuel, unguarded
//!   recursion, or an unknown alias yields [`Eval::Divergent`], which
//!   projects to the gradual `Unknown` — evaluation failure NEVER invents
//!   an error ([TYPEINF-TARGET-GRADUAL]);
//! - a **guardedness acceptance condition** (the Paterson/Coverage-style
//!   analogue): an alias whose recursive self-reference is not under a
//!   constructor (`type X = X`) is rejected up front, with the recursion
//!   depth cap as the escape hatch for accepted-but-deep definitions.

use std::collections::HashMap;

use crate::types::InferredType;

/// Fuel: total alias unfoldings one evaluation may perform.
const EVAL_FUEL: u32 = 256;
/// Depth: maximum nesting of constructors descended while normalizing.
const EVAL_DEPTH: u32 = 64;

/// A type-level term.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeTerm {
    /// A ground type — already a value.
    Ground(InferredType),
    /// A reference to an alias, possibly applied: `Pair[int]`, `Json`.
    Alias(String, Vec<TypeTerm>),
    /// A reference to the enclosing alias's parameter by index.
    Param(usize),
    /// `list[T]` at the type level (constructor — a whnf head).
    List(Box<TypeTerm>),
    /// `T | U` at the type level.
    Union(Vec<TypeTerm>),
    /// `tuple[T, ..]` at the type level.
    Tuple(Vec<TypeTerm>),
}

/// One alias definition: `type Name[P0, P1, ..] = body`.
#[derive(Debug, Clone, PartialEq)]
pub struct AliasDef {
    /// Number of type parameters.
    pub arity: usize,
    /// The right-hand side, with [`TypeTerm::Param`] for parameters.
    pub body: TypeTerm,
}

/// The alias environment (one module's `type` statements).
#[derive(Debug, Clone, Default)]
pub struct AliasEnv {
    aliases: HashMap<String, AliasDef>,
}

impl AliasEnv {
    /// Register an alias; rejects (returns `false`, leaving the environment
    /// unchanged) definitions that fail the guardedness acceptance
    /// condition — a recursive self-reference not under a constructor
    /// (`type X = X`, `type X = X | int` at the top level of a union arm is
    /// GUARDED only through constructors, so plain `X` arms are rejected).
    pub fn insert(&mut self, name: &str, def: AliasDef) -> bool {
        if !recursion_is_guarded(name, &def.body, false) {
            return false;
        }
        let _ = self.aliases.insert(name.to_owned(), def);
        true
    }

    /// Look up an alias.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&AliasDef> {
        self.aliases.get(name)
    }
}

/// The guardedness acceptance condition: every self-reference must sit
/// beneath at least one constructor. `under_constructor` tracks whether the
/// walk has passed through `List`/`Tuple` (unions do NOT guard — a union arm
/// unfolds at the same level).
fn recursion_is_guarded(name: &str, term: &TypeTerm, under_constructor: bool) -> bool {
    match term {
        TypeTerm::Alias(alias, args) => {
            (alias != name || under_constructor)
                && args
                    .iter()
                    .all(|arg| recursion_is_guarded(name, arg, under_constructor))
        }
        TypeTerm::List(inner) => recursion_is_guarded(name, inner, true),
        TypeTerm::Tuple(items) => items
            .iter()
            .all(|item| recursion_is_guarded(name, item, true)),
        TypeTerm::Union(arms) => arms
            .iter()
            .all(|arm| recursion_is_guarded(name, arm, under_constructor)),
        TypeTerm::Ground(_) | TypeTerm::Param(_) => true,
    }
}

/// A weak-head-normal-form outcome.
#[derive(Debug, Clone, PartialEq)]
pub enum Eval {
    /// Normalized to a head constructor (projected to [`InferredType`],
    /// with unevaluated sub-terms projected conservatively).
    Value(InferredType),
    /// Fuel/depth exhausted, unguarded shape, or unknown alias — the
    /// divergent sentinel. Projects to `Unknown`: never an invented error
    /// ([TYPEINF-TARGET-GRADUAL]).
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
    fuel: u32,
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
                self.eval_alias(env, name, alias_args, args, depth)
            }
            TypeTerm::List(inner) => {
                let element = self.eval_at(env, inner, args, depth + 1).into_inferred();
                Eval::Value(InferredType::List(Box::new(element)))
            }
            TypeTerm::Tuple(items) => {
                let elements = items
                    .iter()
                    .map(|item| self.eval_at(env, item, args, depth + 1).into_inferred())
                    .collect();
                Eval::Value(InferredType::Tuple(elements))
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

    /// Unfold one alias application, memoized per `(alias, args)`.
    fn eval_alias(
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
        TypeTerm::List(inner) => TypeTerm::List(Box::new(substitute(inner, args))),
        TypeTerm::Tuple(items) => {
            TypeTerm::Tuple(items.iter().map(|i| substitute(i, args)).collect())
        }
        TypeTerm::Union(arms) => {
            TypeTerm::Union(arms.iter().map(|a| substitute(a, args)).collect())
        }
        TypeTerm::Ground(_) => term.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int() -> TypeTerm {
        TypeTerm::Ground(InferredType::Int)
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
}
