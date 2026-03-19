// Integration tests for BSK-E0129: Literal value assignment incompatibility.

use super::common::*;

#[test]
fn e0129_literal_0_vs_false() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
def func(a: Literal[0], b: Literal[False]) -> None:
    x1: Literal[False] = a
    x2: Literal[0] = b
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0129_augmented_assignment_widens() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
def func2(a: Literal[3, 4, 5]) -> None:
    a += 3
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0129_valid_literal_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
def func(a: Literal[1]) -> None:
    x: Literal[1] = a
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0129"),
        "matching literal assignment should not fire E0129"
    );
    Ok(())
}
