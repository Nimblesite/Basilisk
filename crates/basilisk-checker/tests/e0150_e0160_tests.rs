//! Tests for [typeddicts_extra_items]-[overloads_consistency_3] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
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
#[path = "checker/e0156_tests.rs"]
mod e0156;
#[path = "checker/e0157_tests.rs"]
mod e0157;
#[path = "checker/e0158_tests.rs"]
mod e0158;
#[path = "checker/e0159_tests.rs"]
mod e0159;
#[path = "checker/e0160_tests.rs"]
mod e0160;
