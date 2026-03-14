#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0037: Invalid `TypedDict` functional-syntax call.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn e0037_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0037")
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn e0037_valid_typeddict_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict
Movie = TypedDict("Movie", {"title": str, "year": int})
"#;
    let msgs = e0037_messages(&run(source)?);
    assert!(
        msgs.is_empty(),
        "valid TypedDict should not fire E0037, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0037_name_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict
Movie = TypedDict("Film", {"title": str})
"#;
    let msgs = e0037_messages(&run(source)?);
    assert!(
        msgs.iter().any(|m| m.contains("does not match")),
        "name mismatch should fire E0037, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn e0037_keyword_only_form_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict
Movie = TypedDict("Movie", title=str, year=int)
"#;
    let msgs = e0037_messages(&run(source)?);
    // Keyword-only form should NOT flag keyword names as unrecognised
    let unrecognised: Vec<_> = msgs
        .iter()
        .filter(|m| m.contains("Unrecognised keyword"))
        .collect();
    assert!(
        unrecognised.is_empty(),
        "keyword-only form should not fire unrecognised keyword E0037, got: {unrecognised:?}"
    );
    Ok(())
}
