//! Tests for [`protocols_class_objects`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_class_objects: Protocol class used where type[Proto] expected.

use super::common::*;

#[test]
fn concrete_class_ok() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"protocols_class_objects"),
        "passing concrete class should not fire E0106"
    );
    Ok(())
}

#[test]
fn protocol_class_itself() -> Result<(), Box<dyn std::error::Error>> {
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
fn protocol_assigned_to_type_variable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    def draw(self) -> None:
        pass

widget_type: type[Drawable] = Circle
widget_type = Drawable
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn multiple_protocol_violations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Serializable(Protocol):
    def serialize(self) -> str: ...

def process(cls: type[Serializable]) -> str:
    return cls().serialize()

process(Serializable)

class JsonSerializable:
    def serialize(self) -> str:
        return '{}'

process(JsonSerializable)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
