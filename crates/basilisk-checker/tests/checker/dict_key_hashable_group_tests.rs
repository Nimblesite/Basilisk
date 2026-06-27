//! Tests for [dict_key_hashable]-[match_exhaustiveness] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for dict_key_hashable (unhashable dict key) and match_exhaustiveness (non-exhaustive match).

use super::common::*;

// --- Unhashable dict key ---

#[test]
fn list_as_dict_key_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def bad() -> None:
    d = {[1, 2]: "value"}
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"dict_key_hashable"),
        "list as dict key should fire E0022, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn string_key_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def good() -> None:
    d = {"key": "value"}
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"dict_key_hashable"),
        "string key should not fire E0022"
    );
    Ok(())
}

// --- Non-exhaustive match ---

#[test]
fn match_without_wildcard_fires() -> Result<(), Box<dyn std::error::Error>> {
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
        codes(&diags).contains(&"match_exhaustiveness"),
        "match without wildcard should fire E0023, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn match_with_wildcard_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"match_exhaustiveness"),
        "match with wildcard should not fire E0023"
    );
    Ok(())
}
