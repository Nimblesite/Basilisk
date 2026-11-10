//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_conditional_assigns`.

use super::common::resolve_src;

#[test]
fn elif_else_functions_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "if True:\n",
        "    def foo() -> None:\n",
        "        pass\n",
        "elif False:\n",
        "    def bar() -> None:\n",
        "        pass\n",
        "else:\n",
        "    def baz() -> None:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"foo"));
    assert!(names.contains(&"bar"));
    assert!(names.contains(&"baz"));
    Ok(())
}

// The definite-assignment ("unconditional") analysis moved into the checker's
// `names_unbound` walk ([NARROWPLAN-INTEGRATION] Step 8); its if/else merge is
// pinned end-to-end by `if_else_both_assign_no_diagnostic` in
// `basilisk-checker/tests/checker/names_unbound_tests.rs`.
