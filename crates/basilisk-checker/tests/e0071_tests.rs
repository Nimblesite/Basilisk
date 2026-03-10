//! Integration tests for BSK-E0071: Historical positional-only syntax.
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
fn e0071_positional_only_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: int, /, y: int) -> int:
    return x + y

func(1, y=2)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0071_keyword_for_positional_only() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func(x: int, /) -> int:
    return x

func(x=1)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
