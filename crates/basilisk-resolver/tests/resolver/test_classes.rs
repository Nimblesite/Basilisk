//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_classes`.

use super::common::resolve_src;

#[test]
fn class_assign_without_annotation_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo:\n    x = 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.classes.len(), 1);
    let cls = resolved
        .classes
        .first()
        .expect("expected at least one class");
    assert!(cls
        .attributes
        .iter()
        .any(|a| a.name == "x" && !a.has_annotation));
    Ok(())
}

#[test]
fn nested_class_methods_collected_in_functions() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Outer:\n",
        "    class Inner:\n",
        "        def inner_method(self: 'Inner') -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.functions.iter().any(|f| f.name == "inner_method"),
        "nested class methods must be collected"
    );
    Ok(())
}
