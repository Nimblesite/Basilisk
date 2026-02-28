//! Integration tests for basilisk-plugin.

#[test]
#[ignore = "Phase 5 not yet implemented — WASM plugin host is a stub"]
fn plugin_host_loads_wasm_plugin() {
    // Phase 5: the plugin host must successfully load a sandboxed WASM plugin.
    // Currently always returns an error (placeholder).
    let result = basilisk_plugin::load_plugin("test_plugin.wasm");
    assert!(
        result.is_ok(),
        "WASM plugin must load successfully — Phase 5 plugin host not yet implemented"
    );
}

#[test]
fn plugin_host_rejects_missing_file() {
    // Loading a non-existent plugin path must return an error.
    let result = basilisk_plugin::load_plugin("/nonexistent/plugin.wasm");
    assert!(result.is_err(), "missing plugin file must return an error");
}
