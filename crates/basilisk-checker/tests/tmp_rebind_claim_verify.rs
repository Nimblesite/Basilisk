//! TEMPORARY verification harness for the claimed rebinding-invalidation
//! defect in `narrow/flow.rs` `walk_assign` — delete after use. Not part of the
//! suite.
#![expect(
    clippy::expect_used,
    reason = "test-only parsing/resolution of fixed fixtures"
)]

use std::collections::HashMap;

use basilisk_checker::narrow::{analyse_function, FlowResult, NarrowEnv};
use basilisk_checker::types::InferredType;
use ruff_python_ast::Stmt;

fn analyse(source: &str) -> FlowResult {
    let parsed = basilisk_parser::parse_source(source.to_owned(), "flow.py".to_owned())
        .expect("fixture parses");
    let resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let function = resolved.functions.first().expect("fixture has a function");
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

fn x_uses(result: &FlowResult) -> Vec<&InferredType> {
    result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect()
}

/// Claim scenario 1: tuple unpacking rebinds x to None, but the stale
/// Int narrow persists at `a = x`.
#[test]
fn tmp_tuple_unpack_keeps_stale_narrow() {
    let result = analyse(
        r"
def f(x: int | None) -> None:
    if x is None:
        return
    x, y = None, 0
    a = x
",
    );
    let uses = x_uses(&result);
    eprintln!("scenario 1 recorded x uses: {uses:?}");
    assert!(
        uses.contains(&&InferredType::Int),
        "claim NOT reproduced: no stale Int narrow for x after tuple rebind: {uses:?}"
    );
}

/// Claim scenario 2: for-loop target rebinds x to a None element, but the
/// stale Int narrow persists at `a = x` inside the loop body.
#[test]
fn tmp_for_target_keeps_stale_narrow() {
    let result = analyse(
        r"
def f(x: int | None, items: list[None]) -> None:
    if x is None:
        return
    for x in items:
        a = x
",
    );
    let uses = x_uses(&result);
    eprintln!("scenario 2 recorded x uses: {uses:?}");
    assert!(
        uses.contains(&&InferredType::Int),
        "claim NOT reproduced: no stale Int narrow for x inside for-loop body: {uses:?}"
    );
}

/// Control: single-Name assignment DOES invalidate (the code path the claim
/// concedes works).
#[test]
fn tmp_single_name_assign_updates_narrow() {
    let result = analyse(
        r"
def f(x: int | None) -> None:
    if x is None:
        return
    x = None
    a = x
",
    );
    let uses = x_uses(&result);
    eprintln!("control recorded x uses: {uses:?}");
    // After `x = None` the last recorded use of x should NOT be Int.
    let last = result
        .narrowed_uses
        .iter()
        .rfind(|u| u.name == "x")
        .map(|u| &u.narrowed);
    assert_ne!(
        last,
        Some(&InferredType::Int),
        "control failed: single-name assign did not update the narrow: {uses:?}"
    );
}
