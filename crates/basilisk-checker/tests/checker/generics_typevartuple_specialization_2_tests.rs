//! Tests for [`generics_typevartuple_specialization_2`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for generics_typevartuple_specialization_2: `TypeVarTuple` specialization violations.

use super::common::*;

#[test]
fn valid_specialization() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

IntTupleGeneric = tuple[int, T]
x: IntTupleGeneric[str] = (1, "hello")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_typevartuple_specialization_2"),
        "valid specialization should not fire E0139"
    );
    Ok(())
}

#[test]
fn unpack_on_non_typevar_tuple() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, TypeVarTuple

T = TypeVar("T")
Ts = TypeVarTuple("Ts")

IntTupleGeneric = tuple[int, T]
x: IntTupleGeneric[*Ts] = (1,)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
