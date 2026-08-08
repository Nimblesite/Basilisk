//! Pins for PEP 484 forward references over annotation qualifiers
//! ([RESOLV-CANONICAL-BINDING]).
//!
//! A string annotation contains a type expression evaluated lazily
//! (<https://peps.python.org/pep-0484/#forward-references>): the string must be
//! parsed as a type expression and resolved in the module namespace. The tests
//! deliberately avoid the conformance examples' local names and use qualified
//! or renamed imports so matching qualifier text cannot satisfy them.
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
import builtins as runtime_types
import typing as type_contracts

class MineralLedger:
    shared_depth: "type_contracts.ClassVar[runtime_types.int]" = 0
"#,
        "shared_depth",
    );
    assert!(
        is_class_var,
        "the quoted expression resolves qualified aliases to the ClassVar qualifier"
    );
}

#[test]
fn quoted_aliased_classvar_keeps_its_qualifier() {
    let (is_class_var, _) = class_attribute_flags(
        r#"
from builtins import int as depth_number
from typing import ClassVar as shared_slot

class MineralLedger:
    shared_depth: "shared_slot[depth_number]" = 0
"#,
        "shared_depth",
    );
    assert!(
        is_class_var,
        "the quoted alias resolves through module bindings rather than qualifier spelling"
    );
}

#[test]
fn quoted_final_keeps_its_qualifier() {
    let (_, is_final) = class_attribute_flags(
        r#"
from builtins import int as depth_number
from typing import Final as sealed_slot

class MineralLedger:
    survey_revision: "sealed_slot[depth_number]" = 1
"#,
        "survey_revision",
    );
    assert!(
        is_final,
        "PEP 591 Final semantics survive a PEP 484 forward-reference alias"
    );
}

#[test]
fn quoted_shadowed_qualifier_is_not_the_qualifier() {
    let (is_class_var, _) = class_attribute_flags(
        r#"
from builtins import dict as mapping_factory

shared_slot = mapping_factory

class MineralLedger:
    shared_depth: "shared_slot[int]" = 0
"#,
        "shared_depth",
    );
    assert!(
        !is_class_var,
        "the quoted name resolves to the unrelated module binding, not typing.ClassVar"
    );
}
