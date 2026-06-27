//! Tests for [generics_self_protocols] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
// Integration tests for generics_self_protocols: Protocol self return.

use super::common::*;

#[test]
fn e0077_protocol_self_return_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, Self

class Cloneable(Protocol):
    def clone(self) -> Self: ...

class MyClass:
    def clone(self) -> 'MyClass':
        return MyClass()

x: Cloneable = MyClass()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
