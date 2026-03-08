//! Integration tests for BSK-E0016: Incompatible method override.
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
fn e0016_incompatible_param_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: int) -> str:
        return str(data)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0016"),
        "incompatible param type in @override should fire E0016, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0016_incompatible_return_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: str) -> int:
        return 42
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0016"),
        "incompatible return type in @override should fire E0016, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0016_compatible_override_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import override

class Base:
    def process(self, data: str) -> str:
        return data

class Child(Base):
    @override
    def process(self, data: str) -> str:
        return data.upper()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0016"),
        "compatible override should not fire E0016"
    );
    Ok(())
}
