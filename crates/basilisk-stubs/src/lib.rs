//! Bundled type stubs for Basilisk.
//!
//! Will house typeshed bundles and the auto-stub generation engine in Phase 5.

/// Look up the type annotation string for a built-in symbol.
///
/// Phase 5: currently returns `None` for all names.  Once implemented,
/// returns type information from bundled typeshed stubs.
#[must_use]
pub fn lookup_builtin(name: &str) -> Option<&'static str> {
    let _ = name;
    None
}
