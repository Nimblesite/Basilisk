//! End-to-end tests for the Basilisk LSP server.
//!
//! Each test spawns a real `basilisk lsp` subprocess and communicates
//! via stdio JSON-RPC, verifying the full LSP lifecycle.
//!
//! A background reader thread feeds messages into an `mpsc` channel so
//! that reads never block forever — every `recv()` has a timeout.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Duration;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Default timeout for reading a single LSP message.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Path to the pre-built basilisk binary.
fn basilisk_binary() -> String {
    format!(
        "{}/../../target/debug/basilisk",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Test fixture that manages a `basilisk lsp` child process.
struct LspTestFixture {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<String>,
}

impl LspTestFixture {
    /// Spawn the LSP server and start the background reader thread.
    fn new() -> TestResult<Self> {
        let mut child = Command::new(basilisk_binary())
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child.stdin.take().ok_or("failed to get stdin")?;
        let stdout = child.stdout.take().ok_or("failed to get stdout")?;

        let (tx, rx) = channel();

        // Background reader: parse LSP frames and push bodies into the channel.
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut content_length: Option<usize> = None;
                let mut line = String::new();

                // Read headers until the blank line separator.
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return; // EOF — server exited
                    }
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        break;
                    }
                    if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                        content_length = rest.trim().parse().ok();
                    }
                }

                let Some(length) = content_length else {
                    return;
                };
                let mut buf = vec![0u8; length];
                if reader.read_exact(&mut buf).is_err() {
                    return;
                }
                if let Ok(body) = String::from_utf8(buf) {
                    if tx.send(body).is_err() {
                        return; // receiver dropped
                    }
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            responses: rx,
        })
    }

    /// Send a `serde_json::Value` as an LSP frame.
    fn send_json(&mut self, value: &serde_json::Value) -> TestResult<()> {
        let body = value.to_string();
        let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        self.stdin.write_all(frame.as_bytes())?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Read the next message from the server (with timeout).
    fn recv(&self) -> Option<String> {
        self.responses.recv_timeout(READ_TIMEOUT).ok()
    }

    /// Perform the full initialize / initialized handshake.
    fn initialize(&mut self) -> TestResult<String> {
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": null,
                "capabilities": {},
                "trace": "off"
            }
        }))?;

        let response = self.recv().ok_or("no response to initialize")?;

        // Complete the handshake with the required `initialized` notification.
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }))?;

        // Drain the server's "Basilisk LSP initialized" log message.
        let _ = self.responses.recv_timeout(Duration::from_millis(500));

        Ok(response)
    }

    /// Send `textDocument/didOpen`.
    fn did_open(&mut self, uri: &str, text: &str) -> TestResult<()> {
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": "python",
                    "version": 1,
                    "text": text
                }
            }
        }))
    }

    /// Wait for a `publishDiagnostics` notification, skipping unrelated messages.
    fn wait_for_diagnostics(&self) -> Option<String> {
        for _ in 0..10 {
            let msg = self.recv()?;
            if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") {
                return Some(msg);
            }
        }
        None
    }
}

impl Drop for LspTestFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_initialize() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let response = fixture.initialize()?;

    assert!(response.contains("\"jsonrpc\":\"2.0\""));
    assert!(response.contains("\"id\":1"));
    assert!(response.contains("\"result\""));
    assert!(response.contains("\"basilisk\""));
    assert!(response.contains("\"textDocumentSync\":1"));
    assert!(response.contains("\"hoverProvider\":true"));
    assert!(response.contains("\"codeActionProvider\":true"));
    Ok(())
}

