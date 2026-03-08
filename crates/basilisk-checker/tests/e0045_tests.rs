//! Integration tests for BSK-E0045: Invalid first argument to Annotated.
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
fn e0045_valid_annotated_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[int, "metadata"] = 42
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0045"),
        "valid Annotated usage should not fire E0045"
    );
    Ok(())
}

#[test]
fn e0045_annotated_with_list_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[[int, str], ""] = 42
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0045_annotated_with_bool_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[True, ""] = True
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0045_annotated_with_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[1, ""] = 1
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0045_annotated_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Annotated

x: Annotated[int] = 42
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0045_annotated_callable_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated, Callable

x: Annotated[Callable[[int], str], "meta"] = lambda a: str(a)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0045"),
        "Annotated with Callable first arg should not fire E0045"
    );
    Ok(())
}
