//! Tests for [BSK-E0099] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Integration tests for BSK-E0099: Protocol instantiation.

use super::common::*;

#[test]
fn e0099_direct_protocol_instantiation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class MyProto(Protocol):
    def method(self) -> int: ...

obj = MyProto()
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0099"),
        "direct Protocol instantiation should fire E0099, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0099_non_protocol_class_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class MyClass:
    def method(self) -> int:
        return 42

obj = MyClass()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0099"),
        "non-Protocol instantiation should not fire E0099"
    );
    Ok(())
}
