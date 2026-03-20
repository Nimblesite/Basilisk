// Integration tests for BSK-E0072: No matching overload.

use super::common::*;

#[test]
fn e0072_no_matching_overload_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: int | str) -> int | str:
    return x

process(1.0)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0072_matching_overload_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: int | str) -> int | str:
    return x

process(1)
process('hello')
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0072"),
        "matching overload should not fire E0072"
    );
    Ok(())
}
