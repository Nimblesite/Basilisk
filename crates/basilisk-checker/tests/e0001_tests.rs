//! Integration tests for BSK-E0001: Missing parameter type annotation.
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
fn e0001_missing_param_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def greet(name):
    return name
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0001"),
        "unannotated parameter should fire E0001, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0001_annotated_param_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def greet(name: str) -> str:
    return name
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0001"),
        "annotated parameter should not fire E0001"
    );
    Ok(())
}

#[test]
fn e0001_self_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    def method(self) -> None:
        pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0001"),
        "self parameter should not fire E0001"
    );
    Ok(())
}

#[test]
fn e0001_cls_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Foo:
    @classmethod
    def method(cls) -> None:
        pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0001"),
        "cls parameter should not fire E0001"
    );
    Ok(())
}

#[test]
fn e0001_multiple_unannotated_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def add(a, b):
    return a + b
"#;
    let diags = run(source)?;
    let e0001_count = codes(&diags).iter().filter(|c| **c == "BSK-E0001").count();
    assert!(
        e0001_count >= 2,
        "two unannotated params should fire E0001 at least twice, got {e0001_count}"
    );
    Ok(())
}
