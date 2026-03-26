// Tests for LSP: `lsp_e2e_basics`.

// LSP E2E tests — Initialize, document lifecycle, error handling.

use super::lsp_e2e_common::{LspTestFixture, TestResult};

// ── Initialize + basic document lifecycle ────────────────────────────────────

#[test]
fn test_lsp_initialize() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let response = fixture.initialize()?;

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
    Ok(())
}

#[test]
fn test_lsp_did_open_with_type_errors() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let python_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code)?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("BSK-E0001"));
    assert!(diag.contains("BSK-E0002"));
    assert!(diag.contains("Missing parameter type annotation"));
    assert!(diag.contains("Missing return type annotation"));
    Ok(())
}

#[test]
fn test_lsp_did_open_with_clean_code() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let python_code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code)?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

#[test]
fn test_lsp_did_open_with_syntax_error() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Missing colon after return type.
    let python_code = "def greet(name: str) -> str\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code)?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diag.contains("BSK-PARSE"));
    assert!(diag.contains("Parse error"));
    Ok(())
}

#[test]
fn test_lsp_did_change_updates_diagnostics() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let initial_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", initial_code)?;
    fixture.wait_for_diagnostics()?;

    // Change the document to fully annotated code.
    fixture.send_json(&serde_json::json!({
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
    }))?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics after change")?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

#[test]
fn test_lsp_did_close_clears_diagnostics() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let python_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code)?;
    fixture.wait_for_diagnostics()?;

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": {
            "textDocument": {
                "uri": "file:///test.py"
            }
        }
    }))?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics after close")?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

#[test]
fn test_lsp_hover_on_error_location() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let python_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code)?;
    fixture.wait_for_diagnostics()?;

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": "file:///test.py" },
            "position": { "line": 0, "character": 11 }
        }
    }))?;

    // The hover response is a request reply (has "id"), not a notification.
    // Read messages until we find one with "id":2.
    let mut hover_response = None;
    for _ in 0..10 {
        let Some(msg) = fixture.recv() else { break };
        if msg.contains("\"id\":2") {
            hover_response = Some(msg);
            break;
        }
    }
    let hover = hover_response.ok_or("no hover response")?;

    assert!(hover.contains("\"jsonrpc\":\"2.0\""));
    assert!(hover.contains("BSK-E0001"));
    assert!(hover.contains("Missing parameter type annotation"));
    Ok(())
}

// ── Error handling ───────────────────────────────────────────────────────────

#[test]
fn test_lsp_malformed_json_handling() -> TestResult<()> {
    use std::io::Write as _;
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Send raw malformed JSON (not via send_json which would serialize properly).
    let bad = "{ invalid json }";
    let frame = format!("Content-Length: {}\r\n\r\n{}", bad.len(), bad);
    fixture.stdin.write_all(frame.as_bytes())?;
    fixture.stdin.flush()?;

    // The server may send logMessage notifications (e.g. workspace scan)
    // before the error response; skip notifications and find the error.
    let mut error_response = None;
    for _ in 0..10 {
        let Some(msg) = fixture.recv() else {
            break;
        };
        if msg.contains("\"error\"") {
            error_response = Some(msg);
            break;
        }
    }
    let error_response = error_response.ok_or("no error response")?;

    assert!(error_response.contains("\"error\""));
    assert!(error_response.contains("-32700"));
    assert!(error_response.contains("Parse error"));
    Ok(())
}

#[test]
fn test_lsp_unknown_method_handling() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "textDocument/unknownMethod",
        "params": {}
    }))?;

    // Read messages until we find the error response for id 99.
    let mut error_response = None;
    for _ in 0..10 {
        let Some(msg) = fixture.recv() else { break };
        if msg.contains("\"id\":99") {
            error_response = Some(msg);
            break;
        }
    }
    let resp = error_response.ok_or("no error response")?;

    assert!(resp.contains("\"error\""));
    assert!(resp.contains("-32601"));
    Ok(())
}

// ── Concurrent + large file handling ─────────────────────────────────────────

#[test]
fn test_lsp_concurrent_document_handling() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    fixture.did_open("file:///doc1.py", "def func1(x): pass")?;
    fixture.did_open("file:///doc2.py", "def func2(y): return y")?;

    // Collect two diagnostic notifications (order may vary).
    let mut diags = Vec::new();
    for _ in 0..2 {
        if let Some(msg) = fixture.wait_for_diagnostics() {
            diags.push(msg);
        }
    }
    let combined = diags.join("\n");

    assert!(combined.contains("file:///doc1.py"));
    assert!(combined.contains("file:///doc2.py"));
    assert!(combined.contains("BSK-E0001"));
    Ok(())
}

#[test]
fn test_lsp_large_file_handling() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let mut large_code = String::new();
    for i in 0..50 {
        use std::fmt::Write as _;
        let _ = writeln!(large_code, "def func{i}(x): return x");
    }

    fixture.did_open("file:///large.py", &large_code)?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diag.matches("BSK-E0001").count() >= 50);
    assert!(diag.matches("BSK-E0002").count() >= 50);
    Ok(())
}
