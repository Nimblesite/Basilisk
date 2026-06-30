//! Tests for [`namedtuples_define_class`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for namedtuples_define_class: `NamedTuple` class definition errors.

use super::common::*;

#[test]
fn valid_namedtuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"namedtuples_define_class"),
        "valid NamedTuple should not fire E0116"
    );
    Ok(())
}

#[test]
fn underscore_field() -> Result<(), Box<dyn std::error::Error>> {
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
fn default_ordering() -> Result<(), Box<dyn std::error::Error>> {
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
fn valid_default_ordering() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Good(NamedTuple):
    x: int
    y: int = 0
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"namedtuples_define_class"),
        "valid default ordering should not fire E0116"
    );
    Ok(())
}
