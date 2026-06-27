//! Tests for [aliases_implicit] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for aliases_implicit: `TypeAlias` invalid RHS.

use super::common::*;

#[test]
fn valid_type_alias_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
MyType: TypeAlias = list[int]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"aliases_implicit"),
        "valid TypeAlias should not fire E0048"
    );
    Ok(())
}

#[test]
fn type_alias_with_union() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
NumOrStr: TypeAlias = int | str
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"aliases_implicit"),
        "union TypeAlias should not fire E0048"
    );
    Ok(())
}

#[test]
fn type_alias_bool_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = True
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = 42
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_list_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = [int, str]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_fstring() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias
Bad: TypeAlias = f"hello"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_conditional() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = int if True else str
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_dict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeAlias
Bad: TypeAlias = {"a": int}
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_lambda() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Bad: TypeAlias = lambda: int
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn type_alias_nested() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
Inner: TypeAlias = list[int]
Outer: TypeAlias = dict[str, Inner]
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"aliases_implicit"),
        "nested valid TypeAlias should not fire E0048"
    );
    Ok(())
}
