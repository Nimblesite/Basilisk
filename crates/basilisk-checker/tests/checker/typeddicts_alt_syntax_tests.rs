//! Tests for [`typeddicts_alt_syntax`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
// Integration tests for typeddicts_alt_syntax: Invalid `TypedDict` functional-syntax call.

use super::common::*;

#[test]
fn valid_typeddict_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict
Movie = TypedDict("Movie", {"title": str, "year": int})
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "typeddicts_alt_syntax");
    assert!(
        msgs.is_empty(),
        "valid TypedDict should not fire E0037, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn name_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict
Movie = TypedDict("Film", {"title": str})
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "typeddicts_alt_syntax");
    assert!(
        msgs.iter().any(|m| m.contains("does not match")),
        "name mismatch should fire E0037, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn keyword_only_form_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypedDict
Movie = TypedDict("Movie", title=str, year=int)
"#;
    let diags = run(source)?;

    let msgs = messages_for(&diags, "typeddicts_alt_syntax");
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
