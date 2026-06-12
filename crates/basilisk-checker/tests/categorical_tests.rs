//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
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
    dead_code
)]
mod common;
#[path = "checker/deep_coverage_tests.rs"]
mod deep_coverage;
#[path = "checker/mutation_kill_tests.rs"]
mod mutation_kill;
#[path = "checker/redundant_annotation_tests.rs"]
mod redundant_annotation;
#[path = "checker/rules_coverage_tests.rs"]
mod rules_coverage;
#[path = "checker/suppression_tests.rs"]
mod suppression;
#[path = "checker/version_target_tests.rs"]
mod version_target;
