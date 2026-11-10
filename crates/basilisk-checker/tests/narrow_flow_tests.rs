//! Tests for [TYPEINF-NARROWING] / [TYPEINF-TARGET-NARROWING] Stage 2 flow
//! analysis. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-NARROWING,
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING, and
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

use basilisk_checker::narrow::{analyse_function, guard_outcomes, FlowResult, NarrowEnv};
use basilisk_checker::types::{InferredType, LiteralValue};
use basilisk_resolver::{NarrowingGuard, NarrowingGuardKind};
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

/// Resolve every guard collected for `source`'s first function.
fn resolved_guards(source: &str) -> Vec<NarrowingGuard> {
    let parsed = basilisk_parser::parse_source(source.to_owned(), "flow.py".to_owned())
        .expect("fixture parses");
    let resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    resolved
        .functions
        .first()
        .expect("fixture has a function")
        .narrowing_guards
        .clone()
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

/// `issubclass` extraction is live, but class-object subtyping is staged; both
/// branches therefore preserve the declared type without guessed precision
/// ([TYPEINF-NARROWING-ISSUBCLASS]).
#[test]
fn staged_issubclass_guard_preserves_both_branches() {
    let source = r"
def f(x: type[int] | type[str]) -> None:
    if issubclass(x, int):
        positive = x
    else:
        negative = x
";
    let guards = resolved_guards(source);
    let guard = guards
        .iter()
        .find(|guard| matches!(&guard.kind, NarrowingGuardKind::IsSubclass { variable, type_names, .. } if variable == "x" && type_names == &["int"]))
        .expect("resolver extracts issubclass guard");
    let declared = InferredType::from_annotation("type[int] | type[str]");
    let outcome = guard_outcomes(guard, &declared).expect("checker consumes issubclass guard");
    assert_eq!(outcome.positive, declared);
    assert_eq!(outcome.negative, declared);

    let result = analyse(source);
    assert!(result.narrowed_uses.is_empty());
    assert!(result.unreachable_ranges.is_empty());
}

/// `hasattr` extraction is live, but synthetic-protocol intersections are
/// staged; both branches preserve the declared type without guessed precision
/// ([TYPEINF-NARROWING-HASATTR]).
#[test]
fn staged_hasattr_guard_preserves_both_branches() {
    let source = r#"
def f(x: A | B) -> None:
    if hasattr(x, "value"):
        positive = x
    else:
        negative = x
"#;
    let guards = resolved_guards(source);
    let guard = guards
        .iter()
        .find(|guard| matches!(&guard.kind, NarrowingGuardKind::HasAttr { variable, attribute, .. } if variable == "x" && attribute == "value"))
        .expect("resolver extracts hasattr guard");
    let declared = InferredType::from_annotation("A | B");
    let outcome = guard_outcomes(guard, &declared).expect("checker consumes hasattr guard");
    assert_eq!(outcome.positive, declared);
    assert_eq!(outcome.negative, declared);

    let result = analyse(source);
    assert!(result.narrowed_uses.is_empty());
    assert!(result.unreachable_ranges.is_empty());
}

/// Truthiness partitions a finite literal union exactly: the true branch
/// excludes `None` and every falsy literal, while the false branch keeps only
/// those falsy members ([TYPEINF-NARROWING-TRUTHY]).
#[test]
fn truthiness_partitions_none_and_falsy_literals() {
    let result = analyse(
        r#"
def f(x: Literal[0] | Literal[1] | Literal[""] | Literal["ok"] | Literal[False] | Literal[True] | None) -> None:
    if x:
        truthy = x
    else:
        falsy = x
"#,
    );
    let uses: Vec<&InferredType> = result
        .narrowed_uses
        .iter()
        .filter(|use_site| use_site.name == "x")
        .map(|use_site| &use_site.narrowed)
        .collect();

    let truthy = InferredType::Union(vec![
        InferredType::Literal(LiteralValue::Int(1)),
        InferredType::Literal(LiteralValue::Str("ok".to_owned())),
        InferredType::Literal(LiteralValue::Bool(true)),
    ]);
    let falsy = InferredType::Union(vec![
        InferredType::Literal(LiteralValue::Int(0)),
        InferredType::Literal(LiteralValue::Str(String::new())),
        InferredType::Literal(LiteralValue::Bool(false)),
        InferredType::None_,
    ]);

    assert!(
        uses.contains(&&truthy),
        "the positive branch must contain only truthy members: {uses:?}"
    );
    assert!(
        uses.contains(&&falsy),
        "the negative branch must contain only falsy members: {uses:?}"
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

/// Rebinding invalidation ([TYPEINF-NARROWING-ASSIGN]): a tuple-unpacking
/// assignment rebinds its names — the stale pre-rebind narrow must die, and
/// the element-wise value types take over.
#[test]
fn tuple_unpacking_invalidates_stale_narrows() {
    let result = analyse(
        r"
def f(x: int | None) -> None:
    if x is None:
        return
    x, y = None, 0
    a = x
    b = y
",
    );
    let x_uses: Vec<&InferredType> = result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert_eq!(
        last_use(&result, "x"),
        Some(InferredType::None_),
        "after `x, y = None, 0` the flow type of x is None: {x_uses:?}"
    );
    assert_eq!(
        last_use(&result, "y"),
        Some(InferredType::Literal(LiteralValue::Int(0))),
        "tuple unpacking distributes element-wise: {:?}",
        result.narrowed_uses
    );
}

/// Rebinding invalidation: a `for` target rebinds each iteration — inside
/// the body the target is the iterable's ELEMENT type, and the stale
/// pre-loop narrow never survives (in the body or after the loop).
#[test]
fn for_target_rebinds_to_the_element_type() {
    let result = analyse(
        r"
def f(x: int | None, items: list[None]) -> None:
    if x is None:
        return
    for x in items:
        a = x
    b = x
",
    );
    let x_uses: Vec<&InferredType> = result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert!(
        !x_uses.contains(&&InferredType::Int),
        "the stale pre-loop Int narrow must not survive the rebinding: {x_uses:?}"
    );
    assert!(
        x_uses.contains(&&InferredType::None_),
        "inside the loop, x is the element type of list[None]: {x_uses:?}"
    );
}

/// Rebinding invalidation: a name assigned inside a loop BODY is unreliable
/// both inside the loop (later iterations) and after it.
#[test]
fn loop_body_assignment_invalidates_stale_narrows() {
    let result = analyse(
        r"
def f(x: int | None, items: list[int]) -> None:
    if x is None:
        return
    for i in items:
        x = None
    a = x
",
    );
    let x_uses: Vec<&InferredType> = result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert!(
        !x_uses.contains(&&InferredType::Int),
        "a loop-body rebinding kills the pre-loop narrow: {x_uses:?}"
    );
}

/// Rebinding invalidation: `with ... as x` and `x += 1` rebind without a
/// modelled result type — the stale narrow resets, never a guess. The read
/// BEFORE the rebind (including the aug-assign target's own read) still sees
/// the live narrow; reads after it are back at the declared type and are
/// therefore not recorded.
#[test]
fn with_and_augassign_targets_reset_stale_narrows() {
    let with_result = analyse(
        r"
def f(x: int | None) -> None:
    if x is None:
        return
    c = x
    with open(p) as x:
        a = x
",
    );
    let x_uses: Vec<&InferredType> = with_result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert_eq!(
        x_uses,
        vec![&InferredType::Int],
        "only the pre-rebind read keeps the narrow; `a = x` reverts to declared"
    );

    let aug_result = analyse(
        r"
def f(y: int | None) -> None:
    if y is None:
        return
    y += 1
    b = y
",
    );
    let y_uses: Vec<&InferredType> = aug_result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "y")
        .map(|u| &u.narrowed)
        .collect();
    assert_eq!(
        y_uses,
        vec![&InferredType::Int],
        "the aug-assign target READ sees the old narrow; `b = y` reverts to declared"
    );
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

/// A UNION guard over an ATOMIC declared type distributes member-wise:
/// `isinstance(x, (bool, str))` with `x: int` narrows to `bool` (the
/// overlapping member), never a collapsed `Never`.
#[test]
fn union_guard_over_atomic_declared_narrows_memberwise() {
    let result = analyse(
        r"
def f(x: int) -> None:
    if isinstance(x, (bool, str)):
        y = x
",
    );
    let x_uses: Vec<&InferredType> = result
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert!(
        x_uses.contains(&&InferredType::Bool),
        "int ∧ (bool | str) must keep the overlapping member bool: {x_uses:?}"
    );
    assert!(
        result.unreachable_ranges.is_empty(),
        "the branch is reachable (bool <: int): {:?}",
        result.unreachable_ranges
    );
}

/// Inference-driven divergence: a branch ending in a call typed `Never`
/// (a same-module `NoReturn` function) counts as an early exit, so the
/// complement persists — reachability decided by the type, not an idiom.
#[test]
fn never_returning_call_diverges_and_persists_the_complement() {
    use basilisk_checker::narrow::{analyse_function_in, NarrowContext};
    use basilisk_checker::types::CallableInfo;
    let source = r"
def f(x: int | None) -> None:
    if x is None:
        fail()
    y = x
";
    let parsed = basilisk_parser::parse_source(source.to_owned(), "flow.py".to_owned())
        .expect("fixture parses");
    let resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let function = resolved.functions.first().expect("function");
    let declared: HashMap<String, InferredType> = [(
        "x".to_owned(),
        InferredType::Optional(Box::new(InferredType::Int)),
    )]
    .into_iter()
    .collect();
    let mut ctx = NarrowContext::default();
    let _ = ctx.callables.insert(
        "fail".to_owned(),
        InferredType::Callable(CallableInfo {
            param_types: vec![],
            return_type: Box::new(InferredType::Never),
        }),
    );
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
    assert_eq!(
        last_use(&result, "x"),
        Some(InferredType::Int),
        "after the Never-call branch, the complement must persist: {:?}",
        result.narrowed_uses
    );
}

/// Inference-driven divergence: `while True:` (a proven always-truthy test)
/// without a `break` diverges, so the complement persists after the `if`.
#[test]
fn while_true_branch_diverges_and_persists_the_complement() {
    let result = analyse(
        r"
def f(x: int | None) -> int:
    if x is None:
        while True:
            pass
    return x
",
    );
    assert_eq!(
        last_use(&result, "x"),
        Some(InferredType::Int),
        "a `while True:` branch never falls through: {:?}",
        result.narrowed_uses
    );
}

/// Inference-driven reachability: statements after a proven-diverging
/// statement are reported unreachable — including via an `if`/`else` whose
/// branches ALL diverge (recursive divergence, not a last-statement idiom).
#[test]
fn statements_after_divergence_are_reported_unreachable() {
    let after_return = analyse(
        r"
def f(x: int) -> None:
    return
    y = x
",
    );
    assert_eq!(
        after_return.unreachable_ranges.len(),
        1,
        "code after `return` is unreachable: {:?}",
        after_return.unreachable_ranges
    );

    let after_exhaustive_if = analyse(
        r"
def f(x: int | str) -> int:
    if isinstance(x, int):
        return 1
    else:
        return 2
    y = x
",
    );
    assert_eq!(
        after_exhaustive_if.unreachable_ranges.len(),
        1,
        "an if/else whose branches all diverge is itself divergent: {:?}",
        after_exhaustive_if.unreachable_ranges
    );
}

/// The `type(x) is C` guard carries RESOLVED identity across the resolver
/// boundary: the span of the compared-against type expression, and that
/// class's definition site.
///
/// The deleted consumer rendered the class to a simple name, reparsed it with
/// the text parser, and decided `@final`-ness with
/// `final_classes.contains(&name.to_ascii_lowercase())`. This pins the
/// replacement at the boundary itself, independent of what the narrowing
/// engine can currently do with it.
#[test]
fn type_of_is_guard_carries_the_resolved_class_identity() {
    let source = r"
from typing import final

@final
class Sluice:
    pass

Penstock = Sluice

class Weir:
    pass

def f(x: Sluice | Weir) -> None:
    if type(x) is Penstock:
        p = x
";
    let parsed = basilisk_parser::parse_source(source.to_owned(), "t.py".to_owned())
        .expect("fixture parses");
    let resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let function = resolved
        .functions
        .iter()
        .find(|f| f.name == "f")
        .expect("function `f`");
    let sluice = resolved
        .classes
        .iter()
        .find(|c| c.name == "Sluice")
        .expect("class `Sluice`");

    let type_class_site = function
        .narrowing_guards
        .iter()
        .find_map(|guard| match &guard.kind {
            basilisk_resolver::NarrowingGuardKind::TypeOfIs {
                type_class_site, ..
            } => Some(*type_class_site),
            _ => None,
        })
        .expect("the fixture contains a `type(x) is C` guard");

    assert_eq!(
        type_class_site,
        Some(sluice.name_span),
        "`Penstock` is one more name for `Sluice`; the guard must carry the \
         class it denotes, not the word written at the comparison"
    );
    assert!(
        sluice.is_final,
        "`@final` resolves through the binding table, so the aliased `final` \
         import still marks the class"
    );
}

/// `type(x) is C` implies `isinstance(x, C)` positively; the negative branch
/// excludes `C` only when `C` is `@final` ([TYPEINF-NARROWING-TYPEOF]).
///
/// RED, ON PURPOSE. `InferredType::Named` carries a RENDERING, so the set
/// operations behind narrowing have no way to tell whether `Weir` overlaps
/// `Sluice` — that is a question about the module's class hierarchy, which
/// they do not have. Rather than compare the two strings they abstain
/// (`narrow/set_ops.rs::nominal_pair`), which leaves narrowing over
/// user-defined classes INERT.
///
/// This test is the accurate map of that gap. It passes when a nominal leaf
/// carries its definition site instead of its spelling
/// ([TYPEINF-SUBTYPING-NOMINAL]). Do not delete it, and do not make it pass by
/// comparing renderings in `set_ops`.
#[test]
fn type_of_is_narrows_with_final_awareness() {
    use basilisk_checker::narrow::{analyse_function_in, NarrowContext};
    let source = r"
from typing import final

@final
class Sluice:
    pass

class Weir:
    pass

def f(x: Sluice | Weir) -> None:
    if type(x) is Sluice:
        p = x
    else:
        q = x
";
    let parsed = basilisk_parser::parse_source(source.to_owned(), "t.py".to_owned())
        .expect("fixture parses");
    let resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let function = resolved
        .functions
        .iter()
        .find(|f| f.name == "f")
        .expect("function `f`");
    let declared: HashMap<String, InferredType> = [(
        "x".to_owned(),
        InferredType::Union(vec![
            InferredType::Named("Sluice".to_owned()),
            InferredType::Named("Weir".to_owned()),
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
            Stmt::FunctionDef(def) if def.name.as_str() == "f" => Some(def.body.clone()),
            _ => None,
        })
        .expect("body");

    let type_span = function
        .narrowing_guards
        .iter()
        .find_map(|guard| match &guard.kind {
            basilisk_resolver::NarrowingGuardKind::TypeOfIs { type_span, .. } => Some(*type_span),
            _ => None,
        })
        .expect("the fixture contains a `type(x) is C` guard");
    let sluice = resolved
        .classes
        .iter()
        .find(|c| c.name == "Sluice")
        .expect("class `Sluice`");

    let mut base_ctx = NarrowContext::default();
    let _ = base_ctx
        .type_targets
        .insert(type_span, InferredType::Named("Sluice".to_owned()));

    // Without @final knowledge, the negative branch stays unchanged.
    let plain = analyse_function_in(
        &body,
        NarrowEnv::new(declared.clone()),
        &function.narrowing_guards,
        &base_ctx,
    );
    let plain_uses: Vec<&InferredType> = plain
        .narrowed_uses
        .iter()
        .filter(|u| u.name == "x")
        .map(|u| &u.narrowed)
        .collect();
    assert!(
        plain_uses.contains(&&InferredType::Named("Sluice".to_owned())),
        "positive branch narrows to Sluice: {plain_uses:?}"
    );
    assert_eq!(
        plain_uses.len(),
        1,
        "without @final knowledge Sluice must not be excluded in the negative \
         branch: {plain_uses:?}"
    );

    // With Sluice known @final, the negative branch excludes it.
    let mut ctx = base_ctx;
    let _ = ctx.final_class_sites.insert(sluice.name_span);
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
        final_uses.contains(&&InferredType::Named("Weir".to_owned())),
        "with @final Sluice, the complement must be Weir: {final_uses:?}"
    );
}

/// [NARROWPLAN-INTEGRATION]: the module's callable interfaces are converted
/// once and held in the engine's outermost scope for the whole walk, so a
/// module full of callables the function never mentions must produce EXACTLY
/// the same narrowed uses and unreachable ranges as an empty module. This is
/// the correctness half of making the seed cheap: amortizing it must not let
/// module-level names leak into a function's flow types.
#[test]
fn unrelated_module_callables_never_change_the_walk() {
    use basilisk_checker::narrow::{analyse_function_in, NarrowContext};
    use basilisk_checker::types::CallableInfo;

    // Nested branches so the divergence probe and the body walk ask about the
    // same statements repeatedly — the memoized path.
    let source = r"
def f(x: int | None, y: str | None) -> int:
    if x is None:
        if y is None:
            return 0
        z = y
        return 1
    w = x
    return w
";
    let parsed = basilisk_parser::parse_source(source.to_owned(), "flow.py".to_owned())
        .expect("fixture parses");
    let resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let function = resolved.functions.first().expect("function");
    let declared: HashMap<String, InferredType> = [
        (
            "x".to_owned(),
            InferredType::Optional(Box::new(InferredType::Int)),
        ),
        (
            "y".to_owned(),
            InferredType::Optional(Box::new(InferredType::Str)),
        ),
    ]
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

    let empty_module = analyse_function_in(
        &body,
        NarrowEnv::new(declared.clone()),
        &function.narrowing_guards,
        &NarrowContext::default(),
    );

    let mut crowded = NarrowContext::default();
    for index in 0..500 {
        let _ = crowded.callables.insert(
            format!("unused{index}"),
            InferredType::Callable(CallableInfo {
                param_types: vec![],
                return_type: Box::new(InferredType::Never),
            }),
        );
    }
    let crowded_module = analyse_function_in(
        &body,
        NarrowEnv::new(declared),
        &function.narrowing_guards,
        &crowded,
    );

    assert_eq!(
        crowded_module.narrowed_uses, empty_module.narrowed_uses,
        "500 unmentioned module callables must not alter narrowing"
    );
    assert_eq!(
        crowded_module.unreachable_ranges, empty_module.unreachable_ranges,
        "500 unmentioned module callables must not alter reachability"
    );
    assert!(
        !empty_module.narrowed_uses.is_empty(),
        "the fixture must actually narrow something, or it proves nothing"
    );
}
