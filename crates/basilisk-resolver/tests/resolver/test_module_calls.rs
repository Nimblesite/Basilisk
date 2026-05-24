//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_module_calls`.

use super::common::resolve_src;

#[test]
fn collects_call_from_module_level_ann_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def add(x: int) -> int:\n    return x\n\nresult: int = add(42)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.calls.is_empty(),
        "AnnAssign call must be collected"
    );
    assert_eq!(resolved.calls[0].callee, "add");
    Ok(())
}

#[test]
fn collects_call_from_module_level_expr_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def side_effect() -> None:\n    pass\n\nside_effect()\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.calls.is_empty(),
        "Expr-stmt call must be collected"
    );
    assert_eq!(resolved.calls[0].callee, "side_effect");
    Ok(())
}
