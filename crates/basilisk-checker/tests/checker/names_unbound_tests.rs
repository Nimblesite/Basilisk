//! Tests for [`names_unbound`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! Integration tests for names_unbound: unbound variable on some code paths.
//!
//! [NARROWPLAN-INTEGRATION] Step 8
//! ([#285](https://github.com/Nimblesite/Basilisk/issues/285)): the rule runs
//! a definite-assignment walk with the walker's inference-driven divergence
//! ([NARROWPLAN-FLOW]) — the divergence tests below are mutation-resistant
//! pins: each no-diagnostic case passes ONLY because a diverging branch drops
//! out of the merge, and each is paired with a firing case that keeps the
//! diagnostic alive.

use super::common::*;

#[test]
fn conditionally_assigned_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def maybe_assign(flag: bool) -> int:
    if flag:
        result = 42
    return result
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_unbound"),
        "conditionally assigned variable should fire E0019, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn unconditionally_assigned_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def always_assign() -> int:
    result = 42
    return result
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "unconditionally assigned variable should not fire E0019"
    );
    Ok(())
}

#[test]
fn assigned_in_try_and_except_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Regression test for issue #285: `o` is assigned in both the `try` body
    // and the `except` handler, so it is bound on every path that reaches the
    // `return` — no diagnostic.
    let source = r"
def occ(loc: int) -> int:
    try:
        o = loc
    except KeyError:
        o = 0
        print(o)
    else:
        print(o)
    return o
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "variable assigned in both try and except is always bound; should not fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn parameter_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def identity(x: int) -> int:\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_unbound"),
        "parameter should not fire E0019"
    );
    Ok(())
}
