//! TEMPORARY verification repro (to be deleted): does a UNION guard over an
//! ATOMIC declared type intersect to `Never` instead of distributing
//! member-wise?
#![expect(
    clippy::expect_used,
    reason = "test-only parsing/resolution of fixed fixtures"
)]

use std::collections::HashMap;

use basilisk_checker::narrow::{analyse_function, intersect, FlowResult, NarrowEnv};
use basilisk_checker::types::{InferredType, LiteralValue};
use ruff_python_ast::Stmt;

fn analyse(source: &str) -> FlowResult {
    let parsed = basilisk_parser::parse_source(source.to_owned(), "flow.py".to_owned())
        .expect("fixture parses");
    let resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let function = resolved
        .functions
        .iter()
        .find(|function| function.name == "f")
        .expect("fixture has a function f");
    let declared: HashMap<String, InferredType> = function
        .parameters
        .iter()
        .filter_map(|param| {
            let span = param.annotation_span?;
            let start = usize::try_from(span.start).ok()?;
            let end = usize::try_from(span.end).ok()?;
            let text = source.get(start..end)?;
            Some((param.name.clone(), InferredType::from_annotation(text)))
        })
        .collect();

    let reparsed = ruff_python_parser::parse_module(source).expect("fixture reparses");
    let body = reparsed
        .syntax()
        .body
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::FunctionDef(def) => Some(def.body.clone()),
            _ => None,
        })
        .expect("function body found");

    analyse_function(&body, NarrowEnv::new(declared), &function.narrowing_guards)
}

/// Direct set-op check: int ∧ (bool | str) should be bool, not Never.
#[test]
fn direct_intersect_atom_with_union_guard() {
    let guard = InferredType::Union(vec![InferredType::Bool, InferredType::Str]);
    let narrowed = intersect(&InferredType::Int, &guard);
    eprintln!("intersect(int, bool|str) = {narrowed:?}");
    // Member-wise correct answer: bool. Claim says this yields Never.
    assert_eq!(narrowed, InferredType::Never, "claim refuted if this fails");
}

/// Direct set-op check for the `InLiterals` shape: `int ∧ (Literal[1] | Literal["a"])`.
#[test]
fn direct_intersect_atom_with_literal_union_guard() {
    let guard = InferredType::Union(vec![
        InferredType::Literal(LiteralValue::Int(1)),
        InferredType::Literal(LiteralValue::Str("a".to_owned())),
    ]);
    let narrowed = intersect(&InferredType::Int, &guard);
    eprintln!("intersect(int, Literal[1]|Literal[\"a\"]) = {narrowed:?}");
    assert_eq!(narrowed, InferredType::Never, "claim refuted if this fails");
}

/// End-to-end: the claim's exact isinstance snippet.
#[test]
fn e2e_isinstance_union_guard_over_atomic_declared() {
    let result = analyse(
        r"
def f(x: int):
    if isinstance(x, (bool, str)):
        y = x
",
    );
    eprintln!("narrowed_uses = {:?}", result.narrowed_uses);
    eprintln!("unreachable_ranges = {:?}", result.unreachable_ranges);
    let x_uses: Vec<&InferredType> = result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    eprintln!("x narrowed uses = {x_uses:?}");
}

/// End-to-end: the claim's `InLiterals` snippet.
#[test]
fn e2e_in_literals_union_guard_over_atomic_declared() {
    let result = analyse(
        r#"
def f(x: int):
    if x in (1, "a"):
        y = x
"#,
    );
    eprintln!("narrowed_uses = {:?}", result.narrowed_uses);
    eprintln!("unreachable_ranges = {:?}", result.unreachable_ranges);
}
