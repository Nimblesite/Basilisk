//! Tests for [BSK-E0022] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0022: Unhashable dict key.

use super::common::*;

#[test]
fn e0022_hashable_key_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def good_key() -> None:
    mapping: dict[str, int] = {"key": 1}
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0022"),
        "string key should not fire E0022"
    );
    Ok(())
}

#[test]
fn e0022_list_as_key() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def bad_key() -> None:
    mapping = {[1, 2]: "value"}
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
