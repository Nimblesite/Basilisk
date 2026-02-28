//! Language Server Protocol implementation for Basilisk.
//!
//! Will house the LSP server in Phase 2.

/// Return diagnostics for a Python source string (LSP integration).
///
/// Phase 2: currently returns an empty list.  Once implemented, delegates to
/// the checker pipeline and formats results as LSP diagnostic strings.
#[must_use]
pub fn check_source(_source: &str) -> Vec<String> {
    Vec::new()
}
