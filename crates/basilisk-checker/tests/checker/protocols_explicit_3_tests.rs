//! Tests for [`protocols_explicit_3`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_explicit_3: Super call on abstract protocol method.

use super::common::*;

#[test]
fn super_on_protocol_abstract() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol
from abc import abstractmethod

class PColor(Protocol):
    @abstractmethod
    def draw(self) -> str:
        ...

class BadColor(PColor):
    def draw(self) -> str:
        return super().draw()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn super_on_protocol_with_default() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol

class PColor(Protocol):
    def draw(self) -> str:
        return "default"

class GoodColor(PColor):
    def draw(self) -> str:
        return super().draw() + " extended"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_explicit_3"),
        "super() on protocol with default impl should not fire E0123"
    );
    Ok(())
}
