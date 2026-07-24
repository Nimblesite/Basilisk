//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
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

    // `name` has no default to infer from (BSK-0001) and the returned method
    // call is not inferable (BSK-0002) — an f-string return would infer
    // `-> str` and silence BSK-0002 ([TYPEINF-FUNC-RETURN]).
    let python_code = "def greet(name):\n    return name.upper()";
    fixture.did_open("file:///test.py", python_code)?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("BSK-0001"));
    assert!(diag.contains("BSK-0002"));
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

    // Clean code settles to an empty diagnostics publish; wait for that exact
    // publish so an initial-then-settled double publish cannot race the assert.
    let diag = fixture
        .wait_for_diagnostics_matching(|msg| msg.contains("\"diagnostics\":[]"))
        .ok_or("no empty diagnostics published")?;

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
    let _ = fixture.wait_for_diagnostics();

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

    // The server may still have a stale populated publish in flight from the
    // did_open; wait for the settled empty publish rather than the first one.
    let diag = fixture
        .wait_for_diagnostics_matching(|msg| msg.contains("\"diagnostics\":[]"))
        .ok_or("no empty diagnostics after change")?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

#[test]
fn test_lsp_did_close_clears_diagnostics() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let python_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code)?;
    let _ = fixture.wait_for_diagnostics();

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": {
            "textDocument": {
                "uri": "file:///test.py"
            }
        }
    }))?;

    // A populated publish from the did_open may still be in flight; wait for
    // the clearing publish that didClose triggers rather than the first one.
    let diag = fixture
        .wait_for_diagnostics_matching(|msg| msg.contains("\"diagnostics\":[]"))
        .ok_or("no clearing diagnostics after close")?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

#[test]
fn test_lsp_hover_on_error_location() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let python_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code)?;
    let _ = fixture.wait_for_diagnostics();

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
    assert!(hover.contains("BSK-0001"));
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

    // Drain diagnostic notifications until BOTH documents have been reported.
    // The server may publish for one document more than once (e.g. an initial
    // empty publish followed by the populated one) or in either order, so a
    // fixed two-notification read can miss doc2 under parallel load. Keep
    // reading until both URIs are seen (bounded) — this only strengthens the
    // gate: both documents must still receive diagnostics.
    let mut combined = String::new();
    for _ in 0..8 {
        let Some(msg) = fixture.wait_for_diagnostics() else {
            break;
        };
        combined.push('\n');
        combined.push_str(&msg);
        if combined.contains("file:///doc1.py") && combined.contains("file:///doc2.py") {
            break;
        }
    }

    assert!(
        combined.contains("file:///doc1.py"),
        "no diagnostics published for doc1: {combined}"
    );
    assert!(
        combined.contains("file:///doc2.py"),
        "no diagnostics published for doc2: {combined}"
    );
    assert!(
        combined.contains("BSK-0001"),
        "expected BSK-0001 in diagnostics: {combined}"
    );
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
    assert!(diag.matches("BSK-0001").count() >= 50);
    assert!(diag.matches("BSK-0002").count() >= 50);
    Ok(())
}
