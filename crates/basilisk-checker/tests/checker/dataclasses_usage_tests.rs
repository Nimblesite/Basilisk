//! Tests for [dataclasses_usage] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Integration tests for dataclasses_usage: Dataclass field default factory mismatch.

use super::common::*;

#[test]
fn e0096_valid_default_factory() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import dataclass, field

@dataclass
class DC:
    items: list[int] = field(default_factory=list)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dataclasses_usage"),
        "valid default_factory should not fire E0096"
    );
    Ok(())
}
