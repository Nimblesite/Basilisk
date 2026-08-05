//! Tests for [`super`] — whnf evaluation, memoization, call-by-need
//! laziness, and the gradual guarantee ([TYPEINF-TARGET-GRADUAL]).
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-TYPELEVEL

use super::super::term::AliasDef;
use super::*;

fn int() -> TypeTerm {
    TypeTerm::Ground(InferredType::Int)
}

fn str_ty() -> TypeTerm {
    TypeTerm::Ground(InferredType::Str)
}

/// Arrange: an env holding the accepted 1-ary `wrap` operator
/// (`type wrap[T] = list[T]`).
fn env_with_wrap() -> AliasEnv {
    let mut env = AliasEnv::default();
    assert!(env.insert(
        "wrap",
        AliasDef {
            arity: 1,
            body: TypeTerm::List(Box::new(TypeTerm::Param(0))),
        },
    ));
    env
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
    let env = env_with_wrap();
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
    let mut env = env_with_wrap();
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
    let env = env_with_wrap();
    let wrong_arity = TypeTerm::Alias("wrap".to_owned(), vec![int(), int()]);
    assert_eq!(
        Evaluator::new().evaluate(&env, &wrong_arity),
        Eval::Divergent
    );

    let apply_ground = TypeTerm::Apply(Box::new(int()), vec![int()]);
    assert_eq!(
        Evaluator::new().evaluate(&env, &apply_ground),
        Eval::Divergent
    );

    let unapplied_operator = TypeTerm::Op("wrap".to_owned());
    assert_eq!(
        Evaluator::new().evaluate(&env, &unapplied_operator),
        Eval::Divergent,
        "an unapplied Type → Type operator is not a proper type"
    );
}
