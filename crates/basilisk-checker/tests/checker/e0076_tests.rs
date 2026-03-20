// Integration tests for BSK-E0076: Overload union expansion.

use super::common::*;

#[test]
fn e0076_overload_union_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import overload, Union

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: Union[int, str]) -> Union[int, str]:
    return x

result: int = process(42)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
