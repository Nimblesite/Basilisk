//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Shared helpers for Basilisk CLI end-to-end tests.
//!
//! Every test uses a real `.py` fixture file and asserts the exact set of
//! diagnostics produced: error code, symbol name, byte span, line, column,
//! and message. No hand-wavy count assertions — if a diagnostic appears at
//! the wrong location or with the wrong message, the test fails.
//!
//! Pipeline under test: `parse_file` → resolve → check

use std::path::Path;

use basilisk_checker::{check, Diagnostic};
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;

// Re-export shared helpers from the test-utils crate — used by sibling test modules.
#[expect(
    unused_imports,
    reason = "re-exported for sibling test files via `use common::assert_diagnostics`"
)]
pub use basilisk_test_utils::assert_diagnostics;
#[expect(
    unused_imports,
    reason = "re-exported for sibling test files via `use common::Expected`"
)]
pub use basilisk_test_utils::Expected;

pub fn fixture(rel: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

pub fn run(rel: &str) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
    let path = fixture(rel);
    let parsed = parse_file(&path)?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}
