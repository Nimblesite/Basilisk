//! Tests for [`specialtypes_never_2`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
// Integration tests for specialtypes_never_2: Never type compatibility.

use super::common::*;

#[test]
fn never_type_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never

def never_returns() -> Never:
    raise RuntimeError('never')

x: int = never_returns()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn never_as_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Never

def impossible(x: Never) -> None:
    pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
