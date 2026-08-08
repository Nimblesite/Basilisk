//! Pins for the `isinstance`/`issubclass`-on-`TypedDict` collector under
//! [ASTREBUILD-LAW]: both the callee and the checked class must resolve
//! through the binding table, never by comparing identifier spellings.
//! [PEP 589](https://peps.python.org/pep-0589/#using-typeddict-types) forbids
//! runtime instance and subclass checks because TypedDict classes are not
//! runtime-checkable. Fixtures use renamed imports and unrelated local names.
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
from typing import TypedDict as record_shape
from builtins import int as depth_number
from builtins import isinstance as runtime_probe

class BoreholeRecord(record_shape):
    crystal_depth: depth_number

invalid_probe = runtime_probe({"crystal_depth": 1}, BoreholeRecord)
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
from typing import TypedDict as record_shape
from builtins import int as depth_number
from builtins import isinstance as runtime_probe

class BoreholeRecord(record_shape):
    crystal_depth: depth_number

def runtime_probe(candidate, category):
    return True

ordinary_result = runtime_probe({"crystal_depth": 1}, BoreholeRecord)
"#,
    );
    assert_eq!(
        count, 0,
        "the later user function replaces the imported builtin alias and must not be treated as a runtime type check"
    );
}

#[test]
fn rebound_typeddict_name_is_not_flagged() {
    let count = violation_count(
        r#"
from typing import TypedDict as record_shape
from builtins import int as depth_number
from builtins import isinstance as runtime_probe

class BoreholeRecord(record_shape):
    crystal_depth: depth_number

from unavailable_geology import BoreholeRecord

ordinary_result = runtime_probe({"crystal_depth": 1}, BoreholeRecord)
"#,
    );
    assert_eq!(
        count, 0,
        "after rebinding, `BoreholeRecord` no longer denotes the TypedDict class"
    );
}
