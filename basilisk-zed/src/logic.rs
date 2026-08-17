//! Pure logic for the Zed extension: the approved statement, and the manifest
//! assertions that keep this extension from advertising anything else.
//!
//! **Zero `zed_extension_api` imports.** Everything here takes and returns only
//! `String`/`&str`, so the module compiles and tests on any native target — no
//! WASM host required.

/// The statement, generated from the messaging spec's [WITHDRAWAL-INERT-TEXT]
/// fence by `scripts/gen_withdrawal_copy.py` and drift-gated in CI. Included as
/// bytes rather than a source literal so this crate cannot print its own
/// version of it.
pub const NOTICE: &str = include_str!("withdrawal_notice.txt");

/// The panel heading Zed shows above the statement.
pub const LABEL: &str = "Basilisk is unlisted";

/// The `(panel title, body)` pair `/basilisk` renders. Implements
/// [WITHDRAWAL-SURFACES].
pub fn notice_output() -> (String, String) {
    (LABEL.to_owned(), NOTICE.to_owned())
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod logic_tests;
