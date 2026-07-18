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
use basilisk_checker::types::{InferredType, LiteralValue};
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

/// `match` cases narrow the subject per case ([TYPEINF-NARROWING-MATCH]).
#[test]
fn match_cases_narrow_the_subject() {
    let result = analyse(
        r"
def f(x: int | str) -> None:
    match x:
        case int():
            a = x
        case str():
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
        "int() case must narrow to int: {uses:?}"
    );
    assert!(
        uses.contains(&("x", &InferredType::Str)),
        "str() case must narrow to str: {uses:?}"
    );
}

/// Value patterns (`case 1:`) never fabricate `Never` narrowing.
#[test]
fn match_value_patterns_stay_conservative() {
    let result = analyse(
        r"
def f(x: int | str) -> None:
    match x:
        case 1:
            a = x
",
    );
    for use_site in &result.narrowed_uses {
        assert_ne!(
            use_site.narrowed,
            InferredType::Never,
            "value pattern must not narrow to Never"
        );
    }
}

/// Unreachable branch: a guard impossible for the declared type narrows the
/// variable to `Never` inside that branch — the inference-driven
/// reachability signal.
#[test]
fn impossible_guard_narrows_to_never_in_branch() {
    let result = analyse(
        r"
def f(x: int) -> None:
    if isinstance(x, str):
        y = x
    z = x
",
    );
    assert_eq!(
        last_use(&result, "x"),
        Some(InferredType::Never),
        "inside `isinstance(x, str)` with x: int the branch is unreachable: {:?}",
        result.narrowed_uses
    );
}

/// `x == <literal>` narrows to the literal; the complement removes exactly
/// that literal member ([TYPEINF-NARROWING-EQ-LITERAL]).
#[test]
fn equality_literal_narrows_both_branches() {
    let result = analyse(
        r"
def f(x: Literal[1] | Literal[2]) -> None:
    if x == 1:
        a = x
    else:
        b = x
",
    );
    let uses: Vec<&InferredType> = result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert!(
        uses.contains(&&InferredType::Literal(LiteralValue::Int(1))),
        "positive branch must see Literal[1]: {uses:?}"
    );
    assert!(
        uses.contains(&&InferredType::Literal(LiteralValue::Int(2))),
        "complement must see Literal[2]: {uses:?}"
    );
}

/// `x in (1, 2)` narrows to the union of members; `not in`/else removes them
/// ([TYPEINF-NARROWING-IN-LITERAL]).
#[test]
fn membership_literals_narrow_both_branches() {
    let result = analyse(
        r"
def f(x: Literal[1] | Literal[2] | Literal[3]) -> None:
    if x in (1, 2):
        a = x
    else:
        b = x
",
    );
    let else_use = last_use(&result, "x").expect("else-branch use recorded");
    assert_eq!(
        else_use,
        InferredType::Literal(LiteralValue::Int(3)),
        "the complement of `in (1, 2)` must be Literal[3]: {:?}",
        result.narrowed_uses
    );
}

/// `"key" in td` keeps union members whose `TypedDict` schema declares the
/// key; the complement drops members where it is required
/// ([TYPEINF-NARROWING-TYPEDDICT-KEY]).
#[test]
fn typeddict_key_membership_narrows_union() {
    use basilisk_checker::narrow::{analyse_function_in, NarrowContext, TypedDictKeys};
    let source = r#"
def f(x: WithKey | WithoutKey) -> None:
    if "k" in x:
        a = x
    else:
        b = x
"#;
    let parsed = basilisk_parser::parse_source(source.to_owned(), "td.py".to_owned())
        .expect("fixture parses");
    let resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let function = resolved.functions.first().expect("function");
    let declared: HashMap<String, InferredType> = [(
        "x".to_owned(),
        InferredType::Union(vec![
            InferredType::Named("withkey".to_owned()),
            InferredType::Named("withoutkey".to_owned()),
        ]),
    )]
    .into_iter()
    .collect();
    let mut ctx = NarrowContext::default();
    let _ = ctx.typeddict_keys.insert(
        "withkey".to_owned(),
        TypedDictKeys {
            all: ["k".to_owned()].into_iter().collect(),
            required: ["k".to_owned()].into_iter().collect(),
        },
    );
    let _ = ctx
        .typeddict_keys
        .insert("withoutkey".to_owned(), TypedDictKeys::default());

    let reparsed = ruff_python_parser::parse_module(source).expect("reparses");
    let body = reparsed
        .syntax()
        .body
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::FunctionDef(def) => Some(def.body.clone()),
            _ => None,
        })
        .expect("body");
    let result = analyse_function_in(
        &body,
        NarrowEnv::new(declared),
        &function.narrowing_guards,
        &ctx,
    );
    let uses: Vec<&InferredType> = result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert!(
        uses.contains(&&InferredType::Named("withkey".to_owned())),
        "positive branch keeps the schema with the key: {uses:?}"
    );
    assert!(
        uses.contains(&&InferredType::Named("withoutkey".to_owned())),
        "complement keeps the schema lacking the required key: {uses:?}"
    );
}

