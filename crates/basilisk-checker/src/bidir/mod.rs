//! Implements [TYPEINF-TARGET-BIDIRECTIONAL] / [TYPEINF-TARGET-CONSTRAINTS].
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET.
//!
//! The Stage 0 bidirectional + constraint inference engine
//! ([NARROWPLAN-CHECKLIST], docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md):
//!
//! - [`engine`] / [`check`] — the two-mode core: `synth(e) → τ` bottom-up and
//!   `check(e, τ)` top-down, `check` as the primary driver;
//! - [`constraints`] — the generation pass's `τ₁ <: τ₂` obligations;
//! - [`solve`] — the separate solver (two-stage Pottier–Rémy split);
//! - [`tyvar`] — bounded, polar type variables with deferred generalization;
//! - [`ty`] — the solver language over the existing [`crate::types::InferredType`],
//!   whose [`crate::types::InferredType::is_assignable_to`] remains the single
//!   ground-subtyping authority.
//!
//! Salsa note ([TYPEINF-TARGET-INCREMENTAL]): the engine is a pure function of
//! one module's AST — it reads no other file and no global state — so running
//! it inside the existing file-level tracked queries
//! ([`crate::incremental::checked_file`]) adds **zero** new Salsa dependency
//! edges. Expected-type threading therefore cannot balloon dependency growth
//! until Stage 1 introduces finer-grained queries, where the checklist's
//! peek-ahead fallback would be reconsidered per construct.

pub mod builtins;
pub mod check;
pub mod constraints;
pub mod engine;
pub mod generics;
pub mod solve;
pub mod ty;
pub mod tyvar;

pub use constraints::{Constraint, ConstraintReason, ConstraintSet};
pub use engine::BidirEngine;
pub use generics::{DeclaredVar, DeclaredVarKind, GenericEnv, Resolution, SolvedValue, VarDefault};
pub use solve::{solve, Solution, SolveError};
pub use ty::Ty;
pub use tyvar::{Polarity, TyVarId, TyVarStore};

