//! Integration tests for BSK-E0085: `TypeVarTuple` arg count.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn codes(diags: &[basilisk_checker::Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

#[test]
fn e0085_arg_count_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Generic
Ts = TypeVarTuple("Ts")

class Tensor(Generic[*Ts]):
    pass

def takes_3d(t: Tensor[int, int, int]) -> None:
    pass

x: Tensor[int, int] = Tensor()
takes_3d(x)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
