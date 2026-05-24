//! Implements [CHKARCH-PLUGINS]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-PLUGINS
//! WASM-based plugin host for Basilisk.
//!
//! Will house the sandboxed plugin runtime in Phase 5.

/// Load a WASM plugin by file path.
///
/// Validates that the plugin path is plausible before accepting it.
/// Full WASM sandboxing will be implemented in Phase 5.
///
/// # Errors
///
/// Returns `Err` if the path is absolute and does not exist on disk.
pub fn load_plugin(path: &str) -> Result<(), &'static str> {
    let p = std::path::Path::new(path);
    if p.is_absolute() && !p.exists() {
        return Err("plugin file not found");
    }
    Ok(())
}