#[cfg(test)]
mod tests {
    #![expect(
        clippy::expect_used,
        reason = "test-only parsing of fixed, known-valid expression fixtures"
    )]

    use std::collections::HashMap;

    use crate::types::{InferredType, LiteralValue};

    use super::{BidirEngine, Solution, Ty};

    /// Parse a single Python expression for engine tests.
    fn parse_expr(source: &str) -> ruff_python_ast::ModExpression {
        ruff_python_parser::parse_expression(source)
            .map(ruff_python_parser::Parsed::into_syntax)
            .expect("test expression must parse")
    }

    /// Synthesize a type for `source` under `globals`, then solve.
    fn synth_and_solve(source: &str, globals: HashMap<String, Ty>) -> (InferredType, Solution) {
        let module = parse_expr(source);
        let mut engine = BidirEngine::new(globals);
        let ty = engine.synth(&module.body);
        let solution = engine.finish();
        (ty.to_inferred(&solution.vars), solution)
    }

    /// Check `source` against an annotation-shaped expectation, then solve.
    fn check_and_solve(source: &str, expected: &InferredType) -> Solution {
        let module = parse_expr(source);
        let mut engine = BidirEngine::new(HashMap::new());
        engine.check(&module.body, &Ty::from_inferred(expected));
        engine.finish()
    }

    /// [TYPEINF-TARGET-CONSTRAINTS]: `[1]` synthesizes to
    /// `list[Literal[1]]` via a bounded var — generalization stays deferred,
    /// never eagerly widened to `list[int]`.
    #[test]
    fn list_literal_defers_generalization() {
        let (inferred, solution) = synth_and_solve("[1]", HashMap::new());
        assert!(solution.errors.is_empty(), "{:?}", solution.errors);
        assert_eq!(
            inferred,
            InferredType::List(Box::new(InferredType::Literal(LiteralValue::Int(1))))
        );
    }

    /// [TYPEINF-TARGET-BIDIRECTIONAL]: checking `[1, "x"]` against
    /// `list[int]` threads the element expectation inward and reports exactly
    /// the string element as unsatisfiable.
    #[test]
    fn checked_list_threads_element_expectation() {
        let expected = InferredType::List(Box::new(InferredType::Int));
        let ok = check_and_solve("[1, 2]", &expected);
        assert!(ok.errors.is_empty(), "{:?}", ok.errors);

        let bad = check_and_solve("[1, \"x\"]", &expected);
        assert_eq!(bad.errors.len(), 1, "{:?}", bad.errors);
    }

    /// Nested containers thread context all the way down.
    #[test]
    fn nested_container_context_threads_inward() {
        let expected =
            InferredType::List(Box::new(InferredType::List(Box::new(InferredType::Int))));
        let ok = check_and_solve("[[1], [2, 3]]", &expected);
        assert!(ok.errors.is_empty(), "{:?}", ok.errors);

        let bad = check_and_solve("[[1], [\"x\"]]", &expected);
        assert_eq!(bad.errors.len(), 1, "{:?}", bad.errors);
    }

    /// [TYPEINF-TARGET-BIDIRECTIONAL]: a lambda checked against
    /// `Callable[[int], int]` binds its parameter to `int` and checks the
    /// body under that binding.
    #[test]
    fn checked_lambda_binds_expected_parameter_types() {
        let expected = InferredType::Callable(crate::types::CallableInfo {
            param_types: vec![InferredType::Int],
            return_type: Box::new(InferredType::Int),
        });
        let ok = check_and_solve("lambda x: x + 1", &expected);
        assert!(ok.errors.is_empty(), "{:?}", ok.errors);

        let bad_ret = InferredType::Callable(crate::types::CallableInfo {
            param_types: vec![InferredType::Int],
            return_type: Box::new(InferredType::Str),
        });
        let bad = check_and_solve("lambda x: x + 1", &bad_ret);
        assert_eq!(bad.errors.len(), 1, "{:?}", bad.errors);
    }

    /// Call arguments receive expected types from the callee's declared
    /// parameters — context threads into call arguments.
    #[test]
    fn call_arguments_receive_declared_parameter_context() {
        let callee = Ty::from_inferred(&InferredType::Callable(crate::types::CallableInfo {
            param_types: vec![InferredType::List(Box::new(InferredType::Int))],
            return_type: Box::new(InferredType::Bool),
        }));
        let globals: HashMap<String, Ty> = [("f".to_owned(), callee)].into_iter().collect();

        let (ret, ok) = synth_and_solve("f([1, 2])", globals.clone());
        assert!(ok.errors.is_empty(), "{:?}", ok.errors);
        assert_eq!(ret, InferredType::Bool);

        let (_, bad) = synth_and_solve("f([\"x\"])", globals);
        assert_eq!(bad.errors.len(), 1, "{:?}", bad.errors);
    }

    /// Comprehensions bind their targets from the iterable and check the
    /// element expression against the expected element type.
    #[test]
    fn checked_comprehension_threads_element_expectation() {
        let expected = InferredType::List(Box::new(InferredType::Int));
        let ok = check_and_solve("[x for x in [1, 2]]", &expected);
        assert!(ok.errors.is_empty(), "{:?}", ok.errors);

        let bad = check_and_solve("[\"s\" for x in [1, 2]]", &expected);
        assert_eq!(bad.errors.len(), 1, "{:?}", bad.errors);
    }

    /// Every expression node synthesizes *something* without panicking —
    /// unsupported shapes stay `Unknown` ([TYPEINF-EXCEEDS-NOUNKNOWN]).
    #[test]
    fn synth_is_total_over_expression_nodes() {
        let sources = [
            "await x",
            "x.attr",
            "x[0]",
            "x[0:2]",
            "(yield)",
            "(*x, 1)",
            "{**base, \"k\": 1}",
            "f\"{x}!\"",
            "x if x else y",
            "(n := 3)",
            "not x",
            "-2",
            "a @ b",
            "{i: str(i) for i in [1]}",
            "(i for i in [1])",
            "...",
        ];
        for source in sources {
            let (inferred, solution) = synth_and_solve(source, HashMap::new());
            assert!(
                solution.errors.is_empty(),
                "{source}: unexpected errors {:?}",
                solution.errors
            );
            let _ = inferred;
        }
    }

    /// A ternary in checked position checks both branches; a mismatched
    /// branch is caught even when the other satisfies the expectation.
    #[test]
    fn ternary_checks_both_branches() {
        let expected = InferredType::Int;
        let bad = check_and_solve("1 if c else \"x\"", &expected);
        assert_eq!(bad.errors.len(), 1, "{:?}", bad.errors);
    }

    /// Walrus in checked position: value checks against the expectation and
    /// the bound name is visible afterwards (within one expression).
    #[test]
    fn walrus_checks_value_against_expectation() {
        let bad = check_and_solve("(n := \"s\")", &InferredType::Int);
        assert_eq!(bad.errors.len(), 1, "{:?}", bad.errors);
    }

    /// Union expectations route a literal to its single matching alternative.
    #[test]
    fn union_expectation_picks_the_structural_match() {
        let expected = InferredType::Union(vec![
            InferredType::List(Box::new(InferredType::Int)),
            InferredType::None_,
        ]);
        let ok = check_and_solve("[1]", &expected);
        assert!(ok.errors.is_empty(), "{:?}", ok.errors);

        let bad = check_and_solve("[\"x\"]", &expected);
        assert_eq!(bad.errors.len(), 1, "{:?}", bad.errors);
    }

    /// Empty containers satisfy any element expectation — no fabricated
    /// errors from `list[Never]` ([TYPEINF-TARGET-GRADUAL]).
    #[test]
    fn empty_containers_never_error() {
        for (source, expected) in [
            ("[]", InferredType::List(Box::new(InferredType::Int))),
            (
                "{}",
                InferredType::Dict(Box::new(InferredType::Str), Box::new(InferredType::Int)),
            ),
        ] {
            let solution = check_and_solve(source, &expected);
            assert!(
                solution.errors.is_empty(),
                "{source}: {:?}",
                solution.errors
            );
        }
    }

    /// `Expr` coverage guard: `check` accepts every node kind without
    /// panicking (falling back to synthesis where no rule applies).
    #[test]
    fn check_is_total_over_expression_nodes() {
        let sources = [
            "x.attr",
            "x[i]",
            "await x",
            "lambda: 1",
            "{1, 2}",
            "(1, 2)",
            "{\"k\": 1}",
            "b\"raw\"",
            "1.5",
            "True",
            "None",
            "...",
        ];
        for source in sources {
            let module = parse_expr(source);
            let mut engine = BidirEngine::new(HashMap::new());
            engine.check(&module.body, &Ty::any());
            let solution = engine.finish();
            assert!(
                solution.errors.is_empty(),
                "{source} vs Any: {:?}",
                solution.errors
            );
        }
    }

    /// The homogeneous `tuple[int, ...]` expectation checks every element.
    #[test]
    fn homogeneous_tuple_expectation() {
        let expected = InferredType::Tuple(vec![
            InferredType::Int,
            InferredType::Named("...".to_owned()),
        ]);
        let ok = check_and_solve("(1, 2, 3)", &expected);
        assert!(ok.errors.is_empty(), "{:?}", ok.errors);

        let bad = check_and_solve("(1, \"x\")", &expected);
        assert_eq!(bad.errors.len(), 1, "{:?}", bad.errors);
    }

    /// [NARROWPLAN-INTEGRATION]: one reused engine must answer EXACTLY as a
    /// fresh engine per expression. `solve_expression` resets the variables
    /// and constraints in place, so neither the inferred type nor the error
    /// set of a later expression can be contaminated by an earlier one — the
    /// property that lets the flow walker keep a single engine alive.
    #[test]
    fn reused_engine_matches_a_fresh_engine_per_expression() {
        let sources = ["[1]", "[\"x\", \"y\"]", "{1: \"a\"}", "len(z)", "[[1], [2]]"];

        let fresh: Vec<(InferredType, usize)> = sources
            .iter()
            .map(|source| {
                let (ty, solution) = synth_and_solve(source, HashMap::new());
                (ty, solution.errors.len())
            })
            .collect();

        let mut engine = BidirEngine::new(HashMap::new());
        let reused: Vec<(InferredType, usize)> = sources
            .iter()
            .map(|source| {
                let module = parse_expr(source);
                let ty = engine.synth(&module.body);
                let solution = engine.solve_expression();
                (ty.to_inferred(&solution.vars), solution.errors.len())
            })
            .collect();

        assert_eq!(reused, fresh);
    }

    /// [NARROWPLAN-INTEGRATION]: a pushed overlay shadows the scope beneath it
    /// and is gone after the pop — the flow walker's per-expression binding
    /// layer over a fixed module-callable scope.
    #[test]
    fn pushed_overlay_shadows_and_then_disappears() {
        let module = parse_expr("value");
        let outer: HashMap<String, Ty> = [("value".to_owned(), Ty::Ground(InferredType::Int))]
            .into_iter()
            .collect();
        let mut engine = BidirEngine::new(outer);

        engine.push_scope_with(
            [("value".to_owned(), Ty::Ground(InferredType::Str))]
                .into_iter()
                .collect(),
        );
        let shadowed = engine.synth(&module.body);
        let solution = engine.solve_expression();
        assert_eq!(shadowed.to_inferred(&solution.vars), InferredType::Str);

        engine.pop_scope();
        let restored = engine.synth(&module.body);
        let solution = engine.solve_expression();
        assert_eq!(restored.to_inferred(&solution.vars), InferredType::Int);
    }
}

