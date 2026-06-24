//! Tests for [BSK-E0160] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
// Integration tests for BSK-E0160: overload implementation inconsistent with its signatures.

use super::common::*;

#[test]
fn e0160_return_not_assignable_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload\n@overload\ndef f(x: int) -> int: ...\n@overload\ndef f(x: str) -> str: ...\ndef f(x: int | str) -> int:\n    return 1\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0160"),
        "an overload returning str is not assignable to impl return int; must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0160_param_not_acceptable_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload\n@overload\ndef f(x: int) -> int: ...\n@overload\ndef f(x: str) -> str: ...\ndef f(x: int) -> int | str:\n    return 1\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0160"),
        "impl param int cannot accept overload param str; must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0160_consistent_overloads_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import overload\n@overload\ndef f(x: int) -> int: ...\n@overload\ndef f(x: str) -> str: ...\ndef f(x: int | str) -> int | str:\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0160"),
        "impl with union return/param accepting all overloads must not fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0160_non_primitive_types_skipped() -> Result<(), Box<dyn std::error::Error>> {
    // TypeVar returns cannot be compared textually; the rule must stay silent.
    let source = "from typing import overload, TypeVar\nT = TypeVar('T')\n@overload\ndef f(x: int) -> list[int]: ...\n@overload\ndef f(x: str) -> T: ...\ndef f(x):\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0160"),
        "non-primitive/TypeVar annotations must be skipped (no false positive)"
    );
    Ok(())
}
