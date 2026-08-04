//! Tests for [`generics_syntax_scoping`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_syntax_scoping: PEP 695 type parameter scoping.

use super::common::*;

#[test]
fn type_param_scoping_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn type_alias_bound_violation_fires() -> Result<(), Box<dyn std::error::Error>> {
    // When a PEP 695 type alias with bounded type params is used in an
    // annotation, the type arguments must satisfy the bounds.
    let source = r"
from typing import Callable

type RecursiveTypeAlias2[S: int, T: str, **P] = Callable[P, T] | list[S] | list[RecursiveTypeAlias2[S, T, P]]

r2_1: RecursiveTypeAlias2[str, str, ...] = []
r2_3: RecursiveTypeAlias2[int, int, ...] = []
";
    let diags = run(source)?;
    let msgs = messages_for(&diags, "generics_syntax_scoping");
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
fn type_alias_bound_valid_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    // Valid type arguments that satisfy bounds should NOT fire.
    let source = r"
from typing import Callable

type RecursiveTypeAlias2[S: int, T: str, **P] = Callable[P, T] | list[S] | list[RecursiveTypeAlias2[S, T, P]]

r2_2: RecursiveTypeAlias2[int, str, ...] = []
r2_4: RecursiveTypeAlias2[int, str, [int, str]] = []
";
    let diags = run(source)?;
    let msgs = messages_for(&diags, "generics_syntax_scoping");
    // Filter to only bound-violation messages (not other E0149 violations)
    let bound_msgs: Vec<_> = msgs.iter().filter(|m| m.contains("bound")).collect();
    assert!(
        bound_msgs.is_empty(),
        "E0149 should not fire bound violations for valid type args, got: {bound_msgs:?}"
    );
    Ok(())
}

#[test]
fn no_false_positive_in_module_docstring_spec_ids() -> Result<(), Box<dyn std::error::Error>> {
    // Regression for https://github.com/Nimblesite/Basilisk/issues/43:
    // generics_syntax_scoping must not treat module-docstring prose as code. A docstring
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
    let msgs = messages_for(&diags, "generics_syntax_scoping");
    assert!(
        msgs.is_empty(),
        "generics_syntax_scoping must not fire on docstring prose, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn valid_distinct_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Outer[T]:
    class Inner[U]:
        pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_syntax_scoping"),
        "distinct type params should not fire E0149"
    );
    Ok(())
}

#[test]
fn mutual_alias_cycle_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "type A = B\ntype B = A\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_syntax_scoping"),
        "mutually-recursive bare type aliases must fire E0149, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn recursion_through_container_ok() -> Result<(), Box<dyn std::error::Error>> {
    // Recursion through a container terminates and is legitimate.
    let source = "type A = list[B]\ntype B = list[A]\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_syntax_scoping"),
        "recursion through a container must not fire E0149, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn self_recursion_through_list_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = "type Tree[T] = T | list[Tree[T]]\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_syntax_scoping"),
        "parameterized self-recursion through list must not fire E0149"
    );
    Ok(())
}

/// Regression for [#371](https://github.com/Nimblesite/Basilisk/issues/371):
/// a NON-generic PEP 695 alias whose self-reference sits under a type
/// constructor is ordinary, terminating recursion — PEP 695 mandates it works.
/// Every form below was rejected as "Circular type alias definition"; the
/// generic spellings of the same shapes were already accepted, so the rule was
/// inverted precisely for the parameterless case.
/// Acceptance is decided by [TYPEINF-TARGET-TYPELEVEL]'s guardedness condition
/// (`tyeval::accept`), not by "does the RHS mention my own name".
#[test]
fn recursive_pep695_alias_under_a_constructor_is_accepted() -> Result<(), Box<dyn std::error::Error>>
{
    for source in [
        "type J = list[J]\n",
        "type J = int | list[J]\n",
        "type J = dict[str, J]\n",
        "type JsonValue = None | bool | int | float | str | list[JsonValue] | dict[str, JsonValue]\n",
        "type JsonValue = dict[str, JsonValue] | list[JsonValue] | str | int | float | bool | None\n",
        "type RecursiveTuple = str | int | tuple[\"RecursiveTuple\", ...]\n",
    ] {
        let diags = run(source)?;
        assert!(
            !codes(&diags).contains(&"generics_syntax_scoping"),
            "guarded recursive alias must not fire generics_syntax_scoping.\n\
             source: {source}\n got: {:?}",
            messages_for(&diags, "generics_syntax_scoping")
        );
    }
    Ok(())
}

