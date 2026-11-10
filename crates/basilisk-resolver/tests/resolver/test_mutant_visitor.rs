//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_mutant_visitor`.

use super::common::resolve_src;

// The definite-assignment collectors moved into the checker's
// `names_unbound` walk ([NARROWPLAN-INTEGRATION] Step 8); the annotated-assign
// and for-target acceptances are pinned end-to-end by
// `annotated_assign_no_diagnostic` / `for_target_no_diagnostic` in
// `basilisk-checker/tests/checker/names_unbound_tests.rs`.

#[test]
fn unhashable_keys_in_assign_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "d = {[1, 2]: 'bad'}\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.module_vars.is_empty(),
        "variable d must be resolved"
    );
    Ok(())
}

#[test]
fn unhashable_keys_in_return_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def bad() -> dict:\n", "    return {[1, 2]: 'bad'}\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "bad")
        .ok_or("bad not found")?;
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key in return must be collected"
    );
    Ok(())
}

#[test]
fn unhashable_keys_in_expr_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def f() -> None:\n", "    {[1, 2]: 'key'}\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "f")
        .ok_or("f not found")?;
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key in expr stmt must be collected"
    );
    Ok(())
}

#[test]
fn collect_module_level_calls_from_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', int, str)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.typevar_calls.is_empty(),
        "TypeVar in plain assignment must be collected"
    );
    Ok(())
}
