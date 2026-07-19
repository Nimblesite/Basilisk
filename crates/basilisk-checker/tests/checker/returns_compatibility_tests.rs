//! Tests for [`returns_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for returns_compatibility: Return type mismatch. (The explicit-`Any`
// warning was split out to BSK-0014 — see w0014_tests.rs.)

use super::common::*;

#[test]
fn return_type_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def count() -> str:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"returns_compatibility"),
        "returning int from -> str function should fire E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn correct_return_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def count() -> int:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility"),
        "correct return type should not fire E0011"
    );
    Ok(())
}

#[test]
fn str_annotation_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def greet(name: str) -> str:\n    return name\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility"),
        "str annotation should not fire E0011"
    );
    Ok(())
}

#[test]
fn return_mismatch_stub_exempt() -> Result<(), Box<dyn std::error::Error>> {
    // Return type mismatch check is skipped for stub bodies
    let source = "def count() -> str:\n    ...\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility"),
        "stub body should not fire return type mismatch E0011"
    );
    Ok(())
}

#[test]
fn literal_target_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // `return True` infers `Bool`, not `Literal[True]`; the kind-only return
    // inference cannot verify a `Literal[...]` target, so E0011 must NOT fire
    // (matches __exit__ -> Literal[True] in conformance exceptions_context_managers.py).
    let source = "from typing import Literal\ndef ok() -> Literal[True]:\n    return True\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility"),
        "Literal[True] target must not fire E0011 (value-less inference is unverifiable), got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn quoted_forward_ref_union_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // A quoted forward-ref union annotation parses into `Named` fragments with no
    // concrete member; E0011 must skip it rather than flag a valid `return 1`
    // (conformance constructors_call_metaclass.py).
    let source = "class Meta2: ...\ndef f() -> \"int | Meta2\":\n    return 1\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility"),
        "quoted forward-ref union target must not fire E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn concrete_mismatch_still_fires_after_guard() -> Result<(), Box<dyn std::error::Error>> {
    // The unverifiability guard must NOT suppress genuine, concrete mismatches.
    let source = "def f() -> None:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"returns_compatibility"),
        "returning a value from -> None must still fire E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn empty_list_return_is_checked_in_declared_context() -> Result<(), Box<dyn std::error::Error>> {
    // A literal expression is inferred in its expected return context. `list`
    // remains invariant for already-typed values, but an empty literal can
    // construct a `list[bytes]` without first becoming a `list[Never]` value.
    let source = "def make_bytes() -> list[bytes]:\n    return []\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility"),
        "empty list literal must be valid in a list[bytes] return context, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn empty_list_return_is_valid_for_optional_list() -> Result<(), Box<dyn std::error::Error>> {
    // Contextual typing distributes through `Optional` (`list[int] | None`): an
    // empty literal fits the list arm (guards the `Optional` recursion arm).
    let source = "def g() -> list[int] | None:\n    return []\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility"),
        "empty list literal must be valid for a list[int] | None return, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn list_literal_with_wrong_element_still_errors() -> Result<(), Box<dyn std::error::Error>> {
    // Contextual literal typing ([TYPEINF-SPECIAL-LITERAL-CONTEXT]) is covariant
    // per element, not a blanket pass: a genuine element mismatch must still fire.
    let source = "def f() -> list[str]:\n    return [1]\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"returns_compatibility"),
        "list[int] literal must not satisfy a list[str] return, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn list_literal_with_one_bad_element_still_errors() -> Result<(), Box<dyn std::error::Error>> {
    // Every element must match — one incompatible element fails the whole literal
    // (guards the `all`, not `any`, semantics of the per-element check).
    let source = "def f() -> list[str]:\n    return [\"ok\", 2]\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"returns_compatibility"),
        "a list literal with a non-str element must not satisfy list[str], got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn list_literal_matching_no_union_member_still_errors() -> Result<(), Box<dyn std::error::Error>> {
    // A literal that fits no arm of the union must still fail (guards the union
    // fold that only returns `Some(true)` when a member accepts the literal).
    let source = "def f() -> list[str] | None:\n    return [1]\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"returns_compatibility"),
        "list[int] literal must not satisfy list[str] | None, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn dict_literal_with_wrong_value_still_errors() -> Result<(), Box<dyn std::error::Error>> {
    // The dict key widens (LiteralString -> str) but a genuinely wrong value
    // type must still fire, proving contextual typing is not a blanket pass.
    let source = "def f() -> dict[str, int]:\n    return {\"k\": \"v\"}\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"returns_compatibility"),
        "dict value mismatch must still fire against dict[str, int], got: {:?}",
        codes(&diags)
    );
    Ok(())
}
