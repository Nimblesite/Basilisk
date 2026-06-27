//! Tests for [protocols_definition] from [CHKARCH-DIAG-QUALITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-QUALITY
// Integration tests for protocols_definition: Protocol self attribute violation.

use super::common::*;

#[test]
fn valid_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Proto(Protocol):
    def method(self) -> int: ...
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_definition"),
        "valid protocol should not fire E0097"
    );
    Ok(())
}

#[test]
fn undeclared_self_attr_in_method_body() -> Result<(), Box<dyn std::error::Error>> {
    // PEP 544: attributes set via `self` in ANY method (not just __init__) must
    // be declared. `name` is declared and must not fire; `temp` must.
    let source = r"
from typing import Protocol

class Proto(Protocol):
    name: str
    def method(self) -> None:
        self.name = 'ok'
        self.temp: list[int] = []
";
    let diags = run(source)?;
    let msgs = messages_for(&diags, "protocols_definition");
    assert!(
        msgs.iter().any(|m| m.contains("temp")),
        "undeclared `temp` in a non-init method must fire E0097, got: {msgs:?}"
    );
    assert!(
        !msgs.iter().any(|m| m.contains("`name`")),
        "declared `name` must not be flagged, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn staticmethod_param_is_not_a_receiver() -> Result<(), Box<dyn std::error::Error>> {
    // A `@staticmethod`'s first parameter is not `self`, so assigning to its
    // attribute is not an undeclared-self-attribute violation.
    let source = r"
from typing import Protocol

class Proto(Protocol):
    @staticmethod
    def make(target) -> None:
        target.cache = 1
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_definition"),
        "a static method's parameter is not an instance receiver"
    );
    Ok(())
}
