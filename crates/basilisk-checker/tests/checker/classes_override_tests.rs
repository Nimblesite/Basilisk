//! Tests for [classes_override] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for classes_override: Incompatible method override.

use super::common::*;

#[test]
fn e0016_incompatible_param_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: int) -> str:
        return str(data)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"classes_override"),
        "incompatible param type in @override should fire E0016, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0016_incompatible_return_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: str) -> int:
        return 42
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"classes_override"),
        "incompatible return type in @override should fire E0016, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0016_compatible_override_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: str) -> str:
        return data.upper()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"classes_override"),
        "compatible override should not fire E0016"
    );
    Ok(())
}
