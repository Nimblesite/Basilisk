//! Tests for [protocols_runtime_checkable] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_runtime_checkable: Protocol isinstance/issubclass violations.

use super::common::*;

#[test]
fn e0114_runtime_checkable_isinstance_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Proto(Protocol):
    def meth(self) -> int: ...

x: object = None
isinstance(x, Proto)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_runtime_checkable"),
        "runtime_checkable protocol isinstance should not fire E0114"
    );
    Ok(())
}

#[test]
fn e0114_non_runtime_checkable_isinstance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Proto(Protocol):
    def meth(self) -> int: ...

x: object = None
isinstance(x, Proto)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0114_issubclass_data_protocol() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol, runtime_checkable

@runtime_checkable
class Proto(Protocol):
    name: str
    def method(self) -> int: ...

issubclass(int, Proto)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
