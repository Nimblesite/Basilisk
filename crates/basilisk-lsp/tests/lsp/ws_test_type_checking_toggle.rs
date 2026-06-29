//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
//
// Regression tests for the "Type Checking" toggle (`basilisk.enabled`).
//
// GitHub #65 (sidebar button is a no-op) / #119 (type checking disabled but
// Basilisk diagnostics remain published). The toggle keeps getting reported as
// broken because the LSP — which is authoritative for diagnostics in the
// default mode — ignores `basilisk.enabled` entirely: it publishes diagnostics
// unconditionally and never clears them when the setting flips to `false`.
//
// These tests drive the REAL LSP over the WebSocket transport (no mock, no
// direct `executeCommand("basilisk.toggleFeature")` poke) and assert the
// observable downstream effect a user sees: when type checking is disabled,
// previously-published diagnostics are cleared. Exercises [ANALYSIS-PUBLISH].

use super::ws_test_common::*;

/// A source file that fires at least one diagnostic under the fixture config
/// (which opts into the annotation house rules). Shared by the toggle tests.
const ERRORING_SOURCE: &str = "def greet(name):\n    return f\"Hello, {name}!\"\n";

/// Send `workspace/didChangeConfiguration` with `basilisk.enabled` set.
///
/// Mirrors the exact payload shape the VS Code extension sends
/// (`{ settings: { basilisk: { … } } }` carrying both `analysisMode` and
/// `enabled`, see `buildServerSettings` / `readBasiliskSettings` in
/// `lsp-client.ts`), so the test fails for the real reason and not because of a
/// contrived message shape. `mode` matches the session's analysis mode.
async fn set_type_checking(
    fixture: &mut WsTestFixture,
    enabled: bool,
    mode: &str,
) -> TestResult<()> {
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeConfiguration",
            "params": {
                "settings": {
                    "basilisk": {
                        "analysisMode": mode,
                        "enabled": enabled
                    }
                }
            }
        }))
        .await
}

// Exercises [ANALYSIS-PUBLISH] — disabling type checking (`basilisk.enabled` =
// false) must clear previously-published diagnostics. GitHub #65 / #119.
#[tokio::test]
async fn test_ws_type_checking_disabled_clears_diagnostics() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?; // default mode = wholeModule

    // Open a file with a type error — the server publishes diagnostics for it.
    let uri = "file:///type_checking_toggle.py";
    fixture.did_open(uri, ERRORING_SOURCE).await?;

    let open_diag = fixture.wait_for_diagnostics().await?;
    assert!(
        open_diag.contains(uri) && open_diag.contains("\"diagnostics\":[{"),
        "precondition: server should publish non-empty diagnostics while \
         type checking is enabled: {open_diag}"
    );

    // Flip the Type Checking toggle OFF, exactly as the extension does.
    set_type_checking(&mut fixture, false, "wholeModule").await?;

    // The server must publish EMPTY diagnostics for the open file, clearing the
    // stale ones. Today it ignores `basilisk.enabled` and publishes nothing, so
    // this times out — the bug.
    let cleared = fixture.wait_for_diagnostics().await?;
    assert!(
        cleared.contains(uri) && cleared.contains("\"diagnostics\":[]"),
        "disabling type checking must clear (empty) the file's diagnostics: {cleared}"
    );

    Ok(())
}

// Exercises [ANALYSIS-ENABLED] — while type checking is off, newly-opened files
// must NOT publish diagnostics (the toggle suppresses, not just clears once).
#[tokio::test]
async fn test_ws_type_checking_disabled_suppresses_new_diagnostics() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Turn type checking OFF before opening anything.
    set_type_checking(&mut fixture, false, "wholeModule").await?;

    // Opening an erroring file must produce NO diagnostics while disabled.
    let uri = "file:///suppressed_while_disabled.py";
    fixture.did_open(uri, ERRORING_SOURCE).await?;

    let mut saw_diag = false;
    for _ in 0..5 {
        let msg = tokio::time::timeout(Duration::from_millis(300), fixture.ws_read.next()).await;
        if let Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) = msg {
            if text.contains("\"method\":\"textDocument/publishDiagnostics\"") && text.contains(uri)
            {
                saw_diag = true;
                break;
            }
        } else {
            break;
        }
    }

    assert!(
        !saw_diag,
        "type checking disabled: opening a file must not publish diagnostics"
    );

    Ok(())
}

