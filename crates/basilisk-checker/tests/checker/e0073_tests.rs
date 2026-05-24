//! Tests for [BSK-E0073] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
// Integration tests for BSK-E0073: `NamedTuple` tuple compatibility.

use super::common::*;

#[test]
fn e0073_namedtuple_tuple_compat_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

t: tuple[str, str] = Point(1, 2)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0073_valid_tuple_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

t: tuple[int, int] = Point(1, 2)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0073"),
        "compatible tuple assignment should not fire E0073"
    );
    Ok(())
}
