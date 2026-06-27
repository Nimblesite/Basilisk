//! Tests for [generics_syntax_compatibility] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for generics_syntax_compatibility: PEP 695 mixed with traditional `TypeVar`.

use super::common::*;

#[test]
fn pep695_with_traditional_typevar_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

K = TypeVar("K")

class ClassA[V](dict[K, V]):
    ...
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_syntax_compatibility");
    assert!(
        !msgs.is_empty(),
        "PEP 695 class using traditional TypeVar should fire E0042, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn pep695_only_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Container[T]:
    value: T
";
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_syntax_compatibility");
    assert!(msgs.is_empty(), "pure PEP 695 class should not fire E0042");
    Ok(())
}

#[test]
fn traditional_only_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    pass
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_syntax_compatibility");
    assert!(
        msgs.is_empty(),
        "traditional-only generics should not fire E0042"
    );
    Ok(())
}

#[test]
fn pep695_function_with_traditional_typevar_fires() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

def func[U](x: T, y: U) -> None:
    pass
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "generics_syntax_compatibility");
    assert!(
        !msgs.is_empty(),
        "PEP 695 function using traditional TypeVar should fire E0042, got: {msgs:?}"
    );
    Ok(())
}
