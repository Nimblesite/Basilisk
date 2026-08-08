//! Pins for enum classification ([RESOLV-CANONICAL-BINDING]).
//!
//! `ClassInfo::is_enum` is documented as "directly or transitively inherits
//! from an `Enum` family class" (`scope/class_types.rs`), and the runtime
//! agrees: a subclass of an `Enum` subclass is an enum
//! (<https://docs.python.org/3/library/enum.html#enum.Enum>). Classifying
//! only direct bases breaks the promise for every module-local enum
//! hierarchy.
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

mod common;

fn is_enum(src: &str, class: &str) -> bool {
    let resolved = common::resolve_src(src).expect("source must resolve");
    resolved
        .classes
        .iter()
        .find(|c| c.name == class)
        .unwrap_or_else(|| panic!("class `{class}` not found"))
        .is_enum
}

#[test]
fn subclass_of_local_enum_class_is_an_enum() {
    let src = r"
from enum import Enum

class Base(Enum):
    pass

class Color(Base):
    RED = 1
";
    assert!(
        is_enum(src, "Color"),
        "`Color` inherits `Enum` through the module-local `Base`; \
         enum-ness is transitive"
    );
}

#[test]
fn deeper_local_enum_chain_is_still_an_enum() {
    let src = r"
from enum import IntEnum

class A(IntEnum):
    pass

class B(A):
    pass

class C(B):
    X = 1
";
    assert!(
        is_enum(src, "C"),
        "three levels of module-local inheritance from `IntEnum` is still \
         an enum"
    );
}

#[test]
fn aliased_direct_enum_base_is_an_enum() {
    let src = r"
from enum import Enum as E

class Color(E):
    RED = 1
";
    assert!(
        is_enum(src, "Color"),
        "an aliased import of `Enum` is `Enum`"
    );
}

#[test]
fn unrelated_local_base_is_not_an_enum() {
    let src = r"
class Base:
    pass

class Child(Base):
    pass
";
    assert!(
        !is_enum(src, "Child"),
        "a plain local hierarchy is not an enum"
    );
}
