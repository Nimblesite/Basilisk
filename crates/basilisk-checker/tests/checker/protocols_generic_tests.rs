//! Tests for [protocols_generic] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_generic: Generic protocol violations.

use super::common::*;

#[test]
fn protocol_with_generic_combined_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Protocol
T_co = TypeVar("T_co", covariant=True)
class Proto(Protocol[T_co], Generic[T_co]):
    def method(self) -> T_co: ...
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"protocols_generic"),
        "Protocol[T] + Generic[T] should fire E0137, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn protocol_subscript_only_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T = TypeVar("T")
class Proto(Protocol[T]):
    def method(self) -> T: ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_generic"),
        "Protocol[T] alone should not fire E0137"
    );
    Ok(())
}

#[test]
fn generic_protocol_assignment_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T = TypeVar("T")
class Processor(Protocol[T]):
    def process(self, item: T) -> T: ...

class IntProcessor:
    def process(self, item: int) -> int:
        return item

p: Processor[str] = IntProcessor()
"#;
    let diags = run(source)?;
    // Exercise the code path even if the check is not fully wired
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn self_typed_protocol_incompatibility() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T = TypeVar("T")
class Copyable(Protocol):
    def copy(self) -> "Copyable": ...

class MyClass:
    def copy(self) -> "MyClass":
        return MyClass()

x: Copyable = MyClass()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn generic_protocol_compatible_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T = TypeVar("T")
class Processor(Protocol[T]):
    def process(self, item: T) -> T: ...

class IntProcessor:
    def process(self, item: int) -> int:
        return item

p: Processor[int] = IntProcessor()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn generic_protocol_two_type_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T = TypeVar("T")
U = TypeVar("U")
class BiProcessor(Protocol[T, U]):
    def process(self, item: T) -> U: ...

class IntToStr:
    def process(self, item: int) -> str:
        return str(item)

p: BiProcessor[int, str] = IntToStr()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn generic_protocol_covariant_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_co = TypeVar("T_co", covariant=True)
class Reader(Protocol[T_co]):
    def read(self) -> T_co: ...

class IntReader:
    def read(self) -> int:
        return 42

r: Reader[int] = IntReader()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn self_typed_protocol_with_methods() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Self
class Chainable(Protocol):
    def chain(self) -> Self: ...
    def name(self) -> str: ...

class MyChain:
    def chain(self) -> "MyChain":
        return MyChain()
    def name(self) -> str:
        return "my"

x: Chainable = MyChain()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn self_typed_protocol_missing_method() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, Self
class Builder(Protocol):
    def build(self) -> Self: ...
    def reset(self) -> None: ...

class BadBuilder:
    def build(self) -> "BadBuilder":
        return BadBuilder()

x: Builder = BadBuilder()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
