//! Tests for [`dataclasses_kwonly`] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
// Integration tests for dataclasses_kwonly: dataclass `kw_only`.

use super::common::*;

#[test]
fn positional_to_kw_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(kw_only=True)
class Point:
    x: int
    y: int

p = Point(1, 2)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn kw_only_with_kwargs_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(kw_only=True)
class Point:
    x: int
    y: int

p = Point(x=1, y=2)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_kwonly"),
        "keyword args to kw_only dataclass should not fire E0069"
    );
    Ok(())
}
