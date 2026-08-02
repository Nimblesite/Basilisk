//! Tests for [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
// Coverage-boost tests for the text-document lifecycle: `didChange`
// (incremental edits), `didSave`, and `didClose`. Covers the `did_change`,
// `did_save`, and `did_close` handlers in `document.rs` that the basic
// `ws_test_basics` module only partially reaches.

use super::ws_test_common::*;

/// `didChange` with a full-document replacement republishes diagnostics for
/// the new content. Open clean code, edit it to introduce a redundant
/// annotation, and assert the BSK-0050 diagnostic fires after the change.
#[tokio::test]
async fn test_ws_did_change_republishes_diagnostics() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = "file:///ws_didchange.py";
    fixture.did_open(uri, "x = 1\n").await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Replace the whole document with one that fires BSK-0050.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": "x: int = 1\n" }]
            }
        }))
        .await?;

    // Wait for the republished diagnostics to carry BSK-0050.
    let mut settled = None;
    for _ in 0..20 {
        let Some(text) = fixture.recv().await else {
            break;
        };
        if text.contains("publishDiagnostics") && text.contains("BSK-0050") {
            settled = Some(text);
            break;
        }
    }
    let diag = settled.ok_or("didChange did not republish BSK-0050 diagnostics")?;
    assert!(
        diag.contains("BSK-0050"),
        "republished diagnostics should include BSK-0050: {diag}"
    );

    Ok(())
}

/// `didChange` with a RANGE (incremental) edit splices only the changed
/// region. We delete a character range and assert the document still
/// republishes diagnostics — covering the `start <= end` apply branch in
/// `apply_content_changes`.
#[tokio::test]
async fn test_ws_did_change_incremental_range_edit() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = "file:///ws_didchange_inc.py";
    // `x: int = 42` fires BSK-0050; we'll edit it back to clean `x = 42`.
    fixture.did_open(uri, "x: int = 42\n").await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Incremental edit: replace the range covering `: int` (chars 1..6 on line 0)
    // with the empty string, yielding `x = 42`.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{
                    "range": {
                        "start": { "line": 0, "character": 1 },
                        "end": { "line": 0, "character": 6 }
                    },
                    "text": ""
                }]
            }
        }))
        .await?;

    // Wait for a diagnostics publish that NO LONGER carries BSK-0050.
    let mut cleared = false;
    for _ in 0..20 {
        let Some(text) = fixture.recv().await else {
            break;
        };
        if text.contains("publishDiagnostics") && text.contains(uri) && !text.contains("BSK-0050") {
            cleared = true;
            break;
        }
    }
    assert!(
        cleared,
        "incremental edit should clear the BSK-0050 diagnostic"
    );

    Ok(())
}

/// `didSave` re-runs the pipeline on the cached text and republishes.
#[tokio::test]
async fn test_ws_did_save_republishes() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = "file:///ws_didsave.py";
    fixture.did_open(uri, "x: int = 1\n").await?;
    let _ = fixture.wait_for_diagnostics().await;

    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": { "textDocument": { "uri": uri } }
        }))
        .await?;

    // didSave republishes diagnostics — drain until we see one for this uri.
    let mut saw = false;
    for _ in 0..20 {
        let Some(text) = fixture.recv().await else {
            break;
        };
        if text.contains("publishDiagnostics") && text.contains(uri) {
            saw = true;
            break;
        }
    }
    assert!(
        saw,
        "didSave should republish diagnostics for the saved file"
    );

    Ok(())
}

/// `didClose` clears diagnostics for the closed document (publishes an empty
/// diagnostics array). Covers the `did_close` clear path.
#[tokio::test]
async fn test_ws_did_close_clears_diagnostics() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    let uri = "file:///ws_didclose.py";
    fixture.did_open(uri, "x: int = 1\n").await?;
    let _ = fixture.wait_for_diagnostics().await;

    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": uri } }
        }))
        .await?;

    // Wait for an empty (or BSK-0050-free) diagnostics publish for this uri.
    let mut cleared = false;
    for _ in 0..20 {
        let Some(text) = fixture.recv().await else {
            break;
        };
        if text.contains("publishDiagnostics") && text.contains(uri) && !text.contains("BSK-0050") {
            cleared = true;
            break;
        }
    }
    assert!(
        cleared,
        "didClose should clear diagnostics for the closed document"
    );

    Ok(())
}
