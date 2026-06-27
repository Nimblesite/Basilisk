//! Tests for [classes_override_2] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for classes_override_2: Incompatible class attribute override.

use super::common::*;

#[test]
fn incompatible_attr_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    count: int = 0

class Child(Base):
    count: str = "zero"
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"classes_override_2"),
        "incompatible attr type should fire E0017, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn compatible_attr_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    count: int = 0

class Child(Base):
    count: int = 10
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"classes_override_2"),
        "compatible attr type should not fire E0017"
    );
    Ok(())
}

#[test]
fn different_attr_name_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Base:
    count: int = 0

class Child(Base):
    name: str = "hello"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"classes_override_2"),
        "different attr name should not fire E0017"
    );
    Ok(())
}
