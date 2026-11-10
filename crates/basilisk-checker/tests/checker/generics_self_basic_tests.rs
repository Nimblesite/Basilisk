//! Tests for [`generics_self_basic`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
// Integration tests for generics_self_basic: Self type violation.

use super::common::*;

#[test]
fn self_type_violation_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Base:
    def copy(self) -> Self:
        return Base()

class Child(Base):
    pass

c = Child()
result = c.copy()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
