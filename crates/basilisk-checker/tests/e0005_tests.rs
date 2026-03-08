//! Integration tests for BSK-E0005: Missing class attribute type annotation.
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
fn e0005_unannotated_class_attr_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    value = 42\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0005"),
        "unannotated class attr should fire E0005, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0005_annotated_class_attr_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    value: int = 42\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "annotated class attr should not fire E0005"
    );
    Ok(())
}

#[test]
fn e0005_enum_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from enum import Enum\n\nclass Color(Enum):\n    RED = 1\n    GREEN = 2\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "Enum class should be exempt from E0005"
    );
    Ok(())
}

#[test]
fn e0005_protocol_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import Protocol\n\nclass MyProto(Protocol):\n    name = \"default\"\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "Protocol class should be exempt from E0005"
    );
    Ok(())
}

#[test]
fn e0005_namedtuple_class_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import NamedTuple\n\nclass Point(NamedTuple):\n    x = 0\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0005"),
        "NamedTuple class should be exempt from E0005"
    );
    Ok(())
}

#[test]
fn e0005_multiple_unannotated_attrs() -> Result<(), Box<dyn std::error::Error>> {
    let source = "class Foo:\n    a = 1\n    b = 2\n    c = 3\n";
    let diags = run(source)?;
    let count = diags.iter().filter(|d| d.code.code == "BSK-E0005").count();
    assert_eq!(count, 3, "three unannotated attrs should produce three E0005s");
    Ok(())
}
