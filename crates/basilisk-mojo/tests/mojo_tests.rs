//! Tests for [CHKARCH-MOJO-SAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-MOJO-SAFETY
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for basilisk-mojo.

#[test]
fn ownership_checker_detects_mutation_of_borrowed() {
    let source = "def foo(x: Borrowed[list]) -> None:\n    x.append(1)\n";
    let violations = basilisk_mojo::check_ownership(source);
    assert!(
        !violations.is_empty(),
        "mutation of a Borrowed parameter must be detected"
    );
}

#[test]
fn ownership_checker_is_silent_for_clean_code() {
    let source = "def foo(x: int) -> int:\n    return x\n";
    let violations = basilisk_mojo::check_ownership(source);
    assert!(
        violations.is_empty(),
        "clean code must produce no ownership violations"
    );
}
