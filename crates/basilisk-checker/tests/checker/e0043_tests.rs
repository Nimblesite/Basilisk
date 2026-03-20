// Integration tests for BSK-E0043: Non-`TypeVar` in Generic[...].

use super::common::*;

#[test]
fn e0043_concrete_type_in_generic_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Generic
class Bad(Generic[int]):
    pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0043"),
        "concrete type in Generic should fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0043_typevar_in_generic_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
class Good(Generic[T]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0043"),
        "TypeVar in Generic should not fire E0043"
    );
    Ok(())
}

#[test]
fn e0043_concrete_type_in_protocol_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol
class Bad(Protocol[int]):
    pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0043"),
        "concrete type in Protocol should fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0043_undeclared_typevar_in_base_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic, Iterable
T = TypeVar("T")
S = TypeVar("S")
class Bad(Iterable[T], Generic[S]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0043"),
        "undeclared TypeVar in base should fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0043_multiple_typevars_in_generic_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
T = TypeVar("T")
U = TypeVar("U")
class Good(Generic[T, U]):
    pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0043"),
        "Multiple TypeVars in Generic should not fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0043_typevar_in_protocol_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Protocol
T = TypeVar("T")
class Good(Protocol[T]):
    def method(self, x: T) -> T: ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0043"),
        "TypeVar in Protocol should not fire E0043, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
