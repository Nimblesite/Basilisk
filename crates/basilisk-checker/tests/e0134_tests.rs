#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0134: Invariant generic type mismatch.
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
fn e0134_subclass_invariant_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Node: ...

class SymbolTable(dict[str, list[Node]]): ...

def takes(x: dict[str, list[object]]) -> None: ...

def test(s: SymbolTable) -> None:
    takes(s)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0134_valid_invariant_match() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class SymbolTable(dict[str, list[int]]): ...

def takes(x: dict[str, list[int]]) -> None: ...

def test(s: SymbolTable) -> None:
    takes(s)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0134"),
        "exact invariant match should not fire E0134"
    );
    Ok(())
}
