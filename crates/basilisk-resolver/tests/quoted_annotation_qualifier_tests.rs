//! Pins for PEP 484 forward references over annotation qualifiers
//! ([RESOLV-CANONICAL-BINDING]).
//!
//! A string annotation contains a type expression evaluated lazily
//! (<https://peps.python.org/pep-0484/#forward-references>): `"ClassVar[int]"`
//! means exactly what `ClassVar[int]` means. Dropping the qualifier because
//! the annotation is quoted decides from the surface form of the source, not
//! its meaning ([ASTREBUILD-LAW]).
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

mod common;

fn class_attribute_flags(src: &str, attribute: &str) -> (bool, bool) {
    let resolved = common::resolve_src(src).expect("source must resolve");
    let class = resolved.classes.first().expect("one class expected");
    let attr = class
        .attributes
        .iter()
        .find(|a| a.name == attribute)
        .unwrap_or_else(|| panic!("attribute `{attribute}` not found"));
    (attr.is_class_var, attr.is_final)
}

#[test]
fn quoted_classvar_keeps_its_qualifier() {
    let (is_class_var, _) = class_attribute_flags(
        r#"
from typing import ClassVar

class C:
    a: "ClassVar[int]" = 0
"#,
        "a",
    );
    assert!(
        is_class_var,
        "`\"ClassVar[int]\"` is a forward reference to `ClassVar[int]`; \
         quoting must not drop the qualifier (PEP 484)"
    );
}

#[test]
fn quoted_aliased_classvar_keeps_its_qualifier() {
    let (is_class_var, _) = class_attribute_flags(
        r#"
from typing import ClassVar as CV

class C:
    a: "CV[int]" = 0
"#,
        "a",
    );
    assert!(
        is_class_var,
        "the quoted alias resolves through the module's bindings like any \
         other use; `CV` IS `ClassVar`"
    );
}

#[test]
fn quoted_final_keeps_its_qualifier() {
    let (_, is_final) = class_attribute_flags(
        r#"
from typing import Final

class C:
    b: "Final[int]" = 1
"#,
        "b",
    );
    assert!(
        is_final,
        "`\"Final[int]\"` is a forward reference to `Final[int]` (PEP 591 \
         via PEP 484 forward references)"
    );
}

#[test]
fn quoted_shadowed_qualifier_is_not_the_qualifier() {
    let (is_class_var, _) = class_attribute_flags(
        r#"
ClassVar = dict

class C:
    a: "ClassVar[int]" = 0
"#,
        "a",
    );
    assert!(
        !is_class_var,
        "the module rebound `ClassVar`; the forward reference resolves to \
         that binding, not to `typing.ClassVar`"
    );
}
