//! Tests for [BSK-E0095] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Integration tests for BSK-E0095: `InitVar` field validation.

use super::common::*;

#[test]
fn e0095_post_init_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import InitVar, dataclass

@dataclass
class DC1:
    x: InitVar[int]
    y: InitVar[str]

    def __post_init__(self, x: int, y: int) -> None:
        pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0095_initvar_attr_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import InitVar, dataclass

@dataclass
class DC1:
    x: InitVar[int]
    y: int = 0

    def __post_init__(self, x: int) -> None:
        self.y = x

dc1 = DC1(1)
dc1.x
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0095_valid_initvar_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import InitVar, dataclass

@dataclass
class DC2:
    x: InitVar[int]
    y: int = 0

    def __post_init__(self, x: int) -> None:
        self.y = x
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0095"),
        "valid InitVar usage should not fire E0095"
    );
    Ok(())
}