#[test]
fn test_lsp_did_open_with_type_errors() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

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
    fixture.initialize()?;

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
    fixture.initialize()?;

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
    fixture.initialize()?;

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
    fixture.initialize()?;

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
    fixture.initialize()?;

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
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    // Send raw malformed JSON (not via send_json which would serialize properly).
    let bad = "{ invalid json }";
    let frame = format!("Content-Length: {}\r\n\r\n{}", bad.len(), bad);
    fixture.stdin.write_all(frame.as_bytes())?;
    fixture.stdin.flush()?;

    let error_response = fixture.recv().ok_or("no error response")?;

    assert!(error_response.contains("\"error\""));
    assert!(error_response.contains("-32700"));
    assert!(error_response.contains("Parse error"));
    Ok(())
}

#[test]
fn test_lsp_unknown_method_handling() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

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
    fixture.initialize()?;

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
    fixture.initialize()?;

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

// ────────────────────────────────────────────────────────────────────
// Completion (IntelliSense) tests
// ────────────────────────────────────────────────────────────────────

/// Helper: send a `textDocument/completion` request and wait for the response.
fn request_completion(
    fixture: &mut LspTestFixture,
    uri: &str,
    line: u32,
    character: u32,
    request_id: u64,
) -> TestResult<Option<String>> {
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        }
    }))?;

    let id_str = format!("\"id\":{request_id}");
    for _ in 0..10 {
        let Some(msg) = fixture.recv() else { break };
        if msg.contains(&id_str) {
            return Ok(Some(msg));
        }
    }
    Ok(None)
}

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
    fixture.initialize()?;

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
    assert!(resp.contains("\"label\":\"greet\""), "should complete function 'greet': {resp}");
    assert!(resp.contains("\"label\":\"Animal\""), "should complete class 'Animal': {resp}");
    assert!(resp.contains("\"label\":\"x\""), "should complete variable 'x': {resp}");

    // Should also contain builtins
    assert!(resp.contains("\"label\":\"print\""), "should complete builtin 'print': {resp}");
    assert!(resp.contains("\"label\":\"len\""), "should complete builtin 'len': {resp}");
    Ok(())
}

#[test]
fn test_lsp_completion_prefix_filtering() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

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

    assert!(resp.contains("\"label\":\"greet\""), "should match 'greet' for prefix 'gr': {resp}");
    assert!(!resp.contains("\"label\":\"helper\""), "should NOT match 'helper' for prefix 'gr': {resp}");
    assert!(!resp.contains("\"label\":\"goodbye\""), "should NOT match 'goodbye' for prefix 'gr': {resp}");
    Ok(())
}

#[test]
fn test_lsp_completion_imports() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "\
from typing import Optional, List
import os

";
    fixture.did_open("file:///imports.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Completion at empty position (line 3)
    let resp = request_completion(&mut fixture, "file:///imports.py", 3, 0, 12)?
        .ok_or("no completion response")?;

    assert!(resp.contains("\"label\":\"Optional\""), "should complete imported 'Optional': {resp}");
    assert!(resp.contains("\"label\":\"List\""), "should complete imported 'List': {resp}");
    assert!(resp.contains("\"label\":\"os\""), "should complete imported module 'os': {resp}");
    Ok(())
}

#[test]
fn test_lsp_completion_dot_on_class() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

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

    assert!(resp.contains("\"label\":\"name\""), "should complete attribute 'name': {resp}");
    assert!(resp.contains("\"label\":\"breed\""), "should complete attribute 'breed': {resp}");
    assert!(resp.contains("\"label\":\"bark\""), "should complete method 'bark': {resp}");
    assert!(resp.contains("\"label\":\"fetch\""), "should complete method 'fetch': {resp}");
    Ok(())
}

#[test]
fn test_lsp_completion_self_dot() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

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

    assert!(resp.contains("\"label\":\"color\""), "should complete self.color: {resp}");
    assert!(resp.contains("\"label\":\"age\""), "should complete self.age: {resp}");
    assert!(resp.contains("\"label\":\"meow\""), "should complete self.meow: {resp}");
    assert!(resp.contains("\"label\":\"describe\""), "should complete self.describe: {resp}");
    Ok(())
}

