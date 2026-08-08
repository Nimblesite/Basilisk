//! Golden target — typing-spec area obligations.
//! [PERMTEST-FAMILY-A] / [PERMTEST-FAMILY-B]
//!
//! One module per chapter of the typing spec. Every case is written in a
//! vocabulary disjoint from `conformance/tests/`, and every quarantined `typing`
//! symbol is reached only under an alias or by attribute access, so a rule that
//! matches on a symbol's *spelling* cannot satisfy these.
//!
//! Golden tests live in `tests/golden/` and are kept apart from unit tests,
//! which live in `tests/unit/`. See `tests/golden/README.md`.

#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    missing_docs,
    clippy::needless_raw_string_hashes,
    clippy::uninlined_format_args,
    dead_code,
    unused_imports
)]

mod common;

#[path = "golden/harness.rs"]
mod harness;

#[path = "golden/aliases_tests.rs"]
mod aliases;

#[path = "golden/async_tests.rs"]
mod r#async;

#[path = "golden/async_more_tests.rs"]
mod async_more;

#[path = "golden/callables_paramspec_tests.rs"]
mod callables_paramspec;

#[path = "golden/callables_paramspec_more_tests.rs"]
mod callables_paramspec_more;

#[path = "golden/contextmanagers_tests.rs"]
mod contextmanagers;

#[path = "golden/contextmanagers_more_tests.rs"]
mod contextmanagers_more;

#[path = "golden/dataclasses_tests.rs"]
mod dataclasses;

#[path = "golden/enums_tests.rs"]
mod enums;

#[path = "golden/generics_bounds_tests.rs"]
mod generics_bounds;

#[path = "golden/generics_bounds_more_tests.rs"]
mod generics_bounds_more;

#[path = "golden/introspection_stdlib_tests.rs"]
mod introspection_stdlib;

#[path = "golden/introspection_stdlib_more_tests.rs"]
mod introspection_stdlib_more;

#[path = "golden/narrowing_tests.rs"]
mod narrowing;

#[path = "golden/narrowing_more_tests.rs"]
mod narrowing_more;

#[path = "golden/overloads_tests.rs"]
mod overloads;

#[path = "golden/overloads_more_tests.rs"]
mod overloads_more;

#[path = "golden/protocols_structural_tests.rs"]
mod protocols_structural;

#[path = "golden/qualifiers_tests.rs"]
mod qualifiers;

#[path = "golden/qualifiers_more_tests.rs"]
mod qualifiers_more;

#[path = "golden/typeddicts_tests.rs"]
mod typeddicts;
