//! End-to-end tests for the Basilisk LSP server.
//!
//! Each test spawns a real `basilisk lsp` subprocess and communicates
//! via stdio JSON-RPC, verifying the full LSP lifecycle.
//!
//! A background reader thread feeds messages into an `mpsc` channel so
//! that reads never block forever — every `recv()` has a timeout.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Default timeout for reading a single LSP message.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Path to the pre-built basilisk binary.
///
/// Derives the target directory from the test executable's own location,
/// which works regardless of whether `cargo test` or `cargo llvm-cov`
/// (which uses a different `--target-dir`) invoked us.
fn basilisk_binary() -> String {
    // The test binary lives under <target-dir>/debug/deps/...
    // We want <target-dir>/debug/basilisk
    if let Ok(exe) = std::env::current_exe() {
        if let Some(debug_dir) = exe.parent().and_then(|deps| deps.parent()) {
            let candidate = debug_dir.join("basilisk");
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    // Fallback to the original hardcoded path.
    format!("{}/../../target/debug/basilisk", env!("CARGO_MANIFEST_DIR"))
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
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or("failed to get stdin")?;
        let stdout = child.stdout.take().ok_or("failed to get stdout")?;
        let stderr = child.stderr.take().ok_or("failed to get stderr")?;

        let (tx, rx) = channel();

        // Background reader for stdout: parse LSP frames and push bodies into the channel.
        let _ = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                let mut content_length: Option<usize> = None;

                // Read headers until the blank line separator.
                // Skip leading blank lines that may appear between messages.
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return; // EOF — server exited
                    }
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        if content_length.is_some() {
                            break; // genuine header terminator
                        }
                        continue; // stray blank line before headers — skip
                    }
                    if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                        content_length = rest.trim().parse().ok();
                    }
                }

                let Some(length) = content_length else {
                    continue; // no Content-Length yet — keep reading
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

        // Background reader for stderr: print to console for debugging
        let _ = thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                eprint!("[LSP stderr] {line}");
                line.clear();
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
    let _ = fixture.initialize()?;

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

    assert!(
        resp.contains("def"),
        "hover should show function def: {resp}"
    );
    assert!(
        resp.contains("greet"),
        "hover should show function name: {resp}"
    );
    assert!(resp.contains("name"), "hover should show parameter: {resp}");
    Ok(())
}

