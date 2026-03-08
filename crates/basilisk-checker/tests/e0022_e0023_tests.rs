//! Integration tests for BSK-E0022 (unhashable dict key) and BSK-E0023 (non-exhaustive match).
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

// --- E0022: Unhashable dict key ---

#[test]
fn e0022_list_as_dict_key_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def bad() -> None:
    d = {[1, 2]: "value"}
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0022"),
        "list as dict key should fire E0022, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0022_string_key_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def good() -> None:
    d = {"key": "value"}
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0022"),
        "string key should not fire E0022"
    );
    Ok(())
}

// --- E0023: Non-exhaustive match ---

#[test]
fn e0023_match_without_wildcard_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def check_val(x: int) -> None:
    match x:
        case 1:
            pass
        case 2:
            pass
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0023"),
        "match without wildcard should fire E0023, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0023_match_with_wildcard_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def check_val(x: int) -> None:
    match x:
        case 1:
            pass
        case _:
            pass
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0023"),
        "match with wildcard should not fire E0023"
    );
    Ok(())
}
