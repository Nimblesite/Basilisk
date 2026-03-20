// Integration tests for BSK-E0091: `TypeVar` default incompatible.

use super::common::*;

#[test]
fn e0091_typevar_default_incompat_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", bound=int, default=str)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0091_valid_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T", bound=int, default=int)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0091"),
        "compatible default should not fire E0091"
    );
    Ok(())
}