/// Implied else: when every match case diverges, code after the match sees
/// the subject minus all covered pattern types.
#[test]
fn exhaustive_diverging_match_narrows_after() {
    let result = analyse(
        r"
def f(x: int | str | None) -> None:
    match x:
        case int():
            return
        case str():
            return
    y = x
",
    );
    assert_eq!(
        last_use(&result, "x"),
        Some(InferredType::None_),
        "after int/str cases both return, only None remains: {:?}",
        result.narrowed_uses
    );
}

/// Inference-driven reachability: a branch whose guard narrows the variable
/// to `Never` is reported unreachable — derived from the type lattice, not
/// a pattern-matched idiom.
#[test]
fn impossible_branch_is_reported_unreachable() {
    let result = analyse(
        r"
def f(x: int) -> None:
    if isinstance(x, str):
        y = x
",
    );
    assert_eq!(
        result.unreachable_ranges.len(),
        1,
        "the isinstance(x, str) body must be unreachable for x: int: {:?}",
        result.unreachable_ranges
    );
}

/// `type(x) is C` implies `isinstance(x, C)` positively; the negative branch
/// excludes `C` only when `C` is `@final` ([TYPEINF-NARROWING-TYPEOF]).
#[test]
fn type_of_is_narrows_with_final_awareness() {
    use basilisk_checker::narrow::{analyse_function_in, NarrowContext};
    let source = r"
def f(x: A | B) -> None:
    if type(x) is A:
        p = x
    else:
        q = x
";
    let parsed = basilisk_parser::parse_source(source.to_owned(), "t.py".to_owned())
        .expect("fixture parses");
    let resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let function = resolved.functions.first().expect("function");
    let declared: HashMap<String, InferredType> = [(
        "x".to_owned(),
        InferredType::Union(vec![
            InferredType::Named("a".to_owned()),
            InferredType::Named("b".to_owned()),
        ]),
    )]
    .into_iter()
    .collect();
    let reparsed = ruff_python_parser::parse_module(source).expect("reparses");
    let body = reparsed
        .syntax()
        .body
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::FunctionDef(def) => Some(def.body.clone()),
            _ => None,
        })
        .expect("body");

    // Without @final knowledge, the negative branch stays unchanged.
    let plain = analyse_function_in(
        &body,
        NarrowEnv::new(declared.clone()),
        &function.narrowing_guards,
        &NarrowContext::default(),
    );
    let plain_uses: Vec<&InferredType> = plain
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert!(
        plain_uses.contains(&&InferredType::Named("a".to_owned())),
        "positive branch narrows to A: {plain_uses:?}"
    );
    assert_eq!(
        plain_uses.len(),
        1,
        "non-final A must not be excluded in the negative branch: {plain_uses:?}"
    );

    // With A known @final, the negative branch excludes it.
    let mut ctx = NarrowContext::default();
    let _ = ctx.final_classes.insert("a".to_owned());
    let with_final = analyse_function_in(
        &body,
        NarrowEnv::new(declared),
        &function.narrowing_guards,
        &ctx,
    );
    let final_uses: Vec<&InferredType> = with_final
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert!(
        final_uses.contains(&&InferredType::Named("b".to_owned())),
        "with @final A, the complement must be B: {final_uses:?}"
    );
}
