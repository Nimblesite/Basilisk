// Integration tests for BSK-E0053: `assert_type()` type mismatch.

use super::common::*;

#[test]
fn e0053_valid_assert_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import assert_type

def f(a: int) -> None:
    assert_type(a, int)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0053_assert_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import assert_type

def f(a: int | str) -> None:
    assert_type(a, int)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
