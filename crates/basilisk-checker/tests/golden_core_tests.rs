//! Golden target — core type-checking obligations every conforming checker owes.
//! [PERMTEST-FAMILY-B]
//!
//! Assignability, callability, arity, argument binding, subscript protocols,
//! unpacking, iteration, awaitables, generators, context managers, and returns.
//! These are the obligations that hold for *all* Python, independent of any
//! `typing` construct, and none of them can be decided from source spelling.
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

#[path = "golden/stdlib_assignment_tests.rs"]
mod stdlib_assignment;

#[path = "golden/stdlib_async_context_tests.rs"]
mod stdlib_async_context;

#[path = "golden/stdlib_async_context_more_tests.rs"]
mod stdlib_async_context_more;

#[path = "golden/stdlib_call_argument_tests.rs"]
mod stdlib_call_argument;

#[path = "golden/stdlib_call_arity_tests.rs"]
mod stdlib_call_arity;

#[path = "golden/stdlib_subscript_container_tests.rs"]
mod stdlib_subscript_container;

#[path = "golden/stdlib_subscript_protocol_tests.rs"]
mod stdlib_subscript_protocol;

#[path = "golden/stdlib_unpacking_tests.rs"]
mod stdlib_unpacking;
