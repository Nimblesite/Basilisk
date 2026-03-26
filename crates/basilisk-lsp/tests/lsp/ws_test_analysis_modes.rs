// Tests for LSP: `ws_test_analysis_modes`.

use super::ws_test_common::*;

use std::time::Duration;

use futures_util::StreamExt;

#[tokio::test]
async fn test_ws_whole_module_startup_scan_publishes_diagnostics() -> TestResult<()> {
    // Create a temp workspace with a Python file that has type errors.
    let dir = unique_temp_dir("bsk_ws_startup_scan");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("check_me.py"),
        "def greet(name):\n    return f\"Hello, {name}!\"\n",
    )?;

    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // The startup scan should publish diagnostics for check_me.py without any didOpen.
    let mut found_diag = false;
    for _ in 0..15 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains("\"method\":\"textDocument/publishDiagnostics\"")
            && msg.contains("check_me.py")
        {
            found_diag = true;
            // The file has a missing type annotation — expect at least one diagnostic.
            assert!(
                msg.contains("\"diagnostics\":[{"),
                "expected non-empty diagnostics from startup scan: {msg}"
            );
            break;
        }
    }

    assert!(
        found_diag,
        "startup scan should publish diagnostics for check_me.py without didOpen"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_open_files_only_mode_no_startup_scan() -> TestResult<()> {
    // In openFilesOnly mode, no startup scan should occur.
    let dir = unique_temp_dir("bsk_ws_no_scan");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("closed.py"),
        "def greet(name):\n    return f\"Hello, {name}!\"\n",
    )?;

    let root_uri = format!("file://{}", dir.display());
    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "openFilesOnly").await?;

    // Drain messages for 500ms — should not see any publishDiagnostics for closed.py.
    let mut saw_scan_diag = false;
    for _ in 0..5 {
        let msg = tokio::time::timeout(Duration::from_millis(200), fixture.ws_read.next()).await;
        if let Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) = msg {
            if text.contains("\"method\":\"textDocument/publishDiagnostics\"")
                && text.contains("closed.py")
            {
                saw_scan_diag = true;
                break;
            }
        } else {
            break;
        }
    }

    assert!(
        !saw_scan_diag,
        "openFilesOnly mode should NOT publish diagnostics for closed files at startup"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_whole_module_did_close_keeps_diagnostics() -> TestResult<()> {
    // In wholeModule mode, closing a file should keep diagnostics (re-analyse from disk).
    let dir = unique_temp_dir("bsk_ws_close_keep");
    std::fs::create_dir_all(&dir)?;
    let file_path = dir.join("keep.py");
    std::fs::write(
        &file_path,
        "def greet(name):\n    return f\"Hello, {name}!\"\n",
    )?;

    let file_uri = format!("file://{}", file_path.display());
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // Open the file.
    fixture
        .did_open(
            &file_uri,
            "def greet(name):\n    return f\"Hello, {name}!\"\n",
        )
        .await?;
    // Drain open diagnostics.
    fixture.wait_for_diagnostics().await?;

    // Close the file.
    fixture.did_close(&file_uri).await?;

    // Should receive a publishDiagnostics after close (re-analysis from disk).
    let diag = fixture.wait_for_diagnostics().await;
    assert!(
        diag.is_some(),
        "wholeModule mode should re-publish diagnostics after didClose"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_whole_module_did_close_non_disk_file_returns_empty_diagnostics() -> TestResult<()>
{
    // In wholeModule mode, closing a file that only exists in memory (not on disk)
    // should publish empty diagnostics (file is removed from index).
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?; // default mode = wholeModule

    let uri = "file:///memory_only_close_test.py";
    fixture
        .did_open(uri, "def greet(name):\n    return f\"Hello, {name}!\"\n")
        .await?;
    fixture.wait_for_diagnostics().await?;

    fixture.did_close(uri).await?;

    // File doesn't exist on disk → set_closed() removes it and returns empty diagnostics.
    let diag = fixture.wait_for_diagnostics().await;
    assert!(diag.is_some(), "should publish diagnostics on close");
    let diag_msg = diag.ok_or("expected diagnostics message")?;
    assert!(
        diag_msg.contains("\"diagnostics\":[]"),
        "non-disk file close should produce empty diagnostics in wholeModule mode: {diag_msg}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_open_files_only_did_close_clears_diagnostics() -> TestResult<()> {
    // In openFilesOnly mode, closing a file that EXISTS on disk should still clear
    // its diagnostics — the server does not re-analyse closed files in this mode.
    let dir = unique_temp_dir("bsk_ws_ofo_close");
    std::fs::create_dir_all(&dir)?;
    let file_path = dir.join("ofo_close.py");
    // File has type errors — should produce diagnostics when open.
    std::fs::write(
        &file_path,
        "def greet(name):\n    return f\"Hello, {name}!\"\n",
    )?;

    let file_uri = format!("file://{}", file_path.display());
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "openFilesOnly").await?;

    // Open the file — should produce diagnostics.
    fixture
        .did_open(
            &file_uri,
            "def greet(name):\n    return f\"Hello, {name}!\"\n",
        )
        .await?;
    let open_diag = fixture.wait_for_diagnostics().await;
    assert!(
        open_diag.is_some(),
        "openFilesOnly: should have diagnostics while file is open"
    );

    // Close the file — in openFilesOnly mode the server publishes empty diagnostics.
    fixture.did_close(&file_uri).await?;

    let close_diag = fixture.wait_for_diagnostics().await;
    assert!(
        close_diag.is_some(),
        "should receive publishDiagnostics after didClose"
    );
    let close_msg = close_diag.ok_or("expected diagnostics on close")?;
    assert!(
        close_msg.contains("\"diagnostics\":[]"),
        "openFilesOnly: didClose should clear diagnostics (empty array): {close_msg}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_whole_module_did_close_disk_file_keeps_diagnostics() -> TestResult<()> {
    // In wholeModule mode, closing a file that EXISTS on disk should keep diagnostics
    // (the server re-analyses from disk). This contrasts with openFilesOnly behaviour.
    let dir = unique_temp_dir("bsk_ws_wm_close_disk");
    std::fs::create_dir_all(&dir)?;
    let file_path = dir.join("wm_close_disk.py");
    std::fs::write(
        &file_path,
        "def greet(name):\n    return f\"Hello, {name}!\"\n",
    )?;

    let file_uri = format!("file://{}", file_path.display());
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // Open the file.
    fixture
        .did_open(
            &file_uri,
            "def greet(name):\n    return f\"Hello, {name}!\"\n",
        )
        .await?;
    fixture.wait_for_diagnostics().await?;

    // Close the file — in wholeModule mode the server re-analyses from disk.
    fixture.did_close(&file_uri).await?;

    // Should receive publishDiagnostics with non-empty diagnostics (re-analysed from disk).
    let close_diag = fixture.wait_for_diagnostics().await;
    assert!(
        close_diag.is_some(),
        "wholeModule: should receive publishDiagnostics after didClose"
    );
    let close_msg = close_diag.ok_or("expected diagnostics on close")?;
    assert!(
        !close_msg.contains("\"diagnostics\":[]"),
        "wholeModule: didClose disk file should keep diagnostics (non-empty): {close_msg}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_file_watcher_events_ignored_in_open_files_only() -> TestResult<()> {
    // In openFilesOnly mode, did_change_watched_files events must be ignored entirely.
    // We verify this by sending a file-watcher event and confirming no diagnostics appear.
    let dir = unique_temp_dir("bsk_ws_watcher_ofo");
    std::fs::create_dir_all(&dir)?;
    let file_path = dir.join("watched.py");
    std::fs::write(
        &file_path,
        "def greet(name):\n    return f\"Hello, {name}!\"\n",
    )?;

    let file_uri = format!("file://{}", file_path.display());
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "openFilesOnly").await?;

    // Send a file-watcher changed event for the file (simulating an on-disk save
    // by an external tool while the file is NOT open in the editor).
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": {
                "changes": [{
                    "uri": file_uri,
                    "type": 2  // FileChangeType::Changed
                }]
            }
        }))
        .await?;

    // Drain messages — should NOT receive publishDiagnostics for this file.
    let mut saw_diag = false;
    for _ in 0..5 {
        let msg = tokio::time::timeout(Duration::from_millis(300), fixture.ws_read.next()).await;
        if let Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) = msg {
            if text.contains("\"method\":\"textDocument/publishDiagnostics\"")
                && text.contains("watched.py")
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
        "openFilesOnly: file-watcher events should not trigger diagnostics for closed files"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn test_ws_file_watcher_triggers_reanalysis_in_whole_module() -> TestResult<()> {
    // In wholeModule mode, a file-watcher event for a closed file triggers re-analysis.
    let dir = unique_temp_dir("bsk_ws_watcher_wm");
    std::fs::create_dir_all(&dir)?;
    let file_path = dir.join("wm_watched.py");
    // Start with a clean file.
    std::fs::write(&file_path, "x: int = 1\n")?;

    let file_uri = format!("file://{}", file_path.display());
    let root_uri = format!("file://{}", dir.display());

    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // Drain startup scan messages.
    for _ in 0..5 {
        let msg = tokio::time::timeout(Duration::from_millis(200), fixture.ws_read.next()).await;
        if msg.is_err() {
            break;
        }
    }

    // Now write type errors to the file on disk (external change).
    std::fs::write(
        &file_path,
        "def greet(name):\n    return f\"Hello, {name}!\"\n",
    )?;

    // Send a file-watcher changed event.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "workspace/didChangeWatchedFiles",
            "params": {
                "changes": [{
                    "uri": file_uri,
                    "type": 2  // FileChangeType::Changed
                }]
            }
        }))
        .await?;

    // Should receive publishDiagnostics with errors.
    let diag = fixture.wait_for_diagnostics().await;
    assert!(
        diag.is_some(),
        "wholeModule: file-watcher event should trigger re-analysis and publish diagnostics"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
