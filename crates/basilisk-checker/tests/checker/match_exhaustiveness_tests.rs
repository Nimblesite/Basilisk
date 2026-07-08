//! Tests for [`match_exhaustiveness`] from [CHKARCH-DIAG-TYPESAFETY] / [TYPEINF-NARROWING-MATCH] / [TYPEINF-EXCEEDS-EXHAUSTIVE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for match_exhaustiveness: Non-exhaustive match statement.

use super::common::*;

#[test]
fn match_without_wildcard_fires() -> Result<(), Box<dyn std::error::Error>> {
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
        codes(&diags).contains(&"match_exhaustiveness"),
        "match without wildcard should fire E0023, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn match_with_wildcard_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"match_exhaustiveness"),
        "match with wildcard should not fire E0023"
    );
    Ok(())
}

#[test]
fn bare_capture_is_irrefutable_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // `case other:` is a bare capture — irrefutable, like `case _:` — so the
    // match is exhaustive and E0023 must not fire (conformance
    // tuples_type_compat.py func7).
    let source = r#"
def check_val(x: int) -> str:
    match x:
        case 1:
            return "one"
        case other:
            return "other"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"match_exhaustiveness"),
        "bare capture pattern is irrefutable and must not fire E0023, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn guarded_capture_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    // A capture *with a guard* is refutable (the guard can fail), so the match
    // is not exhaustive and E0023 must still fire.
    let source = r#"
def check_val(x: int) -> str:
    match x:
        case 1:
            return "one"
        case other if other > 5:
            return "big"
    return ""
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"match_exhaustiveness"),
        "guarded capture is refutable and must still fire E0023, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn structural_sequence_match_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Structural decomposition of an open-ended tuple union: a catch-all is not
    // required for correctness, so E0023 must not fire (conformance
    // tuples_type_compat.py func6).
    let source = r#"
def func6(val: tuple[int] | tuple[str, str] | tuple[int, str, int]) -> None:
    match val:
        case (x,):
            pass
        case (x, y):
            pass
        case (x, y, z):
            pass
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"match_exhaustiveness"),
        "structural sequence-pattern match must not fire E0023, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
