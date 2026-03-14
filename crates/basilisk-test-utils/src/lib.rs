//! Shared test helpers for Basilisk integration and E2E tests.
//!
//! This crate consolidates common test utilities that are used across
//! multiple test crates (`basilisk-lsp`, `basilisk-cli`, `basilisk-resolver`).

mod diagnostics;
mod source;

#[cfg(feature = "resolver")]
mod resolver_helpers;

#[cfg(feature = "checker")]
mod checker_helpers;

pub use diagnostics::{assert_valid_range, extract_diagnostic};
pub use source::{basilisk_binary, line_col};

/// Convenient result alias for test functions.
pub type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[cfg(feature = "resolver")]
pub use resolver_helpers::resolve_src;

#[cfg(feature = "checker")]
pub use checker_helpers::{assert_diagnostics, Expected};
