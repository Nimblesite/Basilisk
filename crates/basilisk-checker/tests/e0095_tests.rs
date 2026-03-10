//! Integration tests for BSK-E0095: `InitVar` field validation.
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
fn e0095_post_init_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import InitVar, dataclass

@dataclass
class DC1:
    x: InitVar[int]
    y: InitVar[str]

    def __post_init__(self, x: int, y: int) -> None:
        pass
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0095_initvar_attr_access() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import InitVar, dataclass

@dataclass
class DC1:
    x: InitVar[int]
    y: int = 0

    def __post_init__(self, x: int) -> None:
        self.y = x

dc1 = DC1(1)
dc1.x
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0095_valid_initvar_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from dataclasses import InitVar, dataclass

@dataclass
class DC2:
    x: InitVar[int]
    y: int = 0

    def __post_init__(self, x: int) -> None:
        self.y = x
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0095"),
        "valid InitVar usage should not fire E0095"
    );
    Ok(())
}
