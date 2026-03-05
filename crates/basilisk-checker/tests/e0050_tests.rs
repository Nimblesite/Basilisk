//! E2E tests for BSK-E0050: Invalid `NewType(...)` call.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

/// Helper function to run the checker on source code and return diagnostics.
fn run(source: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let diagnostics = check(&resolved);
    Ok(diagnostics.into_iter().map(|d| d.message).collect())
}

#[test]
fn test_e0050_newtype_name_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType
GoodName = NewType("BadName", int)
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.iter().any(|msg| msg.contains("does not match")),
        "Expected E0050 for NewType name mismatch"
    );
    Ok(())
}

#[test]
fn test_e0050_wrong_arg_count() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NewType
BadNewType = NewType(int)
";
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("takes exactly 2 arguments")),
        "Expected E0050 for wrong argument count"
    );
    Ok(())
}

#[test]
fn test_e0050_base_type_any() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Any
Alias = NewType("Alias", Any)
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("cannot use `Any`")),
        "Expected E0050 for Any base type"
    );
    Ok(())
}

#[test]
fn test_e0050_base_type_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Callable
Alias = NewType("Alias", Callable)
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("cannot use a Protocol class")),
        "Expected E0050 for Callable base type"
    );
    Ok(())
}

#[test]
fn test_e0050_isinstance_with_newtype() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType
UserId = NewType("UserId", int)
u1 = UserId(42)
result = isinstance(u1, UserId)
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("cannot be used as the second argument")),
        "Expected E0050 for isinstance with NewType"
    );
    Ok(())
}

#[test]
fn test_e0050_valid_newtype_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType
UserId = NewType("UserId", int)
u1 = UserId(42)
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.is_empty(),
        "Expected no errors for valid NewType usage"
    );
    Ok(())
}

#[test]
fn test_e0050_base_type_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Literal
Alias = NewType("Alias", Literal["value"])
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("cannot use `Literal`")),
        "Expected E0050 for Literal base type"
    );
    Ok(())
}

#[test]
fn test_e0050_base_type_union() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, Union
Alias = NewType("Alias", Union[int, str])
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("cannot use a union type")),
        "Expected E0050 for Union base type"
    );
    Ok(())
}

#[test]
fn test_e0050_base_type_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType, TypedDict
class Movie(TypedDict):
    title: str
MovieAlias = NewType("MovieAlias", Movie)
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("cannot use a `TypedDict` class")),
        "Expected E0050 for TypedDict base type"
    );
    Ok(())
}

#[test]
fn test_e0050_newtype_subclassing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType
UserId = NewType("UserId", int)
class SpecialUserId(UserId):
    pass
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("cannot subclass")),
        "Expected E0050 for NewType subclassing"
    );
    Ok(())
}

#[test]
fn test_e0050_newtype_generic_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType
UserId = NewType("UserId", int)
x: UserId[int] = UserId(42)
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("cannot be used as a generic type")),
        "Expected E0050 for NewType generic subscript"
    );
    Ok(())
}

#[test]
fn test_e0050_newtype_assigned_to_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType
UserId = NewType("UserId", int)
x: type = UserId
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("not an instance of `type`")),
        "Expected E0050 for NewType assigned to type"
    );
    Ok(())
}

#[test]
fn test_e0050_newtype_call_arg_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType
UserId = NewType("UserId", int)
u1 = UserId("not_a_number")
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics.iter().any(|msg| msg.contains("not compatible")),
        "Expected E0050 for NewType call argument mismatch"
    );
    Ok(())
}

#[test]
fn test_e0050_newtype_literal_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NewType
UserId = NewType("UserId",int)
u1: UserId = 42
"#;
    let diagnostics = run(source)?;
    assert!(
        diagnostics
            .iter()
            .any(|msg| msg.contains("Cannot assign a literal value")),
        "Expected E0050 for literal assignment to NewType"
    );
    Ok(())
}
