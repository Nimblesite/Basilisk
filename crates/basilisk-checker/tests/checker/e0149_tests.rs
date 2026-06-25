//! Tests for [BSK-E0149] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0149: PEP 695 type parameter scoping.

use super::common::*;

#[test]
fn e0149_type_param_scoping_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Outer[T]:
    class Inner[T]:
        pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0149_type_alias_bound_violation_fires() -> Result<(), Box<dyn std::error::Error>> {
    // When a PEP 695 type alias with bounded type params is used in an
    // annotation, the type arguments must satisfy the bounds.
    let source = r"
from typing import Callable

type RecursiveTypeAlias2[S: int, T: str, **P] = Callable[P, T] | list[S] | list[RecursiveTypeAlias2[S, T, P]]

r2_1: RecursiveTypeAlias2[str, str, ...] = []
r2_3: RecursiveTypeAlias2[int, int, ...] = []
";
    let diags = run(source)?;
    let msgs = messages_for(&diags, "BSK-E0149");
    assert!(
        msgs.len() >= 2,
        "E0149 should fire for both bound violations (str vs int bound, int vs str bound), got: {msgs:?}"
    );
    // Verify the messages mention the right parameters
    let joined = msgs.join(" | ");
    assert!(
        joined.contains('S') || joined.contains("int"),
        "should mention S or int bound violation: {joined}"
    );
    assert!(
        joined.contains('T') || joined.contains("str"),
        "should mention T or str bound violation: {joined}"
    );
    Ok(())
}

#[test]
fn e0149_type_alias_bound_valid_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Valid type arguments that satisfy bounds should NOT fire.
    let source = r"
from typing import Callable

type RecursiveTypeAlias2[S: int, T: str, **P] = Callable[P, T] | list[S] | list[RecursiveTypeAlias2[S, T, P]]

r2_2: RecursiveTypeAlias2[int, str, ...] = []
r2_4: RecursiveTypeAlias2[int, str, [int, str]] = []
";
    let diags = run(source)?;
    let msgs = messages_for(&diags, "BSK-E0149");
    // Filter to only bound-violation messages (not other E0149 violations)
    let bound_msgs: Vec<_> = msgs.iter().filter(|m| m.contains("bound")).collect();
    assert!(
        bound_msgs.is_empty(),
        "E0149 should not fire bound violations for valid type args, got: {bound_msgs:?}"
    );
    Ok(())
}

#[test]
fn e0149_no_false_positive_in_module_docstring_spec_ids() -> Result<(), Box<dyn std::error::Error>>
{
    // Regression for https://github.com/Nimblesite/Basilisk/issues/43:
    // BSK-E0149 must not treat module-docstring prose as code. A docstring
    // line that happens to begin with `class ` and contains `[SPEC-ID]`
    // cross-references (our own convention) must NOT be parsed as a PEP 695
    // type parameter list, and the bracketed token must NOT then be flagged
    // as an out-of-scope type-parameter use.
    let source = r#"
"""Docstring.

class as the public Supabase anon key — see [AI-API-AUTH] in
foo bar [AI-API-AUTH].
"""
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "BSK-E0149");
    assert!(
        msgs.is_empty(),
        "BSK-E0149 must not fire on docstring prose, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0149_valid_distinct_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Outer[T]:
    class Inner[U]:
        pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0149"),
        "distinct type params should not fire E0149"
    );
    Ok(())
}

#[test]
fn e0149_mutual_alias_cycle_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "type A = B\ntype B = A\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0149"),
        "mutually-recursive bare type aliases must fire E0149, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0149_recursion_through_container_ok() -> Result<(), Box<dyn std::error::Error>> {
    // Recursion through a container terminates and is legitimate.
    let source = "type A = list[B]\ntype B = list[A]\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0149"),
        "recursion through a container must not fire E0149, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0149_self_recursion_through_list_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Tree[T] = T | list[Tree[T]]\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0149"),
        "parameterized self-recursion through list must not fire E0149"
    );
    Ok(())
}
