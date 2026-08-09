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

// ---------------------------------------------------------------------------
// The CHECKED CLASS resolves through the binding table too
//
// Pins the 2026-08-09 review finding: the callee was resolved correctly, but
// the second argument was accepted only when its `Expr::Name.id` appeared in a
// `HashSet<&str>` built from classes DIRECTLY marked `is_typed_dict`. So a
// TypedDict reached under a second name was missed, a subclass of one was
// missed, and a name rebound away from the TypedDict was flagged anyway.
// ---------------------------------------------------------------------------

#[test]
fn isinstance_against_an_aliased_typeddict_is_flagged() {
    let count = violation_count(
        r#"
from typing import TypedDict as record_shape

class BoreholeRecord(record_shape):
    crystal_depth: int

CoreSample = BoreholeRecord

invalid_probe = isinstance({"crystal_depth": 1}, CoreSample)
"#,
    );
    assert_eq!(
        count, 1,
        "`CoreSample` is one more name for the same TypedDict class object; \
         PEP 589 forbids the runtime check whichever name reaches it"
    );
}

#[test]
fn isinstance_against_a_typeddict_subclass_is_flagged() {
    let count = violation_count(
        r#"
from typing import TypedDict as record_shape

class BoreholeRecord(record_shape):
    crystal_depth: int

class DeepBoreholeRecord(BoreholeRecord):
    survey_year: int

invalid_probe = isinstance({"crystal_depth": 1}, DeepBoreholeRecord)
"#,
    );
    assert_eq!(
        count, 1,
        "a subclass of a TypedDict is a TypedDict, and is equally \
         un-checkable at runtime"
    );
}

#[test]
fn isinstance_against_a_name_rebound_away_from_a_typeddict_is_not_flagged() {
    let count = violation_count(
        r#"
from typing import TypedDict as record_shape

class BoreholeRecord(record_shape):
    crystal_depth: int

class SurveyProbe:
    pass

BoreholeRecord = SurveyProbe

valid_probe = isinstance(object(), BoreholeRecord)
"#,
    );
    assert_eq!(
        count, 0,
        "at the call site the name `BoreholeRecord` is bound to a plain \
         class; `isinstance` against it is legal Python"
    );
}

#[test]
fn isinstance_against_an_ordinary_class_is_not_flagged() {
    let count = violation_count(
        r#"
class SurveyProbe:
    pass

valid_probe = isinstance(object(), SurveyProbe)
"#,
    );
    assert_eq!(count, 0, "a plain class is runtime-checkable");
}
