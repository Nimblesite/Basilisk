//! Integration tests for BSK-E0062: NoReturn/Never function can fall through.
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
fn e0062_noreturn_with_fallthrough_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NoReturn

def bad(x: int) -> NoReturn:
    if x != 0:
        raise RuntimeError("error")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0062"),
        "NoReturn with fallthrough should fire E0062, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0062_noreturn_always_raises_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import NoReturn

def stop() -> NoReturn:
    raise RuntimeError("no way")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0062"),
        "NoReturn that always raises should not fire E0062"
    );
    Ok(())
}

#[test]
fn e0062_normal_return_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def normal() -> int:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0062"),
        "normal return type should not fire E0062"
    );
    Ok(())
}
