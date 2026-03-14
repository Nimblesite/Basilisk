#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Tests for LSP: `lsp_e2e_advanced`.

#![allow(dead_code)]
//! LSP E2E tests — Capabilities, Folding, Selection, Code Lens, Highlight,
//! didSave, Workspace Symbols, Formatting, Execute Command.

mod lsp_e2e_common;
use lsp_e2e_common::{send_request, LspTestFixture, TestResult};

// ── Capability advertisement ─────────────────────────────────────────────────

#[test]
fn test_lsp_initialize_advertises_new_capabilities() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let response = fixture.initialize()?;

    assert!(
        response.contains("\"definitionProvider\""),
        "should advertise definition: {response}"
    );
    assert!(
        response.contains("\"documentSymbolProvider\""),
        "should advertise document symbols: {response}"
    );
    assert!(
        response.contains("\"signatureHelpProvider\""),
        "should advertise signature help: {response}"
    );
    assert!(
        response.contains("\"referencesProvider\""),
        "should advertise references: {response}"
    );
    assert!(
        response.contains("\"renameProvider\""),
        "should advertise rename: {response}"
    );
    assert!(
        response.contains("\"inlayHintProvider\""),
        "should advertise inlay hints: {response}"
    );
    assert!(
        response.contains("\"semanticTokensProvider\""),
        "should advertise semantic tokens: {response}"
    );
    assert!(
        response.contains("\"declarationProvider\""),
        "should advertise declaration: {response}"
    );
    assert!(
        response.contains("\"typeDefinitionProvider\""),
        "should advertise type definition: {response}"
    );
    Ok(())
}

// ── Folding Ranges ───────────────────────────────────────────────────────────

#[test]
fn test_lsp_folding_range() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(name: str) -> str:
    return f\"Hello, {name}!\"
";
    fixture.did_open("file:///fold.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        300,
        "textDocument/foldingRange",
        serde_json::json!({
            "textDocument": { "uri": "file:///fold.py" }
        }),
    )?
    .ok_or("no foldingRange response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("startLine"),
        "should contain folding ranges with startLine: {resp}"
    );
    Ok(())
}

// ── Selection Ranges (Smart Select) ──────────────────────────────────────────

#[test]
fn test_lsp_selection_range() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"
";
    fixture.did_open("file:///sel.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        301,
        "textDocument/selectionRange",
        serde_json::json!({
            "textDocument": { "uri": "file:///sel.py" },
            "positions": [{ "line": 0, "character": 4 }]
        }),
    )?
    .ok_or("no selectionRange response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ── Code Lens ────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_code_lens() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def caller() -> None:
    greet(\"world\")
    greet(\"test\")
";
    fixture.did_open("file:///lens.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        302,
        "textDocument/codeLens",
        serde_json::json!({
            "textDocument": { "uri": "file:///lens.py" }
        }),
    )?
    .ok_or("no codeLens response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ── Document Highlight ───────────────────────────────────────────────────────

#[test]
fn test_lsp_document_highlight() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return name
";
    fixture.did_open("file:///hl.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        303,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": "file:///hl.py" },
            "position": { "line": 0, "character": 10 }
        }),
    )?
    .ok_or("no documentHighlight response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ── didSave re-checks diagnostics ────────────────────────────────────────────

#[test]
fn test_lsp_did_save_rechecks() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///save.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didSave",
        "params": {
            "textDocument": { "uri": "file:///save.py" }
        }
    }))?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics after save")?;

    assert!(
        diag.contains("\"diagnostics\":[]"),
        "clean code should have empty diagnostics after save: {diag}"
    );
    Ok(())
}

// ── Workspace Symbols ────────────────────────────────────────────────────────

#[test]
fn test_lsp_workspace_symbol() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str

def greet(name: str) -> str:
    return name
";
    fixture.did_open("file:///wssym.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        314,
        "workspace/symbol",
        serde_json::json!({
            "query": "greet"
        }),
    )?
    .ok_or("no workspace/symbol response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("greet"),
        "workspace symbols should contain 'greet': {resp}"
    );
    Ok(())
}

// ── Document Formatting (via Ruff) ───────────────────────────────────────────

#[test]
fn test_lsp_formatting() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def   greet( name:str )->str:\n    return    name\n";
    fixture.did_open("file:///fmt.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        315,
        "textDocument/formatting",
        serde_json::json!({
            "textDocument": { "uri": "file:///fmt.py" },
            "options": {
                "tabSize": 4,
                "insertSpaces": true
            }
        }),
    )?
    .ok_or("no formatting response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ── Execute Command ──────────────────────────────────────────────────────────

#[test]
fn test_lsp_execute_command_unknown() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let resp = send_request(
        &mut fixture,
        316,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.nonExistentCommand",
            "arguments": []
        }),
    )?
    .ok_or("no executeCommand response")?;

    assert!(
        resp.contains("\"result\""),
        "should have a result (null) for unknown command: {resp}"
    );
    Ok(())
}
