//! Tests for [BSK-E0011] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0011: Return type mismatch. (The explicit-`Any`
// warning was split out to BSK-W0014 — see w0014_tests.rs.)

use super::common::*;

#[test]
fn e0011_return_type_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def count() -> str:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0011"),
        "returning int from -> str function should fire E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0011_correct_return_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def count() -> int:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0011"),
        "correct return type should not fire E0011"
    );
    Ok(())
}

#[test]
fn e0011_str_annotation_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def greet(name: str) -> str:\n    return name\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0011"),
        "str annotation should not fire E0011"
    );
    Ok(())
}

#[test]
fn e0011_return_mismatch_stub_exempt() -> Result<(), Box<dyn std::error::Error>> {
    // Return type mismatch check is skipped for stub bodies
    let source = "def count() -> str:\n    ...\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0011"),
        "stub body should not fire return type mismatch E0011"
    );
    Ok(())
}

#[test]
fn e0011_literal_target_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // `return True` infers `Bool`, not `Literal[True]`; the kind-only return
    // inference cannot verify a `Literal[...]` target, so E0011 must NOT fire
    // (matches __exit__ -> Literal[True] in conformance exceptions_context_managers.py).
    let source = "from typing import Literal\ndef ok() -> Literal[True]:\n    return True\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0011"),
        "Literal[True] target must not fire E0011 (value-less inference is unverifiable), got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0011_quoted_forward_ref_union_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // A quoted forward-ref union annotation parses into `Named` fragments with no
    // concrete member; E0011 must skip it rather than flag a valid `return 1`
    // (conformance constructors_call_metaclass.py).
    let source = "class Meta2: ...\ndef f() -> \"int | Meta2\":\n    return 1\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0011"),
        "quoted forward-ref union target must not fire E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0011_concrete_mismatch_still_fires_after_guard() -> Result<(), Box<dyn std::error::Error>> {
    // The unverifiability guard must NOT suppress genuine, concrete mismatches.
    let source = "def f() -> None:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0011"),
        "returning a value from -> None must still fire E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