#[cfg(test)]
mod expression_inference_tests {
    #![expect(
        clippy::expect_used,
        reason = "test-only parsing of fixed, known-valid expression fixtures"
    )]

    use std::collections::HashMap;

    use crate::types::{InferredType, LiteralValue};

    use super::{BidirEngine, Ty};

    fn synth(source: &str, globals: HashMap<String, Ty>) -> InferredType {
        let module = ruff_python_parser::parse_expression(source)
            .map(ruff_python_parser::Parsed::into_syntax)
            .expect("test expression must parse");
        let mut engine = BidirEngine::new(globals);
        let ty = engine.synth(&module.body);
        let solution = engine.finish();
        ty.to_inferred(&solution.vars)
    }

    /// Builtin calls answer from the centralized table
    /// ([NARROWPLAN-CHECKLIST]: no rule-local string tables).
    #[test]
    fn builtin_calls_use_the_central_table() {
        assert_eq!(synth("len(x)", HashMap::new()), InferredType::Int);
        assert_eq!(synth("str(3)", HashMap::new()), InferredType::Str);
        assert_eq!(
            synth("isinstance(x, int)", HashMap::new()),
            InferredType::Bool
        );
        assert_eq!(
            synth("sorted(items)", HashMap::new()),
            InferredType::List(Box::new(InferredType::Unknown))
        );
    }

    /// Builtin method calls infer from the receiver's synthesized type.
    #[test]
    fn builtin_methods_infer_from_receiver() {
        assert_eq!(
            synth("\"a b\".split()", HashMap::new()),
            InferredType::List(Box::new(InferredType::Str))
        );
        assert_eq!(synth("\"x\".upper()", HashMap::new()), InferredType::Str);
        assert_eq!(synth("[1, 2].count(1)", HashMap::new()), InferredType::Int);
        let globals: HashMap<String, Ty> = [(
            "table".to_owned(),
            Ty::from_inferred(&InferredType::Dict(
                Box::new(InferredType::Str),
                Box::new(InferredType::Int),
            )),
        )]
        .into_iter()
        .collect();
        assert_eq!(
            synth("table.get(key)", globals),
            InferredType::Optional(Box::new(InferredType::Int))
        );
    }

    /// Constructor calls on user classes yield the class instance type.
    #[test]
    fn constructor_calls_yield_instances() {
        let globals: HashMap<String, Ty> = [(
            "Point".to_owned(),
            Ty::Ground(InferredType::Named("point".to_owned())),
        )]
        .into_iter()
        .collect();
        assert_eq!(
            synth("Point(1, 2)", globals),
            InferredType::Named("point".to_owned())
        );
    }

    /// Subscript inference: list element, dict value, tuple position, str.
    #[test]
    fn subscripts_extract_element_types() {
        let globals: HashMap<String, Ty> = [
            (
                "xs".to_owned(),
                Ty::from_inferred(&InferredType::List(Box::new(InferredType::Int))),
            ),
            (
                "d".to_owned(),
                Ty::from_inferred(&InferredType::Dict(
                    Box::new(InferredType::Str),
                    Box::new(InferredType::Float),
                )),
            ),
            (
                "pair".to_owned(),
                Ty::from_inferred(&InferredType::Tuple(vec![
                    InferredType::Int,
                    InferredType::Str,
                ])),
            ),
        ]
        .into_iter()
        .collect();
        assert_eq!(synth("xs[0]", globals.clone()), InferredType::Int);
        assert_eq!(synth("d[k]", globals.clone()), InferredType::Float);
        assert_eq!(synth("pair[1]", globals.clone()), InferredType::Str);
        assert_eq!(
            synth("xs[1:2]", globals),
            InferredType::List(Box::new(InferredType::Int)),
            "slicing a list yields the list type"
        );
        assert_eq!(
            synth("[1][0]", HashMap::new()),
            InferredType::Literal(LiteralValue::Int(1)),
            "literal list subscript keeps deferred-generalization precision"
        );
    }
}
