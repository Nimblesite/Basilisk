//! Tests for [TYPEINF-TARGET-NARROWING] Stage 2 flow analysis. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING and
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST.
//!
//! End-to-end over the real pipeline: parse → resolve (the resolver collects
//! the guards) → [`basilisk_checker::narrow::analyse_function`]. Covers the
//! checklist's positive/complement branches, early exits, loops, closures,
//! and merge behaviour.
#![expect(
    clippy::expect_used,
    reason = "test-only parsing/resolution of fixed fixtures"
)]

use std::collections::HashMap;

use basilisk_checker::narrow::{analyse_function, FlowResult, NarrowEnv};
use basilisk_checker::types::InferredType;
use ruff_python_ast::Stmt;

/// Run the flow walker over `source`'s FIRST top-level function, with
/// declared types taken from its parameter annotations.
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

/// The narrowed type observed for `name` at its LAST recorded use.
fn last_use(result: &FlowResult, name: &str) -> Option<InferredType> {
    result
        .narrowed_uses
        .iter()
        .rfind(|use_site| use_site.name == name)
        .map(|use_site| use_site.narrowed.clone())
}

/// Positive branch narrows, complement branch subtracts
/// ([TYPEINF-NARROWING-ISINSTANCE]).
#[test]
fn isinstance_positive_and_complement_branches() {
    let result = analyse(
        r"
def f(x: int | str) -> None:
    if isinstance(x, int):
        a = x
    else:
        b = x
",
    );
    let uses: Vec<(&str, &InferredType)> = result
        .narrowed_uses
        .iter()
        .map(|u| (u.name.as_str(), &u.narrowed))
        .collect();
    assert!(
        uses.contains(&("x", &InferredType::Int)),
        "positive branch must see int: {uses:?}"
    );
    assert!(
        uses.contains(&("x", &InferredType::Str)),
        "complement branch must see str: {uses:?}"
    );
}

/// Early exit: `if x is None: return` leaves the complement narrowing for
/// all subsequent statements.
#[test]
fn early_exit_persists_the_complement() {
    let result = analyse(
        r"
def f(x: int | None) -> int:
    if x is None:
        return 0
    return x
",
    );
    assert_eq!(
        last_use(&result, "x"),
        Some(InferredType::Int),
        "after the early return, x must be int: {:?}",
        result.narrowed_uses
    );
}

/// `assert x is not None` narrows the remainder of the scope
/// ([TYPEINF-NARROWING-ASSERT]).
#[test]
fn assert_narrows_the_rest_of_the_scope() {
    let result = analyse(
        r"
def f(x: int | None) -> int:
    assert x is not None
    return x
",
    );
    assert_eq!(last_use(&result, "x"), Some(InferredType::Int));
}

/// Narrowing does NOT leak past the merge when both branches complete.
#[test]
fn merge_reverts_one_sided_narrowing() {
    let result = analyse(
        r"
def f(x: int | None) -> None:
    if x is None:
        y = 1
    z = x
",
    );
    assert_eq!(
        last_use(&result, "x"),
        None,
        "after a non-diverging if, x must revert to its declared type: {:?}",
        result.narrowed_uses
    );
}

/// Guards inside loops do not narrow ([TYPEINF-NARROWING-SCOPE]).
#[test]
fn loop_guards_do_not_leak() {
    let result = analyse(
        r"
def f(x: int | None) -> None:
    for i in [1, 2]:
        if x is None:
            continue
    y = x
",
    );
    assert_eq!(
        last_use(&result, "x"),
        None,
        "loop-internal narrowing must not persist: {:?}",
        result.narrowed_uses
    );
}

/// Nested functions are a narrowing boundary: enclosing narrows do not
/// flow into the closure body.
#[test]
fn closures_do_not_inherit_narrowing() {
    let result = analyse(
        r"
def f(x: int | None) -> None:
    assert x is not None
    def inner() -> None:
        y = x
    z = x
",
    );
    // The closure's `x` read is not walked (fresh boundary), so the only
    // recorded narrowed uses of x are in the outer scope.
    for use_site in &result.narrowed_uses {
        assert_eq!(use_site.name, "x");
        assert_eq!(use_site.narrowed, InferredType::Int);
    }
    assert_eq!(last_use(&result, "x"), Some(InferredType::Int));
}

/// Assignment narrowing: `x = 1` narrows the flow type at later uses.
#[test]
fn assignment_narrows_flow_type() {
    let result = analyse(
        r"
def f(x: int | str) -> None:
    x = 1
    y = x
",
    );
    let narrowed = last_use(&result, "x").expect("x use recorded after assignment");
    assert!(
        narrowed.is_assignable_to(&InferredType::Int),
        "after x = 1 the flow type must be int-compatible, got {narrowed:?}"
    );
}
