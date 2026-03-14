#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0003: Missing variable type (unresolvable inference).
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
fn e0003_empty_list_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items = []\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated empty list should fire E0003, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_empty_dict_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("mapping = {}\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated empty dict should fire E0003, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_none_value_fires() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("result = None\n")?;
    assert!(
        codes(&diags).contains(&"BSK-E0003"),
        "unannotated None should fire E0003, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0003_annotated_empty_list_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items: list[int] = []\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "annotated empty list should not fire E0003"
    );
    Ok(())
}

#[test]
fn e0003_annotated_none_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("result: int | None = None\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "annotated None should not fire E0003"
    );
    Ok(())
}

#[test]
fn e0003_non_empty_list_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items = [1, 2, 3]\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "non-empty list should not fire E0003"
    );
    Ok(())
}

#[test]
fn e0003_string_literal_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("name = \"hello\"\n")?;
    assert!(
        !codes(&diags).contains(&"BSK-E0003"),
        "string literal should not fire E0003"
    );
    Ok(())
}

#[test]
fn e0003_diagnostic_has_help() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("items = []\n")?;
    let e0003 = diags.iter().find(|d| d.code.code == "BSK-E0003");
    assert!(e0003.is_some(), "should fire E0003");
    let Some(diag) = e0003 else {
        return Err("E0003 diagnostic missing after assertion".into());
    };
    assert!(diag.help.is_some(), "E0003 should have help text");
    assert!(diag.note.is_some(), "E0003 should have note text");
    Ok(())
}
