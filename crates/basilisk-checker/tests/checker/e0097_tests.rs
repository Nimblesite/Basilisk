// Integration tests for BSK-E0097: Protocol self attribute violation.

use super::common::*;

#[test]
fn e0097_valid_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Proto(Protocol):
    def method(self) -> int: ...
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0097"),
        "valid protocol should not fire E0097"
    );
    Ok(())
}
