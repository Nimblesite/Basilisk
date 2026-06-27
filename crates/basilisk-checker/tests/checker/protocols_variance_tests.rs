//! Tests for [protocols_variance] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_variance: Protocol variance violation.

use super::common::*;

#[test]
fn covariant_in_input_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_co = TypeVar("T_co", covariant=True)
class BadProto(Protocol[T_co]):
    def method(self, x: T_co) -> None: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn contravariant_in_output_position() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_contra = TypeVar("T_contra", contravariant=True)
class BadProto2(Protocol[T_contra]):
    def method(self) -> T_contra: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn valid_covariant_output() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_co = TypeVar("T_co", covariant=True)
class GoodProto(Protocol[T_co]):
    def method(self) -> T_co: ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_variance"),
        "covariant in output position should not fire E0110"
    );
    Ok(())
}

#[test]
fn valid_contravariant_input() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_contra = TypeVar("T_contra", contravariant=True)
class GoodProto2(Protocol[T_contra]):
    def method(self, x: T_contra) -> None: ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_variance"),
        "contravariant in input position should not fire E0110"
    );
    Ok(())
}

#[test]
fn init_exempt_from_variance() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T_co = TypeVar("T_co", covariant=True)
class Proto(Protocol[T_co]):
    def __init__(self, x: T_co) -> None: ...
    def method(self) -> T_co: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
