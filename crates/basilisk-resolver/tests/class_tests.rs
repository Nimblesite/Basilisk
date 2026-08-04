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

#[path = "resolver/test_classes.rs"]
mod test_classes;

#[path = "resolver/test_recursive_bases.rs"]
mod test_recursive_bases;

#[path = "resolver/test_class_properties.rs"]
mod test_class_properties;

#[path = "resolver/test_dataclass.rs"]
mod test_dataclass;

#[path = "resolver/test_enum_class.rs"]
mod test_enum_class;

#[path = "resolver/test_enum_violations.rs"]
mod test_enum_violations;