#[test]
fn test_lsp_completion_builtins() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "pri";
    fixture.did_open("file:///builtins.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor after "pri" (line 0, character 3)
    let resp = request_completion(&mut fixture, "file:///builtins.py", 0, 3, 15)?
        .ok_or("no completion response")?;

    assert!(resp.contains("\"label\":\"print\""), "should complete builtin 'print' for prefix 'pri': {resp}");
    // Should NOT include unrelated builtins
    assert!(!resp.contains("\"label\":\"len\""), "should NOT include 'len' for prefix 'pri': {resp}");
    assert!(!resp.contains("\"label\":\"map\""), "should NOT include 'map' for prefix 'pri': {resp}");
    Ok(())
}

#[test]
fn test_lsp_completion_function_detail_shows_params() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "\
def calculate(x: int, y: int, op: str) -> int:
    return x

cal";
    fixture.did_open("file:///detail.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor after "cal" (line 3, character 3)
    let resp = request_completion(&mut fixture, "file:///detail.py", 3, 3, 16)?
        .ok_or("no completion response")?;

    assert!(resp.contains("\"label\":\"calculate\""), "should complete 'calculate': {resp}");
    // The detail should include the parameter signature
    assert!(resp.contains("x, y, op"), "should show params in detail: {resp}");
    Ok(())
}

#[test]
fn test_lsp_completion_on_empty_file() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    // Empty file should still return builtins
    fixture.did_open("file:///empty.py", "")?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_completion(&mut fixture, "file:///empty.py", 0, 0, 17)?
        .ok_or("no completion response")?;

    // Should contain builtins
    assert!(resp.contains("\"label\":\"print\""), "empty file should still offer builtins: {resp}");
    assert!(resp.contains("\"label\":\"int\""), "empty file should still offer 'int': {resp}");
    assert!(resp.contains("\"label\":\"str\""), "empty file should still offer 'str': {resp}");
    assert!(resp.contains("\"label\":\"True\""), "empty file should still offer 'True': {resp}");
    assert!(resp.contains("\"label\":\"Exception\""), "empty file should still offer 'Exception': {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Generic LSP request helper
// ────────────────────────────────────────────────────────────────────

/// Send an LSP request and wait for the response matching the given id.
#[allow(clippy::needless_pass_by_value)]
fn send_request(
    fixture: &mut LspTestFixture,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> TestResult<Option<String>> {
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    }))?;

    let id_str = format!("\"id\":{id}");
    for _ in 0..10 {
        let Some(msg) = fixture.recv() else { break };
        if msg.contains(&id_str) {
            return Ok(Some(msg));
        }
    }
    Ok(None)
}

// ────────────────────────────────────────────────────────────────────
// Hover (type signature) tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_hover_shows_function_signature() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///hover.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on "greet" (line 0, character 4)
    let resp = send_request(
        &mut fixture,
        20,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(resp.contains("def"), "hover should show function def: {resp}");
    assert!(resp.contains("greet"), "hover should show function name: {resp}");
    assert!(resp.contains("name"), "hover should show parameter: {resp}");
    Ok(())
}

