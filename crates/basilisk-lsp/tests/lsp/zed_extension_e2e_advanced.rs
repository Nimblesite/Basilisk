//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// E2E tests simulating the Zed extension's interaction with the Basilisk LSP.
//
// Tests: document symbols, execute commands, inlay hints, semantic tokens,
// go to definition, find references, formatting, multiple documents, docs URL.

use super::zed_e2e_common::*;

// ── Document Symbols ────────────────────────────────────────────────────────

/// Document symbols must work — Zed uses these for the outline panel.
#[test]
fn test_zed_document_symbols() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let code = "class MyClass:\n    def method(self) -> None:\n        pass\n\ndef standalone(x: int) -> int:\n    return x\n";
    fixture.did_open("file:///symbols.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let symbols = fixture.request(
        "textDocument/documentSymbol",
        &serde_json::json!({
            "textDocument": { "uri": "file:///symbols.py" }
        }),
    )?;

    let result = &symbols["result"];
    assert!(
        result.is_array(),
        "document symbols must return an array: {symbols}"
    );

    Ok(())
}

// ── Execute Commands ────────────────────────────────────────────────────────

/// Execute command: the Zed extension uses basilisk custom commands via LSP.
/// Verify the organize imports command works.
#[test]
fn test_zed_execute_organize_imports() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let code = "import os\nimport sys\n\ndef foo() -> None:\n    pass\n";
    fixture.did_open("file:///imports.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let result = fixture.request(
        "workspace/executeCommand",
        &serde_json::json!({
            "command": commands::ORGANIZE_IMPORTS,
            "arguments": [{ "uri": "file:///imports.py" }]
        }),
    )?;

    // Must not return an error.
    assert!(
        result.get("error").is_none(),
        "organize imports should not error: {result}"
    );

    Ok(())
}

/// Execute command: start debug session. Even if debugpy isn't installed,
/// the LSP should return a structured error (not crash).
// Tests [LSPDEBUG-START] / [LSPDEBUG-WIRE]: basilisk.startDebugSession dispatch.
#[test]
fn test_zed_execute_start_debug_session() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let result = fixture.request(
        "workspace/executeCommand",
        &serde_json::json!({
            "command": commands::START_DEBUG_SESSION,
            "arguments": []
        }),
    )?;

    // The command should either succeed (if debugpy is installed) or return
    // a structured error — either way, the LSP must stay alive.
    // Verify the LSP didn't crash by sending another request.
    let code = "x: int = 1\n";
    fixture.did_open("file:///alive_check.py", code)?;
    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("LSP died after startDebugSession")?;

    assert!(
        diag.contains("\"diagnostics\""),
        "LSP must still respond: {diag}"
    );

    // Check result shape — must be a response (not a crash).
    assert!(
        result.get("id").is_some(),
        "must have response id: {result}"
    );

    Ok(())
}

/// Execute command: stop debug session with a fake session ID.
/// Should not crash the LSP.
// Tests [LSPDEBUG-STOP] / [LSPDEBUG-WIRE]: basilisk.stopDebugSession dispatch.
#[test]
fn test_zed_execute_stop_debug_session() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let result = fixture.request(
        "workspace/executeCommand",
        &serde_json::json!({
            "command": commands::STOP_DEBUG_SESSION,
            "arguments": [{ "sessionId": "nonexistent-session-id" }]
        }),
    )?;

    // Must not crash. Result should indicate the session wasn't found.
    assert!(
        result.get("id").is_some(),
        "must have response id: {result}"
    );

    Ok(())
}

// ── Inlay Hints ─────────────────────────────────────────────────────────────

/// Inlay hints must work — Zed shows these inline in the editor.
#[test]
fn test_zed_inlay_hints() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let code = "def add(a: int, b: int) -> int:\n    return a + b\n\nresult = add(1, 2)\n";
    fixture.did_open("file:///hints.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let hints = fixture.request(
        "textDocument/inlayHint",
        &serde_json::json!({
            "textDocument": { "uri": "file:///hints.py" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 4, "character": 0 }
            }
        }),
    )?;

    // Must return a response (even if empty array).
    assert!(
        hints.get("result").is_some(),
        "inlay hints must return a result: {hints}"
    );

    Ok(())
}

// ── Semantic Tokens ─────────────────────────────────────────────────────────

