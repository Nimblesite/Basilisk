//! Pins for enum classification ([RESOLV-CANONICAL-BINDING]).
//!
//! `ClassInfo::is_enum` is documented as "directly or transitively inherits
//! from an `Enum` family class" (`scope/class_types.rs`), and the runtime
//! agrees: a subclass of an `Enum` subclass is an enum
//! (<https://docs.python.org/3/library/enum.html#enum.Enum>). Classifying
//! only direct bases breaks the promise for every module-local enum
//! hierarchy. This is the class model introduced by
//! [PEP 435](https://peps.python.org/pep-0435/); fixtures deliberately rename
//! the imported enum roots and every user-defined symbol.
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
from enum import Enum as category_root

class MineralFamily(category_root):
    pass

class IgneousGrade(MineralFamily):
    BASALT = 1
";
    assert!(
        is_enum(src, "IgneousGrade"),
        "`IgneousGrade` inherits the resolved enum root through `MineralFamily`"
    );
}

#[test]
fn deeper_local_enum_chain_is_still_an_enum() {
    let src = r"
import enum as category_tools

class SurveyCode(category_tools.IntEnum):
    pass

class RegionalCode(SurveyCode):
    pass

class BoreholeCode(RegionalCode):
    DEEP_SAMPLE = 1
";
    assert!(
        is_enum(src, "BoreholeCode"),
        "three levels of module-local inheritance from the qualified enum root remain an enum"
    );
}

#[test]
fn aliased_direct_enum_base_is_an_enum() {
    let src = r"
from enum import Enum as category_root

class MineralGrade(category_root):
    GRANITE = 1
";
    assert!(
        is_enum(src, "MineralGrade"),
        "the renamed import resolves to the enum root"
    );
}

#[test]
fn unrelated_local_base_is_not_an_enum() {
    let src = r"
class GeologicalRecord:
    pass

class DerivedRecord(GeologicalRecord):
    pass
";
    assert!(
        !is_enum(src, "DerivedRecord"),
        "a plain local hierarchy is not an enum"
    );
}

// ---------------------------------------------------------------------------
// Propagation follows the RESOLVED base, not the rendered one
//
// Pins the 2026-08-09 review finding against `visitor/mod.rs`: enum-ness was
// closed over a `HashSet<String>` of class NAMES compared against
// `ClassInfo::bases`, a rendering. An assignment alias named no class in that
// set, and two classes spelled the same were one entry in it.
// ---------------------------------------------------------------------------

#[test]
fn enum_base_reached_through_an_assignment_alias_is_an_enum() {
    let src = r"
from enum import Enum as category_root

class MineralFamily(category_root):
    pass

StoneFamily = MineralFamily

class IgneousGrade(StoneFamily):
    BASALT = 1
";
    assert!(
        is_enum(src, "IgneousGrade"),
        "`StoneFamily` is one more name for `MineralFamily`; binding a second \
         name to an enum class does not make its subclasses stop being enums"
    );
}

#[test]
fn a_local_class_sharing_an_enums_name_does_not_make_a_subclass_an_enum() {
    let src = r"
from enum import Enum as category_root

class MineralFamily(category_root):
    pass

class Registry:
    pass

MineralFamily = Registry

class SurveyRecord(MineralFamily):
    pass
";
    assert!(
        !is_enum(src, "SurveyRecord"),
        "at `SurveyRecord`'s base list the name `MineralFamily` is bound to \
         `Registry`; matching the SPELLING of the earlier enum attaches an \
         inheritance edge the program does not have"
    );
}

#[test]
fn an_imported_base_does_not_make_a_class_an_enum() {
    let src = r"
from quarry import MineralFamily

class IgneousGrade(MineralFamily):
    BASALT = 1
";
    assert!(
        !is_enum(src, "IgneousGrade"),
        "another module's class is unknown here — unknown is not `Enum`"
    );
}
