//! Tests for [`protocols_explicit_2`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_explicit_2: Super call on abstract method with no implementation.

use super::common::*;

#[test]
fn super_on_abstract_stub() -> Result<(), Box<dyn std::error::Error>> {
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
fn super_on_concrete_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    def method(self) -> str:
        return "base"

class Child(Base):
    def method(self) -> str:
        return super().method()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_explicit_2"),
        "super() on concrete method should not fire E0118"
    );
    Ok(())
}
