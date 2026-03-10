//! Integration tests for BSK-E0149: PEP 695 type parameter scoping.
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
fn e0149_type_param_scoping_exercise() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Outer[T]:
    class Inner[T]:
        pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0149_valid_distinct_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Outer[T]:
    class Inner[U]:
        pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0149"),
        "distinct type params should not fire E0149"
    );
    Ok(())
}
