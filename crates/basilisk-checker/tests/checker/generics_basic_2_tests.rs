//! Tests for [generics_basic_2] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for generics_basic_2: Non-`TypeVar` in Generic[...].

use super::common::*;

#[test]
fn concrete_type_in_generic_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generic
class Bad(Generic[int]):
    pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic_2"),
        "concrete type in Generic should fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn typevar_in_generic_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class Good(Generic[T]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_basic_2"),
        "TypeVar in Generic should not fire E0043"
    );
    Ok(())
}

#[test]
fn concrete_type_in_protocol_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol
class Bad(Protocol[int]):
    pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic_2"),
        "concrete type in Protocol should fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn undeclared_typevar_in_base_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Iterable
T = TypeVar("T")
S = TypeVar("S")
class Bad(Iterable[T], Generic[S]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"generics_basic_2"),
        "undeclared TypeVar in base should fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn multiple_typevars_in_generic_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
U = TypeVar("U")
class Good(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_basic_2"),
        "Multiple TypeVars in Generic should not fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn typevar_in_protocol_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T = TypeVar("T")
class Good(Protocol[T]):
    def method(self, x: T) -> T: ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"generics_basic_2"),
        "TypeVar in Protocol should not fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
