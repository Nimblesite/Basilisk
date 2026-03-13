#![allow(dead_code)]

mod lsp_e2e_common;
use lsp_e2e_common::*;

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
    assert!(hover.contains("BSK-E0001"));
    assert!(hover.contains("Missing parameter type annotation"));
    Ok(())
}

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

// ── Completion (IntelliSense) ────────────────────────────────────────────────

#[test]
fn test_lsp_initialize_advertises_completion() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let response = fixture.initialize()?;

    assert!(response.contains("\"completionProvider\""));
    assert!(response.contains("\".\""));
    Ok(())
}

#[test]
fn test_lsp_completion_returns_functions_and_classes() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///comp.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Request completion at end of file (empty prefix → all symbols)
    let resp = request_completion(&mut fixture, "file:///comp.py", 9, 0, 10)?
        .ok_or("no completion response")?;

    // Should contain our function, class, and variable
    assert!(
        resp.contains("\"label\":\"greet\""),
        "should complete function 'greet': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"Animal\""),
        "should complete class 'Animal': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"x\""),
        "should complete variable 'x': {resp}"
    );

    // Should also contain builtins
    assert!(
        resp.contains("\"label\":\"print\""),
        "should complete builtin 'print': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"len\""),
        "should complete builtin 'len': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_completion_prefix_filtering() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return name

def goodbye(name: str) -> str:
    return name

def helper(x: int) -> int:
    return x

gr";
    fixture.did_open("file:///prefix.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor at the end of "gr" on the last line (line 9, character 2)
    let resp = request_completion(&mut fixture, "file:///prefix.py", 9, 2, 11)?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"greet\""),
        "should match 'greet' for prefix 'gr': {resp}"
    );
    assert!(
        !resp.contains("\"label\":\"helper\""),
        "should NOT match 'helper' for prefix 'gr': {resp}"
    );
    assert!(
        !resp.contains("\"label\":\"goodbye\""),
        "should NOT match 'goodbye' for prefix 'gr': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_completion_imports() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
from typing import Optional, List
import os

";
    fixture.did_open("file:///imports.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Completion at empty position (line 3)
    let resp = request_completion(&mut fixture, "file:///imports.py", 3, 0, 12)?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"Optional\""),
        "should complete imported 'Optional': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"List\""),
        "should complete imported 'List': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"os\""),
        "should complete imported module 'os': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_completion_dot_on_class() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Dog:
    name: str
    breed: str
    def bark(self) -> str:
        return \"woof\"
    def fetch(self, item: str) -> str:
        return item

Dog.";
    fixture.did_open("file:///dot.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor after "Dog." on last line (line 8, character 4)
    let resp = request_completion(&mut fixture, "file:///dot.py", 8, 4, 13)?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"name\""),
        "should complete attribute 'name': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"breed\""),
        "should complete attribute 'breed': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"bark\""),
        "should complete method 'bark': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"fetch\""),
        "should complete method 'fetch': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_completion_self_dot() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Cat:
    color: str
    age: int
    def meow(self) -> str:
        return \"meow\"
    def describe(self) -> str:
        return self.";
    fixture.did_open("file:///selfdot.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor after "self." inside describe method (line 6, character 20)
    let resp = request_completion(&mut fixture, "file:///selfdot.py", 6, 20, 14)?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"color\""),
        "should complete self.color: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"age\""),
        "should complete self.age: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"meow\""),
        "should complete self.meow: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"describe\""),
        "should complete self.describe: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_completion_builtins() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "pri";
    fixture.did_open("file:///builtins.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor after "pri" (line 0, character 3)
    let resp = request_completion(&mut fixture, "file:///builtins.py", 0, 3, 15)?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"print\""),
        "should complete builtin 'print' for prefix 'pri': {resp}"
    );
    // Should NOT include unrelated builtins
    assert!(
        !resp.contains("\"label\":\"len\""),
        "should NOT include 'len' for prefix 'pri': {resp}"
    );
    assert!(
        !resp.contains("\"label\":\"map\""),
        "should NOT include 'map' for prefix 'pri': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_completion_function_detail_shows_params() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def calculate(x: int, y: int, op: str) -> int:
    return x

cal";
    fixture.did_open("file:///detail.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor after "cal" (line 3, character 3)
    let resp = request_completion(&mut fixture, "file:///detail.py", 3, 3, 16)?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"calculate\""),
        "should complete 'calculate': {resp}"
    );
    // The detail should include the parameter signature
    assert!(
        resp.contains("x, y, op"),
        "should show params in detail: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_completion_on_empty_file() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Empty file should still return builtins
    fixture.did_open("file:///empty.py", "")?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_completion(&mut fixture, "file:///empty.py", 0, 0, 17)?
        .ok_or("no completion response")?;

    // Should contain builtins
    assert!(
        resp.contains("\"label\":\"print\""),
        "empty file should still offer builtins: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"int\""),
        "empty file should still offer 'int': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"str\""),
        "empty file should still offer 'str': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"True\""),
        "empty file should still offer 'True': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"Exception\""),
        "empty file should still offer 'Exception': {resp}"
    );
    Ok(())
}
