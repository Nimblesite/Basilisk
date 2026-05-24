//! Tests for [BSK-E0022]-[BSK-E0023] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0022 (unhashable dict key) and BSK-E0023 (non-exhaustive match).

use super::common::*;

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
