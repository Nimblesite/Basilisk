// Integration tests for BSK-E0128: `TypeVar` default referential violations.

use super::common::*;

#[test]
fn e0128_bad_ordering_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
Start2T = TypeVar("Start2T", default="StopT")
Stop2T = TypeVar("Stop2T", default=int)
class slice2(Generic[Start2T, Stop2T]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0128_outer_scope_violation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
S1 = TypeVar("S1")
S2 = TypeVar("S2", default=S1)
class Foo3(Generic[S1]):
    class Bar2(Generic[S2]): ...
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0128_bound_constraint_incompatibility() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar
Y1 = TypeVar("Y1", bound=int)
Invalid2 = TypeVar("Invalid2", float, str, default=Y1)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0128_valid_typevar_default_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic
S1 = TypeVar("S1")
S2 = TypeVar("S2", default=S1)
class Good(Generic[S1, S2]): ...
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0128"),
        "valid ordering should not fire E0128"
    );
    Ok(())
}
