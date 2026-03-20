// Integration tests for BSK-E0021: Overlapping @overload signatures.

use super::common::*;

#[test]
fn e0021_identical_unannotated_overloads_fires() -> Result<(), Box<dyn std::error::Error>> {
    // Overlap detection requires at least one side to have unannotated params
    let source = r"
from typing import overload

@overload
def process(x) -> int: ...

@overload
def process(x) -> str: ...

def process(x: int) -> int:
    return x
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0021"),
        "identical unannotated overloads should fire E0021, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0021_distinct_overloads_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...

@overload
def process(x: str) -> str: ...

def process(x: int | str) -> int | str:
    return x
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0021"),
        "distinct overloads should not fire E0021"
    );
    Ok(())
}
