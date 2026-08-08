//! Pins for the `isinstance`/`issubclass`-on-`TypedDict` collector under
//! [ASTREBUILD-LAW]: both the callee and the checked class must resolve
//! through the binding table, never by comparing identifier spellings.
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

mod common;

fn violation_count(src: &str) -> usize {
    let resolved = common::resolve_src(src).expect("source must resolve");
    resolved.isinstance_typeddict_violations.len()
}

#[test]
fn aliased_isinstance_on_typeddict_is_flagged() {
    let count = violation_count(
        r#"
from typing import TypedDict
from builtins import isinstance as chk

class TD(TypedDict):
    x: int

ok = chk({"x": 1}, TD)
"#,
    );
    assert_eq!(
        count, 1,
        "an aliased import of `isinstance` is `isinstance`; the runtime \
         check against a TypedDict must be flagged regardless of spelling"
    );
}

#[test]
fn shadowed_isinstance_is_not_flagged() {
    let count = violation_count(
        r#"
from typing import TypedDict

class TD(TypedDict):
    x: int

def isinstance(a, b):
    return True

ok = isinstance({"x": 1}, TD)
"#,
    );
    assert_eq!(
        count, 0,
        "a module-level `def isinstance` shadows the builtin; the call is \
         not a runtime type check and must not be flagged"
    );
}

#[test]
fn rebound_typeddict_name_is_not_flagged() {
    let count = violation_count(
        r#"
from typing import TypedDict

class TD(TypedDict):
    x: int

from other_mod import TD

ok = isinstance({"x": 1}, TD)
"#,
    );
    assert_eq!(
        count, 0,
        "after the import rebinds `TD`, the name no longer refers to the \
         TypedDict class; flagging it decides from spelling, not bindings"
    );
}
