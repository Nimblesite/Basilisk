//! Tests for [`returns_compatibility_2`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for returns_compatibility_2: Return type mismatch (inference-based).

use super::common::*;

#[test]
fn return_list_for_str_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def get_name() -> str:
    return [1, 2, 3]
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"returns_compatibility_2")
            || codes(&diags).contains(&"returns_compatibility"),
        "returning list for str should fire E0013 or E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn correct_return_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def get_name() -> str:
    return "hello"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility_2"),
        "correct return type should not fire E0013"
    );
    Ok(())
}

#[test]
fn return_none_for_int() -> Result<(), Box<dyn std::error::Error>> {
    // None return for int may or may not fire depending on inference depth
    let source = r"
def get_count() -> int:
    return None
";
    let diags = run(source)?;
    // Just ensure no panics; whether this fires depends on inference support
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn empty_list_return_is_valid_for_union_of_lists() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def values() -> list[int] | list[str]:
    return []
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility_2"),
        "empty list literal must use either compatible union return context, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
