//! Tests for [BSK-E0047] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for BSK-E0047: Invalid type expression.

use super::common::*;

#[test]
fn e0047_invalid_type_expr_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: 1 + 2
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_valid_type_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: int = 42
y: list[str] = []
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0047"),
        "valid type annotations should not fire E0047"
    );
    Ok(())
}

#[test]
fn e0047_string_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
x: "int" = 42
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_union_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: int | str = 42
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_nested_generic_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: dict[str, list[int]] = {}
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_callable_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
x: Callable[[int, str], bool] = lambda a, b: True
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_optional_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Optional
x: Optional[int] = None
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_function_param_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: int, y: str, z: list[float]) -> dict[str, int]:
    return {}
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_class_attribute_annotations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import ClassVar, Final
class MyClass:
    x: ClassVar[int] = 1
    y: Final[str] = 'hello'
    z: int
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_tuple_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
x: tuple[int, str, float] = (1, 'a', 1.0)
y: tuple[int, ...] = (1, 2, 3)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeAlias
MyType: TypeAlias = list[int]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_annotated_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated
x: Annotated[int, "metadata"] = 42
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_literal_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Literal
x: Literal["a", "b", "c"] = "a"
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0047_pep695_type_alias() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
type Vector = list[float]
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
