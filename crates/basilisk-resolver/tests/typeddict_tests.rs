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

#[path = "resolver/test_typeddict_calls.rs"]
mod test_typeddict_calls;

#[path = "resolver/test_typeddict_keys_01.rs"]
mod test_typeddict_keys_01;

#[path = "resolver/test_typeddict_keys_02.rs"]
mod test_typeddict_keys_02;

#[path = "resolver/test_namedtuple.rs"]
mod test_namedtuple;

#[path = "resolver/test_unhashable_keys.rs"]
mod test_unhashable_keys;

#[path = "resolver/test_exception_handler.rs"]
mod test_exception_handler;

#[path = "resolver/test_deep_base_chains.rs"]
mod test_deep_base_chains;
