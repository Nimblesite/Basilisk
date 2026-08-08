//! Permutation suite A — structural typing, callables, generics.
//! [PERMTEST-PLAN]. See docs/plans/CHECKER-PYTHON-PERMUTATION-PLAN.md#PERMTEST-PLAN
//!
//! Every case is authored outside the conformance suite's vocabulary
//! ([PERMTEST-VOCABULARY]) and judged by the two oracles in `permutation/harness.rs`:
//! invariance under semantics-preserving respelling, and a directed
//! reject/accept pair taken from the typing spec.
//!
//! **Failures here are the deliverable.** A red test in this suite is a real
//! statement about what Basilisk cannot yet analyse. Per
//! [CHKARCH-TEXT-MATCHED-LOGIC] it is disposed of by deleting the offending
//! logic, never by weakening the assertion.
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

#[path = "permutation/harness.rs"]
mod harness;

#[path = "permutation/protocols_structural_tests.rs"]
mod protocols_structural;
