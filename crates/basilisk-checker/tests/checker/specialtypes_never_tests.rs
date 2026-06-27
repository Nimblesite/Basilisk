//! Tests for [specialtypes_never] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
// Integration tests for specialtypes_never: NoReturn/Never function can fall through.

use super::common::*;

#[test]
fn noreturn_with_fallthrough_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NoReturn

def bad(x: int) -> NoReturn:
    if x != 0:
        raise RuntimeError("error")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"specialtypes_never"),
        "NoReturn with fallthrough should fire E0062, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn noreturn_always_raises_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NoReturn

def stop() -> NoReturn:
    raise RuntimeError("no way")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"specialtypes_never"),
        "NoReturn that always raises should not fire E0062"
    );
    Ok(())
}

#[test]
fn normal_return_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def normal() -> int:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"specialtypes_never"),
        "normal return type should not fire E0062"
    );
    Ok(())
}