/// Semantic tokens must work — Zed uses these with `semantic_tokens: combined`.
#[test]
fn test_zed_semantic_tokens() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let code = "def hello(name: str) -> str:\n    return name\n";
    fixture.did_open("file:///tokens.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let tokens = fixture.request(
        "textDocument/semanticTokens/full",
        &serde_json::json!({
            "textDocument": { "uri": "file:///tokens.py" }
        }),
    )?;

    assert!(
        tokens.get("result").is_some(),
        "semantic tokens must return a result: {tokens}"
    );

    Ok(())
}

// ── Navigation ──────────────────────────────────────────────────────────────

/// Go to definition must work.
#[test]
fn test_zed_go_to_definition() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\ngreet(\"world\")\n";
    fixture.did_open("file:///definition.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let definition = fixture.request(
        "textDocument/definition",
        &serde_json::json!({
            "textDocument": { "uri": "file:///definition.py" },
            "position": { "line": 3, "character": 1 }
        }),
    )?;

    assert!(
        definition.get("result").is_some(),
        "definition must return a result: {definition}"
    );

    Ok(())
}

/// Find references must work.
#[test]
fn test_zed_find_references() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\ngreet(\"a\")\ngreet(\"b\")\n";
    fixture.did_open("file:///refs.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let refs = fixture.request(
        "textDocument/references",
        &serde_json::json!({
            "textDocument": { "uri": "file:///refs.py" },
            "position": { "line": 0, "character": 5 },
            "context": { "includeDeclaration": true }
        }),
    )?;

    assert!(
        refs.get("result").is_some(),
        "references must return a result: {refs}"
    );

    Ok(())
}

// ── Formatting ──────────────────────────────────────────────────────────────

/// Formatting must work via the embedded Ruff formatter ([LSPFMT-ENGINE]).
#[test]
fn test_zed_formatting() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let code = "def  foo(  x:int  )->int:\n    return   x\n";
    fixture.did_open("file:///format.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let format_result = fixture.request(
        "textDocument/formatting",
        &serde_json::json!({
            "textDocument": { "uri": "file:///format.py" },
            "options": {
                "tabSize": 4,
                "insertSpaces": true
            }
        }),
    )?;

    assert!(
        format_result.get("id").is_some(),
        "formatting must return a response: {format_result}"
    );
    // The engine is embedded in the binary — badly formatted code MUST come
    // back Ruff-formatted, never a silent null (#254).
    assert!(
        format_result.to_string().contains("def foo(x: int) -> int:"),
        "formatting must produce ruff-format output: {format_result}"
    );

    Ok(())
}

// ── Multiple Documents ──────────────────────────────────────────────────────

/// Multiple documents open concurrently — the Zed editor can have many tabs.
#[test]
fn test_zed_multiple_documents() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let code_with_error = "def foo(x):\n    return x\n";
    let code_clean = "def bar(x: int) -> int:\n    return x\n";

    fixture.did_open("file:///doc_a.py", code_with_error)?;
    fixture.did_open("file:///doc_b.py", code_clean)?;

    // Collect diagnostics for both documents.
    let mut got_a = false;
    let mut got_b = false;

    for _ in 0..20 {
        let Some(msg) = fixture.recv() else { break };
        if msg.contains("doc_a.py") && msg.contains("BSK-E0001") {
            got_a = true;
        }
        if msg.contains("doc_b.py") && msg.contains("\"diagnostics\":[]") {
            got_b = true;
        }
        if got_a && got_b {
            break;
        }
    }

    assert!(got_a, "doc_a.py should have diagnostics");
    assert!(got_b, "doc_b.py should be clean");

    Ok(())
}

// ── Docs URL ────────────────────────────────────────────────────────────────

/// Verify the LSP uses the shared constant for docs URL.
#[test]
fn test_zed_diagnostic_docs_url() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

    let code = "def foo(x):\n    return x\n";
    fixture.did_open("file:///docs_url.py", code)?;

    let diag = fixture.wait_for_diagnostics().ok_or("no diagnostics")?;

    // Diagnostics should reference the Basilisk docs URL from basilisk_common.
    assert!(
        diag.contains(basilisk_common::diagnostics::DOCS_URL),
        "diagnostics should contain docs URL '{}': {diag}",
        basilisk_common::diagnostics::DOCS_URL
    );

    Ok(())
}
