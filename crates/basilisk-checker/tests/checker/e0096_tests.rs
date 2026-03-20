// Integration tests for BSK-E0096: Dataclass field default factory mismatch.

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
        !codes(&diags).contains(&"BSK-E0096"),
        "valid default_factory should not fire E0096"
    );
    Ok(())
}
