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

#[path = "resolver/test_protocol_01.rs"]
mod test_protocol_01;

#[path = "resolver/test_protocol_02.rs"]
mod test_protocol_02;

#[path = "resolver/test_readonly.rs"]
mod test_readonly;

#[path = "resolver/test_slots.rs"]
mod test_slots;

#[path = "resolver/test_final_violations.rs"]
mod test_final_violations;
