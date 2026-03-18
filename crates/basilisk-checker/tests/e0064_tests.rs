#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0064: Invalid `NamedTuple` call.
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
fn e0064_valid_namedtuple_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import NamedTuple

class Point(NamedTuple):
    x: int
    y: int
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0064"),
        "valid NamedTuple should not fire E0064"
    );
    Ok(())
}

#[test]
fn e0064_namedtuple_functional_form() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
Point = NamedTuple("Point", [("x", int), ("y", int)])
"#;
    let diags = run(source)?;
    // Just exercise - functional form may or may not fire
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0064_unknown_keyword_field_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
Point = NamedTuple("Point", [("x", int), ("y", int)])
p = Point(x=1, z=3)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0064"),
        "unknown keyword field should fire E0064, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0064_keyword_type_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
Point = NamedTuple("Point", [("x", int), ("y", int)])
p = Point(x="hello", y="world")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0064"),
        "keyword type mismatch should fire E0064, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0064_positional_type_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
Point = NamedTuple("Point", [("x", int), ("y", int)])
p = Point("hello", "world")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0064"),
        "positional type mismatch should fire E0064, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0064_valid_keyword_args_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
Point = NamedTuple("Point", [("x", int), ("y", int)])
p = Point(x=1, y=2)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0064"),
        "valid keyword args should not fire E0064, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0064_bytes_for_int_field_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
Record = NamedTuple("Record", [("count", int), ("label", str)])
r = Record(count=b"data", label="ok")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0064"),
        "bytes literal for int field should fire E0064, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0064_float_for_int_field_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NamedTuple
Record = NamedTuple("Record", [("count", int), ("label", str)])
r = Record(count=3.14, label="ok")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0064"),
        "float literal for int field should fire E0064, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