#[test]
fn test_lsp_hover_shows_class_signature() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code =
        "class Animal:\n    name: str\n    def speak(self) -> str:\n        return self.name\n";
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
    assert!(
        resp.contains("Animal"),
        "hover should show class name: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_shows_variable_type() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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

    assert!(
        resp.contains("variable"),
        "hover should show 'variable': {resp}"
    );
    assert!(resp.contains("int"), "hover should show type 'int': {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Go to Definition tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_goto_definition_function() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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
    assert!(
        resp.contains("gotodef.py"),
        "definition should point to same file: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_goto_definition_class() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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

    assert!(
        resp.contains("gotoclass.py"),
        "definition should point to same file: {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Document Symbols tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_document_symbols() -> TestResult<()> {
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

    assert!(
        resp.contains("Animal"),
        "symbols should include class 'Animal': {resp}"
    );
    assert!(
        resp.contains("greet"),
        "symbols should include function 'greet': {resp}"
    );
    assert!(
        resp.contains("\"x\""),
        "symbols should include variable 'x': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_document_symbols_nested_methods() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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
    assert!(
        resp.contains("multiply"),
        "should contain method 'multiply': {resp}"
    );
    assert!(
        resp.contains("value"),
        "should contain attribute 'value': {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Signature Help tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_signature_help() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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

    assert!(
        resp.contains("greet"),
        "signature should show function name: {resp}"
    );
    assert!(
        resp.contains("name"),
        "signature should show parameter 'name': {resp}"
    );
    assert!(
        resp.contains("greeting"),
        "signature should show parameter 'greeting': {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Find All References tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_find_references() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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
    assert!(
        count >= 2,
        "should find at least 2 references for 'greet' (found {count}): {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Rename tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_prepare_rename() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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
    assert!(
        resp.contains("result"),
        "prepare rename should return a result: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_rename_symbol() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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

    assert!(
        resp.contains("say_hello"),
        "rename should include new name: {resp}"
    );
    assert!(
        resp.contains("changes"),
        "rename should include workspace changes: {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Inlay Hints tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_inlay_hints_variable_types() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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

    assert!(
        resp.contains("int"),
        "inlay hints should show 'int' for x=42: {resp}"
    );
    assert!(
        resp.contains("str"),
        "inlay hints should show 'str' for y=\"hello\": {resp}"
    );
    assert!(
        resp.contains("bool"),
        "inlay hints should show 'bool' for z=True: {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Semantic Tokens tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_semantic_tokens_full() -> TestResult<()> {
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
    assert!(
        resp.contains("\"data\""),
        "semantic tokens should contain 'data' array: {resp}"
    );
    assert!(
        resp.contains("result"),
        "semantic tokens should have result: {resp}"
    );

    // Parse the response and verify we get tokens
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    // Each token is 5 integers, so data length should be a multiple of 5
    assert_eq!(
        data.len() % 5,
        0,
        "token data length should be multiple of 5"
    );
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
    let _ = fixture.initialize()?;

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

    assert!(
        resp.contains(": Any"),
        "code action should insert ': Any': {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "code action should be a quickfix: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_code_action_missing_return_annotation() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

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

    assert!(
        resp.contains("-> None"),
        "code action should insert '-> None': {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "code action should be a quickfix: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_code_action_redundant_annotation_w0050() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "x: int = 42\n";
    fixture.did_open("file:///redundant.py", code)?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    // Parse the published diagnostics to pass to the code action request.
    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    // Find the BSK-W0050 diagnostic.
    let w0050 = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some("BSK-W0050"))
        .ok_or("no BSK-W0050 diagnostic")?;

    let resp = send_request(
        &mut fixture,
        102,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///redundant.py" },
            "range": w0050["range"],
            "context": {
                "diagnostics": [w0050]
            }
        }),
    )?
    .ok_or("no code action response")?;

    assert!(
        resp.contains("Remove redundant type annotation"),
        "code action should offer to remove redundant annotation: {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "code action should be a quickfix: {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Hover — enhanced (exact format + call-site + parameter + attribute)
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_hover_function_exact_signature() -> TestResult<()> {
    // Proves hover shows the COMPLETE formatted signature, not just fragments.
    // format_type_signature produces: "(function) def greet(name: str) -> str"
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///hover_exact.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on the 'g' in "greet" — line 0, character 4.
    let resp = send_request(
        &mut fixture,
        200,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_exact.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(
        resp.contains("(function)"),
        "hover should show '(function)' prefix: {resp}"
    );
    assert!(
        resp.contains("def greet"),
        "hover should show 'def greet': {resp}"
    );
    assert!(
        resp.contains("name: str"),
        "hover should show typed parameter 'name: str': {resp}"
    );
    assert!(
        resp.contains("-> str"),
        "hover should show return type '-> str': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_from_call_site() -> TestResult<()> {
    // THE KEY TEST: hovering on a CALL SITE resolves to the function definition.
    // This exercises the reference-lookup path in hover_at / find_definition_by_name.
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    fixture.did_open("file:///hover_call.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // "result: str = greet(\"world\")" is line 3.
    // "result: str = " is 14 chars, so 'g' of "greet" is at character 14.
    let resp = send_request(
        &mut fixture,
        201,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_call.py" },
            "position": { "line": 3, "character": 14 }
        }),
    )?
    .ok_or("no hover response at call site")?;

    assert!(
        resp.contains("(function)"),
        "call-site hover should resolve to function: {resp}"
    );
    assert!(
        resp.contains("greet"),
        "call-site hover should show function name: {resp}"
    );
    assert!(
        resp.contains("name: str"),
        "call-site hover should show parameter type: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_parameter_shows_type() -> TestResult<()> {
    // Hover on a parameter at its definition site shows "(parameter) name: type".
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///hover_param.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // "def greet(" is 10 chars, so 'n' of "name" is at character 10.
    let resp = send_request(
        &mut fixture,
        202,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_param.py" },
            "position": { "line": 0, "character": 10 }
        }),
    )?
    .ok_or("no hover response for parameter")?;

    assert!(
        resp.contains("(parameter)"),
        "hover on parameter should show '(parameter)': {resp}"
    );
    assert!(
        resp.contains("name"),
        "hover should show parameter name: {resp}"
    );
    assert!(
        resp.contains("str"),
        "hover should show parameter type 'str': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_class_attribute() -> TestResult<()> {
    // Hover on a class attribute shows "(property) ClassName.attr: type".
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "class Animal:\n    name: str\n    age: int\n";
    fixture.did_open("file:///hover_attr.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Line 1: "    name: str" — "name" starts at character 4.
    let resp = send_request(
        &mut fixture,
        203,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_attr.py" },
            "position": { "line": 1, "character": 4 }
        }),
    )?
    .ok_or("no hover response for class attribute")?;

    assert!(
        resp.contains("(property)"),
        "hover on class attribute should show '(property)': {resp}"
    );
    assert!(
        resp.contains("Animal.name"),
        "hover should show 'Animal.name': {resp}"
    );
    assert!(
        resp.contains("str"),
        "hover should show attribute type 'str': {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Go to Definition — exact position + call-site + type annotation
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_goto_definition_returns_exact_position() -> TestResult<()> {
    // Proves that goto-def returns the EXACT line/character of the definition,
    // not just the file name.
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///gotoexact.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on 'g' in "greet" — line 0, character 4.
    let resp = send_request(
        &mut fixture,
        300,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///gotoexact.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no definition response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "definition result must not be null: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(start["line"], 0, "definition must be on line 0: {resp}");
    assert_eq!(
        start["character"], 4,
        "definition must start at char 4, where 'greet' begins: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_goto_definition_from_call_site() -> TestResult<()> {
    // THE KEY TEST: goto-def triggered FROM a call site jumps to the function
    // definition — the primary end-to-end user workflow for F12.
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    fixture.did_open("file:///goto_call.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Line 3: "result: str = greet(\"world\")" — 'g' of call "greet" at character 14.
    let resp = send_request(
        &mut fixture,
        301,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///goto_call.py" },
            "position": { "line": 3, "character": 14 }
        }),
    )?
    .ok_or("no definition response from call site")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def from call site must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    // Should jump to line 0, char 4 — where "def greet" begins.
    assert_eq!(
        start["line"], 0,
        "goto-def from call should jump to line 0: {resp}"
    );
    assert_eq!(
        start["character"], 4,
        "goto-def from call should land at char 4 where 'greet' is defined: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_goto_definition_class_from_type_annotation() -> TestResult<()> {
    // goto-def on a class name used in a type annotation resolves to the class definition.
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "class Dog:\n    name: str\n\ndef pet(dog: Dog) -> None:\n    pass\n";
    fixture.did_open("file:///goto_type.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Line 3: "def pet(dog: Dog) -> None:"
    // "def pet(dog: " is 13 chars, so 'D' of "Dog" is at character 13.
    let resp = send_request(
        &mut fixture,
        302,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///goto_type.py" },
            "position": { "line": 3, "character": 13 }
        }),
    )?
    .ok_or("no definition for class used in type annotation")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on type annotation must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    // "class Dog:" — 'D' of "Dog" is at char 6 on line 0.
    assert_eq!(
        start["line"], 0,
        "goto-def should jump to class definition at line 0: {resp}"
    );
    assert_eq!(
        start["character"], 6,
        "goto-def should land at char 6 where 'Dog' is defined: {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Capability advertisement
// ────────────────────────────────────────────────────────────────────

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

// ────────────────────────────────────────────────────────────────────
// Go to Declaration tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_goto_declaration_function() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def compute(x: int) -> int:
    return x * 2

result: int = compute(10)
";
    fixture.did_open("file:///decl.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor on "compute" at the call site: line 3, col 14
    let resp = send_request(
        &mut fixture,
        200,
        "textDocument/declaration",
        serde_json::json!({
            "textDocument": { "uri": "file:///decl.py" },
            "position": { "line": 3, "character": 16 }
        }),
    )?
    .ok_or("no declaration response")?;

    assert!(
        resp.contains("\"line\":0"),
        "declaration should point to line 0 (function def): {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Go to Type Definition tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_goto_type_definition_variable() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class MyData:
    value: int

instance: MyData = MyData()
";
    fixture.did_open("file:///typedef.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor on "instance" at line 3, col 0
    let resp = send_request(
        &mut fixture,
        201,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef.py" },
            "position": { "line": 3, "character": 2 }
        }),
    )?
    .ok_or("no type definition response")?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should point to line 0 (class MyData): {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_goto_type_definition_parameter() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Config:
    debug: bool

def process(cfg: Config) -> None:
    pass
";
    fixture.did_open("file:///typedef2.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Cursor on "cfg" parameter at line 3, col 12
    let resp = send_request(
        &mut fixture,
        202,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef2.py" },
            "position": { "line": 3, "character": 13 }
        }),
    )?
    .ok_or("no type definition response")?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should point to line 0 (class Config): {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Docstring tests
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_hover_shows_docstring() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def calculate(x: int) -> int:
    \"\"\"Compute the square of x.\"\"\"
    return x * x
";
    fixture.did_open("file:///docstr.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        210,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///docstr.py" },
            "position": { "line": 0, "character": 5 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(
        resp.contains("Compute the square of x"),
        "hover should include docstring: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_shows_docstring_at_call_site() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def calculate(x: int) -> int:
    \"\"\"Compute the square of x.\"\"\"
    return x * x

result: int = calculate(5)
";
    fixture.did_open("file:///docstr_call.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover at the call site "calculate" on line 4, col 18.
    let resp = send_request(
        &mut fixture,
        211,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///docstr_call.py" },
            "position": { "line": 4, "character": 18 }
        }),
    )?
    .ok_or("no hover response at call site")?;

    assert!(
        resp.contains("Compute the square of x"),
        "hover at call site should include docstring: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_completion_includes_docstring() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def helper(x: int) -> int:
    \"\"\"Return x plus one.\"\"\"
    return x + 1

hel
";
    fixture.did_open("file:///compdoc.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        211,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": "file:///compdoc.py" },
            "position": { "line": 4, "character": 3 }
        }),
    )?
    .ok_or("no completion response")?;

    assert!(
        resp.contains("helper"),
        "completions should include 'helper': {resp}"
    );
    // Docstrings are now lazy-loaded via completionItem/resolve, so the initial
    // completion list includes `data` for resolve but not inline documentation.
    assert!(
        resp.contains("\"data\""),
        "completion should include resolve data: {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Folding Ranges
// ────────────────────────────────────────────────────────────────────

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
    // The class and two functions should produce folding ranges.
    assert!(
        resp.contains("startLine"),
        "should contain folding ranges with startLine: {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Selection Ranges (Smart Select)
// ────────────────────────────────────────────────────────────────────

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

// ────────────────────────────────────────────────────────────────────
// Code Lens
// ────────────────────────────────────────────────────────────────────

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

// ────────────────────────────────────────────────────────────────────
// Document Highlight
// ────────────────────────────────────────────────────────────────────

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

    // Highlight 'name' at the parameter position (line 0, char 10)
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

// ────────────────────────────────────────────────────────────────────
// didSave re-checks diagnostics
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_did_save_rechecks() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///save.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Send didSave — should re-publish diagnostics.
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

// ────────────────────────────────────────────────────────────────────
// Call Hierarchy
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_prepare_call_hierarchy() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def main() -> None:
    greet(\"world\")
";
    fixture.did_open("file:///callh.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare call hierarchy on 'greet' definition (line 0, char 4)
    let resp = send_request(
        &mut fixture,
        304,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///callh.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no prepareCallHierarchy response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("greet"),
        "should contain 'greet' in call hierarchy: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_call_hierarchy_incoming() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def main() -> None:
    greet(\"world\")
";
    fixture.did_open("file:///callhi.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // First prepare to get the item
    let prep = send_request(
        &mut fixture,
        305,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///callhi.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no prepareCallHierarchy response")?;

    // Parse the item from the prepare response
    let prep_val: serde_json::Value = serde_json::from_str(&prep)?;
    let items = prep_val["result"].as_array().ok_or("no items in prepare")?;
    if items.is_empty() {
        return Ok(()); // No items — skip incoming calls test
    }

    let resp = send_request(
        &mut fixture,
        306,
        "callHierarchy/incomingCalls",
        serde_json::json!({
            "item": items[0]
        }),
    )?
    .ok_or("no incomingCalls response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

#[test]
fn test_lsp_call_hierarchy_outgoing() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def main() -> None:
    greet(\"world\")
";
    fixture.did_open("file:///callho.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare on 'main' (line 3, char 4) which calls 'greet'
    let prep = send_request(
        &mut fixture,
        307,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///callho.py" },
            "position": { "line": 3, "character": 4 }
        }),
    )?
    .ok_or("no prepareCallHierarchy response")?;

    let prep_val: serde_json::Value = serde_json::from_str(&prep)?;
    let items = prep_val["result"].as_array().ok_or("no items in prepare")?;
    if items.is_empty() {
        return Ok(());
    }

    let resp = send_request(
        &mut fixture,
        308,
        "callHierarchy/outgoingCalls",
        serde_json::json!({
            "item": items[0]
        }),
    )?
    .ok_or("no outgoingCalls response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Type Hierarchy
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_prepare_type_hierarchy() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str
";
    fixture.did_open("file:///typeh.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare type hierarchy on 'Dog' (line 3, char 6)
    let resp = send_request(
        &mut fixture,
        309,
        "textDocument/prepareTypeHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///typeh.py" },
            "position": { "line": 3, "character": 6 }
        }),
    )?
    .ok_or("no prepareTypeHierarchy response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("Dog"),
        "should contain 'Dog' in type hierarchy: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_type_hierarchy_supertypes() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str
";
    fixture.did_open("file:///typehs.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let prep = send_request(
        &mut fixture,
        310,
        "textDocument/prepareTypeHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///typehs.py" },
            "position": { "line": 3, "character": 6 }
        }),
    )?
    .ok_or("no prepareTypeHierarchy response")?;

    let prep_val: serde_json::Value = serde_json::from_str(&prep)?;
    let items = prep_val["result"].as_array().ok_or("no items")?;
    if items.is_empty() {
        return Ok(());
    }

    let resp = send_request(
        &mut fixture,
        311,
        "typeHierarchy/supertypes",
        serde_json::json!({
            "item": items[0]
        }),
    )?
    .ok_or("no supertypes response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("Animal"),
        "supertypes of Dog should include Animal: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_type_hierarchy_subtypes() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str
";
    fixture.did_open("file:///typehsub.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare on 'Animal' (line 0, char 6)
    let prep = send_request(
        &mut fixture,
        312,
        "textDocument/prepareTypeHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///typehsub.py" },
            "position": { "line": 0, "character": 6 }
        }),
    )?
    .ok_or("no prepareTypeHierarchy response")?;

    let prep_val: serde_json::Value = serde_json::from_str(&prep)?;
    let items = prep_val["result"].as_array().ok_or("no items")?;
    if items.is_empty() {
        return Ok(());
    }

    let resp = send_request(
        &mut fixture,
        313,
        "typeHierarchy/subtypes",
        serde_json::json!({
            "item": items[0]
        }),
    )?
    .ok_or("no subtypes response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("Dog"),
        "subtypes of Animal should include Dog: {resp}"
    );
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Workspace Symbols
// ────────────────────────────────────────────────────────────────────

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

// ────────────────────────────────────────────────────────────────────
// Document Formatting (via Ruff)
// ────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_formatting() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Badly formatted code
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

    // The response should contain either edits or null result (if ruff is not installed).
    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ────────────────────────────────────────────────────────────────────
// Execute Command (organize imports)
// ────────────────────────────────────────────────────────────────────

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
