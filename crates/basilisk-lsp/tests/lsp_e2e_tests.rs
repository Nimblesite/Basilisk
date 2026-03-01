//! End-to-end tests for the Basilisk LSP server.
//!
//! These tests simulate real LSP client interactions and verify
//! the server produces correct responses for various scenarios.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Test fixture for LSP server communication.
struct LspTestFixture {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
}

impl LspTestFixture {
    /// Start the basilisk lsp server as a child process.
    fn new() -> Self {
        let mut child = Command::new("target/debug/basilisk")
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to start basilisk lsp");

        let stdin = Arc::new(Mutex::new(child.stdin.take().expect("failed to get stdin")));
        let stdout = Arc::new(Mutex::new(BufReader::new(
            child.stdout.take().expect("failed to get stdout"),
        )));

        Self {
            child,
            stdin,
            stdout,
        }
    }

    /// Send an LSP message to the server.
    fn send_message(&self, message: &str) {
        let content = format!("Content-Length: {}\r\n\r\n{}", message.len(), message);
        let mut stdin = self.stdin.lock().unwrap();
        stdin.write_all(content.as_bytes()).unwrap();
        stdin.flush().unwrap();
    }

    /// Read the next LSP response from the server.
    fn read_response(&self) -> Option<String> {
        let mut stdout = self.stdout.lock().unwrap();
        let mut line = String::new();

        // Read headers
        loop {
            line.clear();
            stdout.read_line(&mut line).ok()?;
            if line.trim().is_empty() {
                break;
            }
        }

        // Read content length
        let content_length_header = line
            .lines()
            .find(|l| l.starts_with("Content-Length:"))
            .expect("missing Content-Length header");
        let content_length: usize = content_length_header
            .split(':')
            .nth(1)
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        // Read the JSON content
        let mut content = vec![0; content_length];
        stdout.read_exact(&mut content).ok()?;
        Some(String::from_utf8(content).unwrap())
    }

    /// Send initialize request and wait for response.
    fn initialize(&self) -> String {
        let init_request = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": 12345,
                "rootUri": null,
                "capabilities": {},
                "trace": "off"
            }
        }"#;

        self.send_message(init_request);
        thread::sleep(Duration::from_millis(100));
        self.read_response().expect("no response to initialize")
    }

    /// Send a text document didOpen notification.
    fn did_open(&self, uri: &str, text: &str) {
        let did_open = format!(
            r#"{{
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {{
                "textDocument": {{
                    "uri": "{}",
                    "languageId": "python",
                    "version": 1,
                    "text": "{}"
                }}
            }}
        }}"#,
            uri,
            text.replace('"', "\\\"")
        );

        self.send_message(&did_open);
        thread::sleep(Duration::from_millis(200)); // Give time for processing
    }

    /// Wait for diagnostics to be published.
    fn wait_for_diagnostics(&self) -> Option<String> {
        // Read any pending messages
        thread::sleep(Duration::from_millis(300));
        self.read_response()
    }
}

impl Drop for LspTestFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn test_lsp_initialize() {
    let fixture = LspTestFixture::new();
    let response = fixture.initialize();

    assert!(response.contains("\"jsonrpc\":\"2.0\""));
    assert!(response.contains("\"id\":1"));
    assert!(response.contains("\"result\""));
    assert!(response.contains("\"basilisk\""));
    assert!(response.contains("\"textDocumentSync\":1"));
    assert!(response.contains("\"hoverProvider\":true"));
    assert!(response.contains("\"codeActionProvider\":true"));
}

#[test]
fn test_lsp_did_open_with_type_errors() {
    let fixture = LspTestFixture::new();
    fixture.initialize();

    let python_code = r#"def greet(name):
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", python_code);

    let diagnostics_response = fixture.wait_for_diagnostics().expect("no diagnostics published");

    assert!(diagnostics_response.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diagnostics_response.contains("BSK-E0001"));
    assert!(diagnostics_response.contains("BSK-E0002"));
    assert!(diagnostics_response.contains("Missing parameter type annotation"));
    assert!(diagnostics_response.contains("Missing return type annotation"));
}

