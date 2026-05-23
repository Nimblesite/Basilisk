//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    missing_docs
)]

mod common;

#[path = "resolver/test_annotated.rs"]
mod test_annotated;

#[path = "resolver/test_annotations.rs"]
mod test_annotations;

#[path = "resolver/test_assert_type.rs"]
mod test_assert_type;
