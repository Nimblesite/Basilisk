//! External API regression for [TYPEINF-TARGET] and
//! [TYPEINF-TARGET-TYPELEVEL].

use basilisk_checker::{
    tyeval::{AliasDef, AliasEnv, Eval, Evaluator, TypeTerm},
    types::InferredType,
};

#[test]
fn bounded_alias_evaluation_and_gradual_divergence_are_public() {
    let mut aliases = AliasEnv::default();
    assert!(aliases.insert(
        "Pair",
        AliasDef {
            arity: 1,
            body: TypeTerm::Tuple(vec![TypeTerm::Param(0), TypeTerm::Param(0)]),
        },
    ));

    let mut evaluator = Evaluator::new();
    let pair_of_int = TypeTerm::Alias("Pair".to_owned(), vec![TypeTerm::Ground(InferredType::Int)]);
    assert_eq!(
        evaluator.evaluate(&aliases, &pair_of_int),
        Eval::Value(InferredType::Tuple(vec![
            InferredType::Int,
            InferredType::Int,
        ])),
    );

    assert!(aliases.insert(
        "Left",
        AliasDef {
            arity: 0,
            body: TypeTerm::Alias("Right".to_owned(), Vec::new()),
        },
    ));
    assert!(aliases.insert(
        "Right",
        AliasDef {
            arity: 0,
            body: TypeTerm::Alias("Left".to_owned(), Vec::new()),
        },
    ));

    let bounded_result =
        Evaluator::new().evaluate(&aliases, &TypeTerm::Alias("Left".to_owned(), Vec::new()));
    assert_eq!(bounded_result, Eval::Divergent);
    assert_eq!(bounded_result.into_inferred(), InferredType::Unknown);
}
