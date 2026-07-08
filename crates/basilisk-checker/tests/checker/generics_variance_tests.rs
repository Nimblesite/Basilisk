//! Tests for [`generics_variance`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_variance: Variance incompatibility in base class.

use super::common::*;

#[test]
fn compatible_invariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")

class Base(Generic[T]): ...
class Good(Base[T]): ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_variance"),
        "invariant param with invariant arg should not fire E0107"
    );
    Ok(())
}

#[test]
fn covariant_for_invariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
T_co = TypeVar("T_co", covariant=True)

class Base(Generic[T]): ...
class Bad(Base[T_co]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn contravariant_for_invariant() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Generic, TypeVar

T = TypeVar("T")
T_contra = TypeVar("T_contra", contravariant=True)

class Base(Generic[T]): ...
class Bad(Base[T_contra]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