// Exercises [ANALYSIS-ENABLED] — re-enabling type checking must re-publish the
// diagnostics that were cleared on disable (the toggle is reversible).
#[tokio::test]
async fn test_ws_type_checking_reenabled_republishes_diagnostics() -> TestResult<()> {
    // Use an on-disk file so the wholeModule re-scan on re-enable re-reads it.
    let mut fixture = WsTestFixture::new().await?;
    let file_path = fixture.workspace_root.join("reenable.py");
    std::fs::write(&file_path, ERRORING_SOURCE)?;
    let file_uri = format!("file://{}", file_path.display());

    let _ = fixture.initialize().await?; // wholeModule startup scan

    // Startup scan publishes diagnostics for the on-disk file.
    let startup = fixture.wait_for_diagnostics().await?;
    assert!(
        startup.contains("reenable.py") && startup.contains("\"diagnostics\":[{"),
        "precondition: startup scan should publish diagnostics: {startup}"
    );

    // Disable → diagnostics cleared.
    set_type_checking(&mut fixture, false, "wholeModule").await?;
    let cleared = fixture.wait_for_diagnostics().await?;
    assert!(
        cleared.contains(&file_uri) && cleared.contains("\"diagnostics\":[]"),
        "disable should clear the file's diagnostics: {cleared}"
    );

    // Re-enable → diagnostics come back from the re-scan.
    set_type_checking(&mut fixture, true, "wholeModule").await?;
    let republished = fixture.wait_for_diagnostics().await?;
    assert!(
        republished.contains("reenable.py") && republished.contains("\"diagnostics\":[{"),
        "re-enabling type checking must re-publish diagnostics: {republished}"
    );

    Ok(())
}

// Exercises [ANALYSIS-ENABLED] — a client that initializes with type checking
// already OFF (via `initializationOptions`) must get NO diagnostics from the
// startup scan. Covers the flat top-level `enabled` shape parsed in `initialize`.
#[tokio::test]
async fn test_ws_type_checking_disabled_at_startup_suppresses_scan() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let file_path = fixture.workspace_root.join("startup_disabled.py");
    std::fs::write(&file_path, ERRORING_SOURCE)?;
    let root_uri = format!("file://{}", fixture.workspace_root.to_string_lossy());

    // Initialize with the Type Checking toggle OFF (top-level init option).
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {},
                "trace": "off",
                "initializationOptions": {
                    "analysisMode": "wholeModule",
                    "enabled": false
                }
            }
        }))
        .await?;
    let _ = fixture.recv().await;
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }))
        .await?;

    // The startup scan must publish NOTHING for the on-disk erroring file.
    let mut saw_diag = false;
    for _ in 0..5 {
        let msg = tokio::time::timeout(Duration::from_millis(300), fixture.ws_read.next()).await;
        if let Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) = msg {
            if text.contains("\"method\":\"textDocument/publishDiagnostics\"")
                && text.contains("startup_disabled.py")
            {
                saw_diag = true;
                break;
            }
        } else {
            break;
        }
    }

    assert!(
        !saw_diag,
        "initializing with type checking disabled must suppress the startup scan"
    );

    Ok(())
}

// Exercises [ANALYSIS-ENABLED] — the openFilesOnly re-enable path: re-checking
// and re-publishing the open documents (no workspace scan in this mode).
#[tokio::test]
async fn test_ws_type_checking_reenabled_open_files_only_republishes() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let root_uri = format!("file://{}", fixture.workspace_root.to_string_lossy());
    let _ = initialize_with_root(&mut fixture, &root_uri, "openFilesOnly").await?;

    // Open an in-memory erroring file — diagnostics appear (file is open).
    let uri = "file:///ofo_reenable.py";
    fixture.did_open(uri, ERRORING_SOURCE).await?;
    let open = fixture.wait_for_diagnostics().await?;
    assert!(
        open.contains(uri) && open.contains("\"diagnostics\":[{"),
        "precondition: open file should have diagnostics: {open}"
    );

    // Disable → cleared.
    set_type_checking(&mut fixture, false, "openFilesOnly").await?;
    let cleared = fixture.wait_for_diagnostics().await?;
    assert!(
        cleared.contains(uri) && cleared.contains("\"diagnostics\":[]"),
        "disable should clear the open file's diagnostics: {cleared}"
    );

    // Re-enable → the open file is re-checked and re-published.
    set_type_checking(&mut fixture, true, "openFilesOnly").await?;
    let republished = fixture.wait_for_diagnostics().await?;
    assert!(
        republished.contains(uri) && republished.contains("\"diagnostics\":[{"),
        "re-enabling (openFilesOnly) must re-publish the open file's diagnostics: {republished}"
    );

    Ok(())
}
