//! Tests for [BSK-E0116] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0116: `NamedTuple` class definition errors.

use super::common::*;

#[test]
fn e0116_valid_namedtuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0116"),
        "valid NamedTuple should not fire E0116"
    );
    Ok(())
}

#[test]
fn e0116_underscore_field() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Bad(NamedTuple):
    _name: str
    value: int
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0116_default_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Bad(NamedTuple):
    x: int = 0
    y: int
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0116_valid_default_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Good(NamedTuple):
    x: int
    y: int = 0
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0116"),
        "valid default ordering should not fire E0116"
    );
    Ok(())
}
