//! Tests for [`directives_reveal_type`]-[`directives_assert_type`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for directives_reveal_type (invalid `reveal_type`) and directives_assert_type (invalid `assert_type`).

use super::common::*;

// --- Invalid reveal_type ---

#[test]
fn reveal_type_zero_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "reveal_type()\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_reveal_type"),
        "reveal_type() with 0 args should fire E0033, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn reveal_type_two_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "reveal_type(1, 2)\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_reveal_type"),
        "reveal_type() with 2 args should fire E0033, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn reveal_type_one_arg_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "x: int = 42\nreveal_type(x)\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_reveal_type"),
        "reveal_type() with 1 arg should not fire E0033"
    );
    Ok(())
}

// --- Invalid assert_type ---

#[test]
fn assert_type_zero_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import assert_type\nassert_type()\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_assert_type"),
        "assert_type() with 0 args should fire E0039, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn assert_type_one_arg_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import assert_type\nassert_type(42)\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_assert_type"),
        "assert_type() with 1 arg should fire E0039, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn assert_type_three_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import assert_type\nassert_type(42, int, str)\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_assert_type"),
        "assert_type() with 3 args should fire E0039, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn assert_type_two_args_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import assert_type\nassert_type(42, int)\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_assert_type"),
        "assert_type() with 2 args should not fire E0039"
    );
    Ok(())
}
