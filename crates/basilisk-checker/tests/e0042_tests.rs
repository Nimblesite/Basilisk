//! Integration tests for BSK-E0042: PEP 695 mixed with traditional TypeVar.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0042_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0042")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn e0042_pep695_with_traditional_typevar_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

K = TypeVar("K")

class ClassA[V](dict[K, V]):
    ...
"#;
    let msgs = e0042_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "PEP 695 class using traditional TypeVar should fire E0042, got: {:?}",
        msgs
    );
    Ok(())
}

#[test]
fn e0042_pep695_only_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Container[T]:
    value: T
"#;
    let msgs = e0042_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "pure PEP 695 class should not fire E0042"
    );
    Ok(())
}

#[test]
fn e0042_traditional_only_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    pass
"#;
    let msgs = e0042_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "traditional-only generics should not fire E0042"
    );
    Ok(())
}

#[test]
fn e0042_pep695_function_with_traditional_typevar_fires() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

def func[U](x: T, y: U) -> None:
    pass
"#;
    let msgs = e0042_messages(&run(source)?);
    assert!(
        !msgs.is_empty(),
        "PEP 695 function using traditional TypeVar should fire E0042, got: {:?}",
        msgs
    );
    Ok(())
}