#[test]
fn test_lsp_hover_shows_class_signature() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "class Animal:\n    name: str\n    def speak(self) -> str:\n        return self.name\n";
    fixture.did_open("file:///hclass.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on "Animal" (line 0, character 6)
    let resp = send_request(
        &mut fixture,
        21,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hclass.py" },
            "position": { "line": 0, "character": 6 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(resp.contains("class"), "hover should show 'class': {resp}");
    assert!(resp.contains("Animal"), "hover should show class name: {resp}");
    Ok(())
}

#[test]
fn test_lsp_hover_shows_variable_type() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "x: int = 42\n";
    fixture.did_open("file:///hvar.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on "x" (line 0, character 0)
    let resp = send_request(
        &mut fixture,
        22,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hvar.py" },
            "position": { "line": 0, "character": 0 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(resp.contains("variable"), "hover should show 'variable': {resp}");
    assert!(resp.contains("int"), "hover should show type 'int': {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Go to Definition tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_goto_definition_function() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///gotodef.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Go to definition on "greet" (line 0, character 4)
    let resp = send_request(
        &mut fixture,
        30,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///gotodef.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no definition response")?;

    // Should return a location pointing to the function definition
    assert!(resp.contains("gotodef.py"), "definition should point to same file: {resp}");
    Ok(())
}

#[test]
fn test_lsp_goto_definition_class() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "class Dog:\n    name: str\n    def bark(self) -> str:\n        return \"woof\"\n";
    fixture.did_open("file:///gotoclass.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Go to definition on "Dog" (line 0, character 6)
    let resp = send_request(
        &mut fixture,
        31,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///gotoclass.py" },
            "position": { "line": 0, "character": 6 }
        }),
    )?
    .ok_or("no definition response")?;

    assert!(resp.contains("gotoclass.py"), "definition should point to same file: {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Document Symbols tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_document_symbols() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///symbols.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        40,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///symbols.py" }
        }),
    )?
    .ok_or("no document symbols response")?;

    assert!(resp.contains("Animal"), "symbols should include class 'Animal': {resp}");
    assert!(resp.contains("greet"), "symbols should include function 'greet': {resp}");
    assert!(resp.contains("\"x\""), "symbols should include variable 'x': {resp}");
    Ok(())
}

#[test]
fn test_lsp_document_symbols_nested_methods() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "\
class Calculator:
    value: int
    def add(self, x: int) -> int:
        return self.value + x
    def multiply(self, x: int) -> int:
        return self.value * x
";
    fixture.did_open("file:///nested.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        41,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///nested.py" }
        }),
    )?
    .ok_or("no document symbols response")?;

    assert!(resp.contains("Calculator"), "should contain class: {resp}");
    assert!(resp.contains("add"), "should contain method 'add': {resp}");
    assert!(resp.contains("multiply"), "should contain method 'multiply': {resp}");
    assert!(resp.contains("value"), "should contain attribute 'value': {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Signature Help tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_signature_help() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "\
def greet(name: str, greeting: str) -> str:
    return f\"{greeting}, {name}!\"

result: str = greet(\"world\", \"Hi\")
";
    fixture.did_open("file:///sighel.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor inside the greet() call — after the opening paren
    // "result: str = greet(" is line 3, character 20
    let resp = send_request(
        &mut fixture,
        50,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": "file:///sighel.py" },
            "position": { "line": 3, "character": 21 }
        }),
    )?
    .ok_or("no signature help response")?;

    assert!(resp.contains("greet"), "signature should show function name: {resp}");
    assert!(resp.contains("name"), "signature should show parameter 'name': {resp}");
    assert!(resp.contains("greeting"), "signature should show parameter 'greeting': {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Find All References tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_find_references() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

result: str = greet(\"world\")
";
    fixture.did_open("file:///refs.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Find references for "greet" (line 0, character 4)
    let resp = send_request(
        &mut fixture,
        60,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": "file:///refs.py" },
            "position": { "line": 0, "character": 4 },
            "context": { "includeDeclaration": true }
        }),
    )?
    .ok_or("no references response")?;

    // Should find at least 2 references (definition + usage)
    let count = resp.matches("refs.py").count();
    assert!(count >= 2, "should find at least 2 references for 'greet' (found {count}): {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Rename tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_prepare_rename() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///rename.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare rename on "greet" (line 0, character 4)
    let resp = send_request(
        &mut fixture,
        70,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": "file:///rename.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no prepare rename response")?;

    // Should return a range covering "greet"
    assert!(resp.contains("result"), "prepare rename should return a result: {resp}");
    Ok(())
}

#[test]
fn test_lsp_rename_symbol() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

result: str = greet(\"world\")
";
    fixture.did_open("file:///ren.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Rename "greet" to "say_hello" (line 0, character 4)
    let resp = send_request(
        &mut fixture,
        71,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": "file:///ren.py" },
            "position": { "line": 0, "character": 4 },
            "newName": "say_hello"
        }),
    )?
    .ok_or("no rename response")?;

    assert!(resp.contains("say_hello"), "rename should include new name: {resp}");
    assert!(resp.contains("changes"), "rename should include workspace changes: {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Inlay Hints tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_inlay_hints_variable_types() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "x = 42\ny = \"hello\"\nz = True\n";
    fixture.did_open("file:///inlay.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        80,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": "file:///inlay.py" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 3, "character": 0 }
            }
        }),
    )?
    .ok_or("no inlay hint response")?;

    assert!(resp.contains("int"), "inlay hints should show 'int' for x=42: {resp}");
    assert!(resp.contains("str"), "inlay hints should show 'str' for y=\"hello\": {resp}");
    assert!(resp.contains("bool"), "inlay hints should show 'bool' for z=True: {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Semantic Tokens tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_semantic_tokens_full() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///semtok.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        90,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": "file:///semtok.py" }
        }),
    )?
    .ok_or("no semantic tokens response")?;

    // Should return a data array with encoded tokens
    assert!(resp.contains("\"data\""), "semantic tokens should contain 'data' array: {resp}");
    assert!(resp.contains("result"), "semantic tokens should have result: {resp}");

    // Parse the response and verify we get tokens
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    // Each token is 5 integers, so data length should be a multiple of 5
    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");
    // We should have tokens for Animal, name, speak, self, greet, animal, x at minimum
    assert!(data.len() >= 5, "should have at least 1 token: {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Code Actions tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_code_action_missing_param_annotation() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///actions.py", code)?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    // Parse the published diagnostics to pass to the code action request.
    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    // Find the BSK-E0001 diagnostic.
    let e0001 = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some("BSK-E0001"))
        .ok_or("no BSK-E0001 diagnostic")?;

    let resp = send_request(
        &mut fixture,
        100,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///actions.py" },
            "range": e0001["range"],
            "context": {
                "diagnostics": [e0001]
            }
        }),
    )?
    .ok_or("no code action response")?;

    assert!(resp.contains(": Any"), "code action should insert ': Any': {resp}");
    assert!(resp.contains("quickfix"), "code action should be a quickfix: {resp}");
    Ok(())
}

