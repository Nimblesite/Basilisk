//! Tests for [dataclasses_order] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-COERCION
// Integration tests for dataclasses_order: dataclass ordering invalid.

use super::common::*;

#[test]
fn e0060_comparison_without_order() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

p1 = Point(1, 2)
p2 = Point(3, 4)
result = p1 < p2
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0060_comparison_with_order_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass(order=True)
class Point:
    x: int
    y: int

p1 = Point(1, 2)
p2 = Point(3, 4)
result = p1 < p2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_order"),
        "order=True dataclass comparison should not fire E0060"
    );
    Ok(())
}

#[test]
fn e0060_eq_comparison_always_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

p1 = Point(1, 2)
p2 = Point(3, 4)
result = p1 == p2
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_order"),
        "== comparison always valid for dataclass"
    );
    Ok(())
}
