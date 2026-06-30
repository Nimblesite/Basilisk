//! Tests for [`protocols_class_objects_2`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_class_objects_2: Protocol class object violations.

use super::common::*;

#[test]
fn concrete_subtype_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Proto(Protocol):
    def meth(self) -> int: ...

class Concrete:
    def meth(self) -> int:
        return 42

def fun(cls: type[Proto]) -> int:
    return cls().meth()

fun(Concrete)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_class_objects_2"),
        "concrete subtype should not fire E0146"
    );
    Ok(())
}

#[test]
fn protocol_class_passed() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Proto(Protocol):
    def meth(self) -> int: ...

def fun(cls: type[Proto]) -> int:
    return cls().meth()

fun(Proto)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn protocol_class_assigned() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Proto(Protocol):
    def meth(self) -> int: ...

var: type[Proto]
var = Proto
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
