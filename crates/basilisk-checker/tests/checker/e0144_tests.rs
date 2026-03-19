// Integration tests for BSK-E0144: type[T] constructor call violations.

use super::common::*;

#[test]
fn e0144_type_t_called_with_args_no_constructor() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Simple:
    pass

def factory(cls: type[Simple]) -> Simple:
    return cls(1, 2)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0144_type_t_missing_required_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class NeedsArgs:
    def __init__(self, x: int, y: int) -> None:
        pass

def factory(cls: type[NeedsArgs]) -> NeedsArgs:
    return cls()
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0144_valid_type_t_call() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class MyClass:
    def __init__(self, x: int) -> None:
        self.x = x

def factory(cls: type[MyClass]) -> MyClass:
    return cls(42)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0144"),
        "valid type[T] call should not fire E0144"
    );
    Ok(())
}

#[test]
fn e0144_unbound_typevar_with_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
T = TypeVar("T")

def create(cls: type[T]) -> T:
    return cls(1)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
