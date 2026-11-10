//! Pins for stub binding tables under version guards
//! ([RESOLV-CANONICAL-BINDING]).
//!
//! A `.pyi` may bind the SAME name differently in mutually exclusive
//! `sys.version_info` branches. For a concrete target, only the selected
//! branch executes (PEP 484 version checks); a binding table flattened over
//! every branch lets the infeasible branch's binding control resolution in
//! and after the selected one — deciding from source layout, not from what
//! the target actually binds ([ASTREBUILD-LAW]).
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use basilisk_stubs::pyi_parser::parse_pyi_source_for_target;
use basilisk_stubs::types::{StubSource, StubTarget, StubTargetPlatform, StubTier};
use basilisk_stubs::StubModule;

fn parse_for_312(source: &str) -> StubModule {
    parse_pyi_source_for_target(
        source,
        std::path::Path::new("test.pyi"),
        "test",
        StubSource::UserStub,
        StubTier::Tier1,
        &StubTarget {
            python_version: (3, 12),
            platform: StubTargetPlatform::All,
        },
    )
    .expect("stub must parse")
}

#[test]
fn infeasible_branch_rebinding_does_not_suppress_overload_recognition() {
    // At 3.12 the guard is false: `overload` is never rebound, so the two
    // decorated defs ARE overloads. A table flattened across branches sees
    // the infeasible `def overload` and stops recognising the decorator.
    let module = parse_for_312(
        r"
import sys
from typing import overload

if sys.version_info < (3, 0):
    def overload(f): ...

@overload
def f(x: int) -> int: ...
@overload
def f(x: str) -> str: ...
",
    );
    let overloads = module.overloads.get("f").map_or(0, Vec::len);
    assert_eq!(
        overloads, 2,
        "for the 3.12 target the infeasible branch never executes; its \
         `def overload` must not shadow `typing.overload`"
    );
}

#[test]
fn feasible_branch_rebinding_still_suppresses_overload_recognition() {
    // The mirror case: at 3.12 the guard is TRUE, the rebinding is real,
    // and the decorated defs are NOT `typing.overload` overloads.
    let module = parse_for_312(
        r"
import sys
from typing import overload

if sys.version_info >= (3, 0):
    def overload(f): ...

@overload
def f(x: int) -> int: ...
@overload
def f(x: str) -> str: ...
",
    );
    assert!(
        !module.overloads.contains_key("f"),
        "the selected branch rebinds `overload`; the decorator is that \
         function, not `typing.overload`"
    );
}
