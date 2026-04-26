//! Integration tests for basilisk-mojo.

#[test]
#[ignore = "Phase 4 not yet implemented — ownership checker is a stub"]
fn ownership_checker_detects_mutation_of_borrowed() {
    // Phase 4: mutation of a `Borrowed` parameter must be flagged as BSK-E0031.
    // Currently returns no violations (placeholder).
    let source = "def foo(x: Borrowed[list]) -> None:\n    x.append(1)\n";
    let violations = basilisk_mojo::check_ownership(source);
    assert!(
        !violations.is_empty(),
        "mutation of Borrowed must be detected — Phase 4 ownership checker not yet implemented"
    );
}

#[test]
fn ownership_checker_is_silent_for_clean_code() {
    // Phase 4: code with no ownership violations must produce no diagnostics.
    let source = "def foo(x: int) -> int:\n    return x\n";
    let violations = basilisk_mojo::check_ownership(source);
    assert!(
        violations.is_empty(),
        "clean code must produce no ownership violations"
    );
}
