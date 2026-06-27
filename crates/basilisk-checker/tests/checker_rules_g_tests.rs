//! Tests for [`typeddicts_extra_items`]-[`overloads_consistency_3`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
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
#[path = "checker/classes_override_3_tests.rs"]
mod classes_override_3;
mod common;
#[path = "checker/dataclasses_inheritance_tests.rs"]
mod dataclasses_inheritance;
#[path = "checker/overloads_consistency_2_tests.rs"]
mod overloads_consistency_2;
#[path = "checker/overloads_consistency_3_tests.rs"]
mod overloads_consistency_3;
#[path = "checker/typeddicts_extra_items_tests.rs"]
mod typeddicts_extra_items;
