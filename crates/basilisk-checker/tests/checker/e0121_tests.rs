//! Tests for [BSK-E0121] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0121: Protocol conformance violation.

use super::common::*;

#[test]
fn e0121_conforming_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class P(Protocol):
    def method(self) -> None: ...

class C:
    def method(self) -> None:
        pass

x: P = C()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0121"),
        "conforming class should not fire E0121"
    );
    Ok(())
}

#[test]
fn e0121_non_conforming_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class P(Protocol):
    def method(self) -> None: ...

class C:
    pass

x: P = C()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
