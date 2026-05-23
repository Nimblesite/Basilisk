//! Tests for [BSK-E0098] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Integration tests for BSK-E0098: Non-protocol base in Protocol.

use super::common::*;

#[test]
fn e0098_non_protocol_base_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Base:
    pass

class MyProto(Protocol, Base):
    def method(self) -> None: ...
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0098"),
        "Protocol with non-Protocol base should fire E0098, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0098_protocol_only_bases_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class MyProto(Protocol):
    def method(self) -> None: ...
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0098"),
        "Protocol with only Protocol base should not fire E0098"
    );
    Ok(())
}
