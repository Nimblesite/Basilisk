#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Shared test helpers for basilisk-resolver integration tests.

// Re-export from the shared test-utils crate.
pub use basilisk_test_utils::resolve_src;
