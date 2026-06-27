//! Tests for [`literals_parameterizations`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// Integration tests for literals_parameterizations: Invalid Literal parameterization.

use super::common::*;

#[test]
fn valid_int_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[1] = 1
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(msgs.is_empty(), "valid Literal[1] should not fire E0051");
    Ok(())
}

#[test]
fn valid_str_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal["hello"] = "hello"
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(
        msgs.is_empty(),
        "valid Literal[\"hello\"] should not fire E0051"
    );
    Ok(())
}

#[test]
fn valid_bool_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[True] = True
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(msgs.is_empty(), "valid Literal[True] should not fire E0051");
    Ok(())
}

#[test]
fn valid_none_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[None] = None
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(msgs.is_empty(), "valid Literal[None] should not fire E0051");
    Ok(())
}

#[test]
fn float_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[3.14] = 3.14
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(
        !msgs.is_empty(),
        "Literal[3.14] should fire E0051, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn bare_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(
        !msgs.is_empty(),
        "bare Literal should fire E0051, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn type_object_in_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[int]
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(
        !msgs.is_empty(),
        "Literal[int] should fire E0051, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn ellipsis_in_literal_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[...]
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(
        !msgs.is_empty(),
        "Literal[...] should fire E0051, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn valid_negative_int_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
x: Literal[-1] = -1
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(msgs.is_empty(), "Literal[-1] should not fire E0051");
    Ok(())
}

#[test]
fn valid_enum_member_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Literal
from enum import Enum

class Color(Enum):
    RED = 1

x: Literal[Color.RED]
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "literals_parameterizations");
    assert!(msgs.is_empty(), "Literal[Color.RED] should not fire E0051");
    Ok(())
}
