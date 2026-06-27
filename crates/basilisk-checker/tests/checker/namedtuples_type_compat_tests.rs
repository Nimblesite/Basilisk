//! Tests for [namedtuples_type_compat] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
// Integration tests for namedtuples_type_compat: `NamedTuple` tuple compatibility.

use super::common::*;

#[test]
fn namedtuple_tuple_compat_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
fn valid_tuple_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int

t: tuple[int, int] = Point(1, 2)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"namedtuples_type_compat"),
        "compatible tuple assignment should not fire E0073"
    );
    Ok(())
}
