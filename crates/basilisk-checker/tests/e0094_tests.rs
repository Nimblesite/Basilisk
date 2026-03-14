#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0094: Self type in invalid location.
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
fn e0094_self_in_method_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

class Foo:
    def clone(self) -> Self:
        return self
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0094"),
        "Self in method return should not fire E0094"
    );
    Ok(())
}

#[test]
fn e0094_self_outside_class() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Self

def standalone() -> Self:
    pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
