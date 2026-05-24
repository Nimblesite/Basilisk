//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_slots`.

use super::common::resolve_src;

#[test]
fn class_manual_slots_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass\n",
        "class MyClass:\n",
        "    __slots__ = ('x',)\n",
        "    x: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "MyClass");
    assert!(cls.is_some());
    assert!(
        cls.is_some_and(|c| c.has_manual_slots),
        "class with __slots__ assignment must have has_manual_slots=true"
    );
    Ok(())
}

#[test]
fn class_ann_assign_slots_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass\n",
        "class MyClass:\n",
        "    __slots__: tuple[str, ...] = ('x',)\n",
        "    x: int\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "MyClass");
    assert!(cls.is_some());
    assert!(
        cls.is_some_and(|c| c.has_manual_slots),
        "class with __slots__ ann_assign must have has_manual_slots=true"
    );
    Ok(())
}

#[test]
fn class_has_manual_slots_flag() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo:\n", "    __slots__ = ('x', 'y')\n",).to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Foo");
    assert!(cls.is_some_and(|c| c.has_manual_slots));
    Ok(())
}
