#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for LSP: `ws_test_basics`.

#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::Message;

// ── Basic handshake ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_initialize() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(response.contains("\"jsonrpc\":\"2.0\""));
    assert!(response.contains("\"id\":1"));
    assert!(response.contains("\"result\""));
    assert!(response.contains("\"basilisk\""));
    assert!(response.contains("\"textDocumentSync\":2"));
    assert!(response.contains("\"hoverProvider\":true"));
    assert!(
        response.contains("\"codeActionProvider\""),
        "should advertise code actions: {response}"
    );
    assert!(response.contains("\"completionProvider\""));
    Ok(())
}

// ── didOpen diagnostics ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_did_open_with_type_errors() -> TestResult<()> {
    let (_fixture, diag) = open_and_diagnose(
        "file:///test.py",
        "def greet(name):\n    return f\"Hello, {name}!\"",
    )
    .await?;

    assert!(diag.contains("BSK-E0001"));
    assert!(diag.contains("BSK-E0002"));
    assert!(diag.contains("Missing parameter type annotation"));
    assert!(diag.contains("Missing return type annotation"));
    Ok(())
}

#[tokio::test]
async fn test_ws_did_open_with_clean_code() -> TestResult<()> {
    let (_fixture, diag) = open_and_diagnose(
        "file:///test.py",
        "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"",
    )
    .await?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

#[tokio::test]
async fn test_ws_did_open_with_syntax_error() -> TestResult<()> {
    let (_fixture, diag) = open_and_diagnose(
        "file:///test.py",
        "def greet(name: str) -> str\n    return f\"Hello, {name}!\"",
    )
    .await?;

    assert!(diag.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diag.contains("BSK-PARSE"));
    assert!(diag.contains("Parse error"));
    Ok(())
}

// ── didChange / didClose ────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_did_change_updates_diagnostics() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let initial_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", initial_code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Change to fully annotated code.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": "file:///test.py",
                    "version": 2
                },
                "contentChanges": [{
                    "text": "def greet(name: str) -> str:\n    return f\"Hello, {name}!\""
                }]
            }
        }))
        .await?;

    let diag = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics after change")?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

#[tokio::test]
async fn test_ws_did_close_clears_diagnostics() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let python_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    fixture.did_close("file:///test.py").await?;

    let diag = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics after close")?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

// ── Hover ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_hover_on_error_location() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let python_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let hover = fixture
        .request(
            2,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///test.py" },
                "position": { "line": 0, "character": 11 }
            }),
        )
        .await?
        .ok_or("no hover response")?;

    assert!(hover.contains("\"jsonrpc\":\"2.0\""));
    assert!(hover.contains("BSK-E0001"));
    assert!(hover.contains("Missing parameter type annotation"));
    Ok(())
}

// ── Error handling ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_malformed_json_handling() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    // Send raw malformed JSON as a text frame.
    fixture
        .ws_write
        .send(Message::Text("{ invalid json }".into()))
        .await?;

    // Skip any leftover notification messages (e.g. window/showMessage from
    // initialization) and look for the parse error response.
    let mut error_response = None;
    for _ in 0..10 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains("-32700") {
            error_response = Some(msg);
            break;
        }
    }
    let error_response = error_response.ok_or("no -32700 parse error response")?;

    assert!(error_response.contains("\"error\""));
    assert!(error_response.contains("-32700"));
    assert!(error_response.contains("Parse error"));
    Ok(())
}

#[tokio::test]
async fn test_ws_unknown_method_handling() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let resp = fixture
        .request(99, "textDocument/unknownMethod", serde_json::json!({}))
        .await?
        .ok_or("no error response")?;

    assert!(resp.contains("\"error\""));
    assert!(resp.contains("-32601"));
    Ok(())
}

// ── Stress / concurrency ───────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_concurrent_document_handling() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    fixture
        .did_open("file:///doc1.py", "def func1(x): pass")
        .await?;
    fixture
        .did_open("file:///doc2.py", "def func2(y): return y")
        .await?;

    let mut diags = Vec::new();
    for _ in 0..2 {
        if let Some(msg) = fixture.wait_for_diagnostics().await {
            diags.push(msg);
        }
    }
    let combined = diags.join("\n");

    assert!(combined.contains("file:///doc1.py"));
    assert!(combined.contains("file:///doc2.py"));
    assert!(combined.contains("BSK-E0001"));
    Ok(())
}

#[tokio::test]
async fn test_ws_large_file_handling() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let mut large_code = String::new();
    for i in 0..50 {
        use std::fmt::Write as _;
        let _ = writeln!(large_code, "def func{i}(x): return x");
    }

    fixture.did_open("file:///large.py", &large_code).await?;

    let diag = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diag.matches("BSK-E0001").count() >= 50);
    assert!(diag.matches("BSK-E0002").count() >= 50);
    Ok(())
}
