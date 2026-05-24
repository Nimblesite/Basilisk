//! Tests for [BSK-E0044] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for BSK-E0044: Final used in invalid position.

use super::common::*;

#[test]
fn e0044_valid_module_final_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final
X: Final[int] = 42
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0044");
    assert!(
        msgs.is_empty(),
        "valid module-level Final should not fire E0044, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0044_final_in_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final
def f(x: Final[int]) -> None:
    pass
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0044");
    assert!(
        !msgs.is_empty(),
        "Final in function parameter should fire E0044, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0044_final_nested_in_list_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final
x: list[Final[int]] = []
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0044");
    assert!(
        !msgs.is_empty(),
        "Final nested in list should fire E0044, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0044_classvar_with_final_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar, Final

class MyClass:
    VALUE: ClassVar[Final[int]] = 1
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0044");
    assert!(
        !msgs.is_empty(),
        "ClassVar[Final[...]] should fire E0044, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0044_bare_final_no_assignment_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final
BAD: Final
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0044");
    assert!(
        !msgs.is_empty(),
        "bare Final with no assignment should fire E0044, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0044_final_too_many_type_args_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Final
BAD: Final[str, int] = ""
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0044");
    assert!(
        !msgs.is_empty(),
        "Final[str, int] should fire E0044, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0044_final_in_class_body_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Final

class MyClass:
    VALUE: Final[int] = 42
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "BSK-E0044");
    assert!(
        msgs.is_empty(),
        "Final in class body should not fire E0044, got: {msgs:?}"
    );
    Ok(())
}