/// Companion to the above: unguarded self-reference — the self-reference is
/// NOT under a constructor, so unfolding never reaches a head constructor —
/// must still be rejected. This is the half of the old check that was right.
#[test]
fn unguarded_self_reference_is_still_rejected() -> Result<(), Box<dyn std::error::Error>> {
    for source in ["type X = X\n", "type X = int | X\n"] {
        let diags = run(source)?;
        assert!(
            codes(&diags).contains(&"generics_syntax_scoping"),
            "unguarded self-referential alias must still fire.\nsource: {source}\n got: {:?}",
            codes(&diags)
        );
    }
    Ok(())
}

/// `Union[..]`/`Optional[..]`/`Annotated[..]` are transparent operators —
/// semantically the `|`-spellings — so recursion through them is exactly as
/// circular as `type X = int | X` (conformance `aliases_recursive.py` marks
/// the old-style twin `# E: cyclical reference`), while recursion through a
/// real constructor INSIDE them stays valid.
#[test]
fn union_spelled_self_reference_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    for source in [
        "type X = Union[int, X]\n",
        "type Y = Optional[Y]\n",
        "type Z = Annotated[Z, \"meta\"]\n",
    ] {
        let diags = run(source)?;
        assert!(
            messages_for(&diags, "generics_syntax_scoping")
                .iter()
                .any(|m| m.contains("Circular")),
            "Union/Optional/Annotated-spelled self-reference must fire.\nsource: {source}"
        );
    }
    for source in [
        "type A = Union[int, list[A]]\n",
        "type B = Optional[list[B]]\n",
    ] {
        let diags = run(source)?;
        assert!(
            !codes(&diags).contains(&"generics_syntax_scoping"),
            "guarded recursion inside a transparent form is valid.\nsource: {source}\n got: {:?}",
            messages_for(&diags, "generics_syntax_scoping")
        );
    }
    Ok(())
}

/// One diagnostic per circular alias: an alias flagged as unguarded by the
/// acceptance pass must not be reported AGAIN by the mutual-cycle pass at
/// the same span.
#[test]
fn circular_alias_is_reported_exactly_once() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("type A = A | B\ntype B = A\n")?;
    let circular: Vec<_> = messages_for(&diags, "generics_syntax_scoping")
        .into_iter()
        .filter(|m| m.contains("Circular"))
        .collect();
    assert_eq!(
        circular.len(),
        2,
        "exactly one circular diagnostic per alias (A unguarded, B in the chain): {circular:?}"
    );
    Ok(())
}

/// Mutual cycles hidden behind transparent forms — `Union[..]` subscripts
/// and string forward references — are still cycles: no arm ever reaches a
/// constructor head.
#[test]
fn mutual_cycle_through_transparent_forms_fires() -> Result<(), Box<dyn std::error::Error>> {
    for source in [
        "type A = Union[int, B]\ntype B = A\n",
        "type A = \"B\"\ntype B = A\n",
    ] {
        let diags = run(source)?;
        assert!(
            messages_for(&diags, "generics_syntax_scoping")
                .iter()
                .any(|m| m.contains("Circular")),
            "a mutual cycle through a transparent form must fire.\nsource: {source}"
        );
    }
    // A constructor inside the transparent form guards: NOT a cycle.
    let diags = run("type A = Union[int, list[B]]\ntype B = A\n")?;
    assert!(
        !codes(&diags).contains(&"generics_syntax_scoping"),
        "recursion through list[..] inside Union[..] is valid, got: {:?}",
        messages_for(&diags, "generics_syntax_scoping")
    );
    Ok(())
}
