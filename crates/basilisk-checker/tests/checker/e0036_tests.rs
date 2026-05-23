//! Tests for [BSK-E0036] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for BSK-E0036: `ClassVar` used in invalid context.

use super::common::*;

#[test]
fn e0036_classvar_in_class_body_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    count: ClassVar[int] = 0
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0036");
    assert!(
        msgs.is_empty(),
        "ClassVar in class body should not fire E0036, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_module_var_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar
bad: ClassVar[int] = 3
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0036");
    assert!(
        !msgs.is_empty(),
        "ClassVar at module level should fire E0036, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_function_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    def method(self, a: ClassVar[int]) -> None:
        pass
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0036");
    assert!(
        !msgs.is_empty(),
        "ClassVar in function param should fire E0036, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_return_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    def method(self) -> ClassVar[int]:
        return 0
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0036");
    assert!(
        !msgs.is_empty(),
        "ClassVar in return type should fire E0036, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_local_var_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import ClassVar

class MyClass:
    def method(self) -> None:
        x: ClassVar[str] = ""
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0036");
    assert!(
        !msgs.is_empty(),
        "ClassVar in local variable should fire E0036, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0036_nested_classvar_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar, Final

class MyClass:
    bad: Final[ClassVar[int]] = 3
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0036");
    assert!(
        !msgs.is_empty(),
        "nested ClassVar should fire E0036, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0036_classvar_in_list_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar

class MyClass:
    bad: list[ClassVar[int]] = []
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0036");
    assert!(
        !msgs.is_empty(),
        "ClassVar nested in list should fire E0036, got: {msgs:?}"
    );
    Ok(())
}
