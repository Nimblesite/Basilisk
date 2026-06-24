//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_mutant_classify_rhs`.

use super::common::resolve_src;

#[test]
fn classify_rhs_empty_list_vs_nonempty() -> Result<(), Box<dyn std::error::Error>> {
    // Use two module vars: one with empty list, one with non-empty.
    let src = concat!("empty: list = []\n", "nonempty: list = [1, 2, 3]\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let empty_var = resolved
        .module_vars
        .iter()
        .find(|v| v.name == "empty")
        .ok_or("empty not found")?;
    let nonempty_var = resolved
        .module_vars
        .iter()
        .find(|v| v.name == "nonempty")
        .ok_or("nonempty not found")?;
    assert_eq!(
        format!("{:?}", empty_var.rhs_kind),
        "EmptyList",
        "empty list must produce EmptyList"
    );
    assert_ne!(
        format!("{:?}", nonempty_var.rhs_kind),
        "EmptyList",
        "non-empty list must NOT produce EmptyList"
    );
    Ok(())
}

#[test]
fn classify_rhs_empty_dict_vs_nonempty() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("empty: dict = {}\n", "nonempty: dict = {'a': 1}\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let empty_var = resolved
        .module_vars
        .iter()
        .find(|v| v.name == "empty")
        .ok_or("empty not found")?;
    let nonempty_var = resolved
        .module_vars
        .iter()
        .find(|v| v.name == "nonempty")
        .ok_or("nonempty not found")?;
    assert_eq!(
        format!("{:?}", empty_var.rhs_kind),
        "EmptyDict",
        "empty dict must produce EmptyDict"
    );
    assert_ne!(
        format!("{:?}", nonempty_var.rhs_kind),
        "EmptyDict",
        "non-empty dict must NOT produce EmptyDict"
    );
    Ok(())
}

#[test]
fn is_wildcard_pattern_bare_capture_is_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x = 1\n",
        "match x:\n",
        "    case y:\n", // bare capture (no guard) — irrefutable, IS a wildcard
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    // A bare capture `case y:` always matches (like `case _:`), so the match is
    // exhaustive and has_wildcard must be true.
    let stmt = resolved.match_stmts.first().ok_or("no match stmt")?;
    assert!(
        stmt.has_wildcard,
        "bare capture pattern `case y:` must be treated as a wildcard"
    );
    Ok(())
}

#[test]
fn is_wildcard_pattern_guarded_capture_is_not_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x = 1\n",
        "match x:\n",
        "    case y if y > 0:\n", // capture WITH guard — refutable, NOT a wildcard
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    // A guard makes the capture refutable, so it is not a wildcard.
    let stmt = resolved.match_stmts.first().ok_or("no match stmt")?;
    assert!(
        !stmt.has_wildcard,
        "guarded capture `case y if ...:` must not be a wildcard"
    );
    Ok(())
}

#[test]
fn is_wildcard_pattern_bare_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x = 1\n",
        "match x:\n",
        "    case 1:\n",
        "        pass\n",
        "    case _:\n", // bare wildcard
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let stmt = resolved.match_stmts.first().ok_or("no match stmt")?;
    assert!(
        stmt.has_wildcard,
        "bare `case _:` must set has_wildcard=true"
    );
    Ok(())
}
