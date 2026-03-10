//! Integration tests for BSK-E0122: Callable call-site arity violations.
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
fn e0122_correct_callable_arity() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def invoke(cb: Callable[[int, str], bool]) -> bool:
    return cb(1, "hello")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0122"),
        "correct arity should not fire E0122"
    );
    Ok(())
}

#[test]
fn e0122_wrong_callable_arity() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def invoke(cb: Callable[[int, str], bool]) -> bool:
    return cb(1)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0122_keyword_arg_on_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def invoke(cb: Callable[[int], bool]) -> bool:
    return cb(x=1)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
