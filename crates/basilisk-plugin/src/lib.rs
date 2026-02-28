//! WASM-based plugin host for Basilisk.
//!
//! Will house the sandboxed plugin runtime in Phase 5.

/// Load a WASM plugin by file path.
///
/// Phase 5: currently always returns an error.  Once implemented, loads and
/// sandboxes the WASM module, exposing the Basilisk plugin API.
///
/// # Errors
///
/// Always returns `Err` until Phase 5 is implemented.
pub fn load_plugin(_path: &str) -> Result<(), &'static str> {
    Err("WASM plugin host not yet implemented (Phase 5)")
}
