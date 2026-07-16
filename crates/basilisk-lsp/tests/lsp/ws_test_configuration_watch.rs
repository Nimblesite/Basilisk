//! Tests for [LSPARCH-CONFIG]. See
//! docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CONFIG.
//!
//! The LSP itself watches the active configuration source (the root's
//! `pyproject.toml`) and reactively pushes updates — republished diagnostics
//! plus `basilisk/configurationChanged` — for EVERY way the configuration can
//! change, including external disk edits that arrive through no LSP channel
//! at all. Clients without file-watcher support (Zed advertises none —
//! docs/specs/ZED-SPEC.md) get identical behaviour to VS Code.

use super::ws_test_common::*;

/// [LSPARCH-CONFIG]: rewriting `pyproject.toml` on disk — with NO
/// `workspace/didChangeWatchedFiles`, no open config buffer, no LSP message
/// of any kind — must be detected by the server's own watcher, which then
/// rechecks the workspace, republishes the open file's diagnostics at the new
/// severity, and emits `basilisk/configurationChanged` so UIs refresh.
#[tokio::test]
async fn disk_config_edit_without_client_watcher_republishes_and_notifies() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let uri = format!(
        "file://{}/watched_config_app.py",
        fixture.workspace_root.to_string_lossy()
    );
    fixture
        .did_open(&uri, "def handler(value):\n    return value\n")
        .await?;
    let raw = fixture.wait_for_diagnostics().await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;
    let diag = extract_diagnostic(&json, "BSK-0001")
        .ok_or("BSK-0001 should fire before the config change")?;
    assert_eq!(
        diag["severity"].as_u64(),
        Some(1),
        "BSK-0001 must start at error severity: {diag}"
    );

    // External edit: BSK-0001 drops to "warning" on disk. Deliberately sent
    // through no LSP channel — the server must notice on its own.
    std::fs::write(
        fixture.workspace_root.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"BSK-0001\" = \"warning\"\n\"BSK-0002\" = \"error\"\n",
    )?;

    let mut saw_configuration_changed = false;
    let mut saw_warning_severity = false;
    for _ in 0..20 {
        if saw_configuration_changed && saw_warning_severity {
            break;
        }
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains("\"method\":\"basilisk/configurationChanged\"") {
            saw_configuration_changed = true;
        }
        if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") {
            let parsed: serde_json::Value = serde_json::from_str(&msg)?;
            if parsed["params"]["uri"].as_str() == Some(uri.as_str()) {
                if let Some(updated) = extract_diagnostic(&parsed, "BSK-0001") {
                    saw_warning_severity = updated["severity"].as_u64() == Some(2);
                }
            }
        }
    }

    assert!(
        saw_configuration_changed,
        "server never sent basilisk/configurationChanged after an external \
         disk edit — the configuration is not watched server-side"
    );
    assert!(
        saw_warning_severity,
        "BSK-0001 was never republished at warning severity after the \
         external disk edit — diagnostics went stale"
    );
    Ok(())
}