#[test]
fn test_lsp_code_action_missing_return_annotation() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code = "def greet(name: str):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///retact.py", code)?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let e0002 = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some("BSK-E0002"))
        .ok_or("no BSK-E0002 diagnostic")?;

    let resp = send_request(
        &mut fixture,
        101,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///retact.py" },
            "range": e0002["range"],
            "context": {
                "diagnostics": [e0002]
            }
        }),
    )?
    .ok_or("no code action response")?;

    assert!(resp.contains("-> None"), "code action should insert '-> None': {resp}");
    assert!(resp.contains("quickfix"), "code action should be a quickfix: {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Capability advertisement
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_initialize_advertises_new_capabilities() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let response = fixture.initialize()?;

    assert!(response.contains("\"definitionProvider\""), "should advertise definition: {response}");
    assert!(response.contains("\"documentSymbolProvider\""), "should advertise document symbols: {response}");
    assert!(response.contains("\"signatureHelpProvider\""), "should advertise signature help: {response}");
    assert!(response.contains("\"referencesProvider\""), "should advertise references: {response}");
    assert!(response.contains("\"renameProvider\""), "should advertise rename: {response}");
    assert!(response.contains("\"inlayHintProvider\""), "should advertise inlay hints: {response}");
    assert!(response.contains("\"semanticTokensProvider\""), "should advertise semantic tokens: {response}");
    Ok(())
}
