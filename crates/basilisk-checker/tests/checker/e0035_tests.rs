// Integration tests for BSK-E0035: Required/NotRequired in invalid context.

use super::common::*;

#[test]
fn e0035_required_outside_typeddict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Required

class NotTypedDict:
    x: Required[int] = 0
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0035"),
        "Required outside TypedDict should fire E0035, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0035_required_in_typeddict_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypedDict, Required

class Movie(TypedDict, total=False):
    name: Required[str]
    year: int
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0035"),
        "Required inside TypedDict should not fire E0035"
    );
    Ok(())
}

#[test]
fn e0035_required_in_function_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Required

def func(x: Required[int]) -> None:
    pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0035"),
        "Required in function param should fire E0035, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
