//! Tests for [BSK-E0004] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
// Integration tests for BSK-E0004: Missing *args/**kwargs type annotation.

use super::common::*;

#[test]
fn e0004_unannotated_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_strict("def foo(*args) -> None:\n    pass\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0004"),
        "unannotated *args should fire E0004, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0004_unannotated_kwargs_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_strict("def foo(**kwargs) -> None:\n    pass\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0004"),
        "unannotated **kwargs should fire E0004, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0004_annotated_args_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_strict("def foo(*args: int) -> None:\n    pass\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0004"),
        "annotated *args should not fire E0004"
    );
    Ok(())
}

#[test]
fn e0004_annotated_kwargs_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_strict("def foo(**kwargs: str) -> None:\n    pass\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0004"),
        "annotated **kwargs should not fire E0004"
    );
    Ok(())
}

#[test]
fn e0004_both_unannotated_fires_twice() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_strict("def foo(*args, **kwargs) -> None:\n    pass\n")?;
    let count = diags.iter().filter(|d| d.code.code == "BSK-E0004").count();
    assert_eq!(
        count, 2,
        "both unannotated *args and **kwargs should fire E0004"
    );
    Ok(())
}

#[test]
fn e0004_stub_body_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_strict("def foo(*args) -> None:\n    ...\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0004"),
        "stub body should be exempt from E0004"
    );
    Ok(())
}
