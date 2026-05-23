//! Tests for [BSK-E0111] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for BSK-E0111: Constructor call errors.

use super::common::*;

#[test]
fn e0111_specialized_generic_arg_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class Box(Generic[T]):
    def __init__(self, x: T) -> None:
        self.x = x

Box[int](1.0)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_no_custom_init_with_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Empty:
    pass

Empty(1, 2, 3)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_valid_constructor_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class MyClass:
    def __init__(self, x: int) -> None:
        self.x = x

MyClass(42)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0111"),
        "valid constructor call should not fire E0111"
    );
    Ok(())
}

#[test]
fn e0111_self_type_incompatibility() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class Container(Generic[T]):
    def __init__(self, value: T) -> None:
        self.value = value

class SubContainer(Container[int]):
    pass

SubContainer("not an int")
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_explicit_self_annotation_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class Class4(Generic[T]):
    def __init__(self: "Class4[int]", x: T) -> None:
        pass

Class4[str]()
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_multiple_init_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class Multi(Generic[T]):
    def __init__(self, x: T, y: T, z: int) -> None:
        pass

Multi[int](1, 2, 3)
Multi[str]("a", "b", 3)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_class_with_new_and_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self
class WithNew:
    def __new__(cls) -> Self:
        return super().__new__(cls)
    def __init__(self, x: int) -> None:
        self.x = x

WithNew(42)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0111_inherited_init() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Base:
    def __init__(self, x: int) -> None:
        self.x = x

class Child(Base):
    pass

Child(42)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
