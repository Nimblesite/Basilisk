// Integration tests for BSK-E0078: Self type violation.

use super::common::*;

#[test]
fn e0078_self_type_violation_exercise() -> Result<(), Box<dyn std::error::Error>> {
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
