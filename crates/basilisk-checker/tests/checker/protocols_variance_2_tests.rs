//! Tests for [protocols_variance_2] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for protocols_variance_2: Protocol `TypeVar` variance mismatch.

use super::common::*;

#[test]
fn covariant_protocol_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T_co = TypeVar("T_co", covariant=True)

class MyProto(Protocol[T_co]):
    def method(self) -> T_co: ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"protocols_variance_2"),
        "correctly declared covariant protocol should not fire E0133"
    );
    Ok(())
}

#[test]
fn invariant_should_be_covariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Protocol, TypeVar

T = TypeVar("T")

class MyProto(Protocol[T]):
    def method(self) -> T: ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