#[test]
fn test_lsp_did_open_with_clean_code() {
    let fixture = LspTestFixture::new();
    fixture.initialize();

    let python_code = r#"def greet(name: str) -> str:
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", python_code);

    let diagnostics_response = fixture.wait_for_diagnostics().expect("no diagnostics published");

    // Clean code should have empty diagnostics array
    assert!(diagnostics_response.contains("\"diagnostics\":[]"));
}

#[test]
fn test_lsp_did_open_with_syntax_error() {
    let fixture = LspTestFixture::new();
    fixture.initialize();

    let python_code = r#"def greet(name: str) -> str
    return f"Hello, {name}!""#; // Missing colon

    fixture.did_open("file:///test.py", python_code);

    let diagnostics_response = fixture.wait_for_diagnostics().expect("no diagnostics published");

    assert!(diagnostics_response.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diagnostics_response.contains("BSK-PARSE"));
    assert!(diagnostics_response.contains("Parse error"));
}

#[test]
fn test_lsp_did_change_updates_diagnostics() {
    let fixture = LspTestFixture::new();
    fixture.initialize();

    // Start with code that has errors
    let initial_code = r#"def greet(name):
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", initial_code);

    // Wait for initial diagnostics
    let _ = fixture.wait_for_diagnostics();

    // Send a change that fixes the errors
    let change_request = r#"{
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": {
                "uri": "file:///test.py",
                "version": 2
            },
            "contentChanges": [
                {
                    "text": "def greet(name: str) -> str:\n    return f\"Hello, {name}!\""
                }
            ]
        }
    }"#;

    fixture.send_message(change_request);
    thread::sleep(Duration::from_millis(200));

    let diagnostics_response = fixture.wait_for_diagnostics().expect("no diagnostics published");

    // After fixing, diagnostics should be empty
    assert!(diagnostics_response.contains("\"diagnostics\":[]"));
}

#[test]
fn test_lsp_did_close_clears_diagnostics() {
    let fixture = LspTestFixture::new();
    fixture.initialize();

    let python_code = r#"def greet(name):
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", python_code);

    // Wait for initial diagnostics
    let _ = fixture.wait_for_diagnostics();

    // Send didClose
    let close_request = r#"{
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": {
            "textDocument": {
                "uri": "file:///test.py"
            }
        }
    }"#;

    fixture.send_message(close_request);
    thread::sleep(Duration::from_millis(200));

    let diagnostics_response = fixture.wait_for_diagnostics().expect("no diagnostics published");

    // Closing should clear diagnostics
    assert!(diagnostics_response.contains("\"diagnostics\":[]"));
}

#[test]
fn test_lsp_hover_on_error_location() {
    let fixture = LspTestFixture::new();
    fixture.initialize();

    let python_code = r#"def greet(name):
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", python_code);

    // Wait for diagnostics to be published
    let _ = fixture.wait_for_diagnostics();

    // Send hover request on the parameter that has an error
    let hover_request = r#"{
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/hover",
        "params": {
            "textDocument": {
                "uri": "file:///test.py"
            },
            "position": {
                "line": 0,
                "character": 11
            }
        }
    }"#;

    fixture.send_message(hover_request);
    thread::sleep(Duration::from_millis(200));

    let hover_response = fixture.read_response().expect("no hover response");

    assert!(hover_response.contains("\"jsonrpc\":\"2.0\""));
    assert!(hover_response.contains("\"id\":2"));
    assert!(hover_response.contains("BSK-E0001"));
    assert!(hover_response.contains("Missing parameter type annotation"));
}

#[test]
fn test_lsp_malformed_json_handling() {
    let fixture = LspTestFixture::new();
    fixture.initialize();

    // Send malformed JSON
    fixture.send_message("{ invalid json }");

    let error_response = fixture.read_response().expect("no error response");

    assert!(error_response.contains("\"error\""));
    assert!(error_response.contains("-32700")); // Parse error code
    assert!(error_response.contains("Parse error"));
}

