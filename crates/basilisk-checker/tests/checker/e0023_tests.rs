// Integration tests for BSK-E0023: Non-exhaustive match statement.

use super::common::*;

#[test]
fn e0023_match_without_wildcard_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def check_val(x: int) -> str:
    match x:
        case 1:
            return "one"
        case 2:
            return "two"
    return ""
"#;
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
    let source = r#"
def check_val(x: int) -> str:
    match x:
        case 1:
            return "one"
        case _:
            return "other"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0023"),
        "match with wildcard should not fire E0023"
    );
    Ok(())
}
