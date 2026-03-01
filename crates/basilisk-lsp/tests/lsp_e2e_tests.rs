//! End-to-end tests for the Basilisk LSP server.
//!
//! These tests simulate real LSP client interactions and verify
//! the server produces correct responses for various scenarios.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Test fixture for LSP server communication.
struct LspTestFixture {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
}

impl LspTestFixture {
    /// Start the basilisk lsp server as a child process.
    fn new() -> TestResult<Self> {
        let mut child = Command::new("../../target/debug/basilisk")
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or("failed to get stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("failed to get stdout")?;

        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
        })
    }

    /// Send an LSP message to the server.
    fn send_message(&self, message: &str) -> TestResult<()> {
        let content = format!("Content-Length: {}\r\n\r\n{}", message.len(), message);
        let mut stdin = self.stdin.lock().map_err(|e| e.to_string())?;
        stdin.write_all(content.as_bytes())?;
        stdin.flush()?;
        Ok(())
    }

    /// Read the next LSP response from the server.
    fn read_response(&self) -> Option<String> {
        let mut stdout = self.stdout.lock().ok()?;
        let mut line = String::new();

        // Read headers until blank line
        loop {
            line.clear();
            stdout.read_line(&mut line).ok()?;
            if line.trim().is_empty() {
                break;
            }
        }

        // Re-read to find Content-Length (line was cleared in loop)
        // We need to read headers properly — restart with a fresh approach
        // by reading until we get a content-length line
        drop(stdout);

        // Simpler approach: read raw header block then content
        let mut stdout = self.stdout.lock().ok()?;
        let mut header_line = String::new();
        let mut content_length: Option<usize> = None;

        loop {
            header_line.clear();
            stdout.read_line(&mut header_line).ok()?;
            let trimmed = header_line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                content_length = rest.trim().parse().ok();
            }
        }

        let length = content_length?;
        let mut content = vec![0u8; length];
        stdout.read_exact(&mut content).ok()?;
        String::from_utf8(content).ok()
    }

    /// Send initialize request and wait for response.
    fn initialize(&self) -> TestResult<String> {
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

        self.send_message(init_request)?;
        thread::sleep(Duration::from_millis(100));
        self.read_response().ok_or_else(|| "no response to initialize".into())
    }

    /// Send a text document didOpen notification.
    fn did_open(&self, uri: &str, text: &str) -> TestResult<()> {
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

        self.send_message(&did_open)?;
        thread::sleep(Duration::from_millis(200));
        Ok(())
    }

    /// Wait for diagnostics to be published.
    fn wait_for_diagnostics(&self) -> Option<String> {
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
fn test_lsp_initialize() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
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
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let python_code = r#"def greet(name):
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", python_code)?;

    let diagnostics_response = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diagnostics_response.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diagnostics_response.contains("BSK-E0001"));
    assert!(diagnostics_response.contains("BSK-E0002"));
    assert!(diagnostics_response.contains("Missing parameter type annotation"));
    assert!(diagnostics_response.contains("Missing return type annotation"));
    Ok(())
}

#[test]
fn test_lsp_did_open_with_clean_code() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let python_code = r#"def greet(name: str) -> str:
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", python_code)?;

    let diagnostics_response = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diagnostics_response.contains("\"diagnostics\":[]"));
    Ok(())
}

#[test]
fn test_lsp_did_open_with_syntax_error() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let python_code = r#"def greet(name: str) -> str
    return f"Hello, {name}!""#; // Missing colon

    fixture.did_open("file:///test.py", python_code)?;

    let diagnostics_response = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diagnostics_response.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diagnostics_response.contains("BSK-PARSE"));
    assert!(diagnostics_response.contains("Parse error"));
    Ok(())
}

#[test]
fn test_lsp_did_change_updates_diagnostics() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let initial_code = r#"def greet(name):
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", initial_code)?;
    let _ = fixture.wait_for_diagnostics();

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

    fixture.send_message(change_request)?;
    thread::sleep(Duration::from_millis(200));

    let diagnostics_response = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diagnostics_response.contains("\"diagnostics\":[]"));
    Ok(())
}

#[test]
fn test_lsp_did_close_clears_diagnostics() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let python_code = r#"def greet(name):
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", python_code)?;
    let _ = fixture.wait_for_diagnostics();

    let close_request = r#"{
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": {
            "textDocument": {
                "uri": "file:///test.py"
            }
        }
    }"#;

    fixture.send_message(close_request)?;
    thread::sleep(Duration::from_millis(200));

    let diagnostics_response = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diagnostics_response.contains("\"diagnostics\":[]"));
    Ok(())
}

#[test]
fn test_lsp_hover_on_error_location() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let python_code = r#"def greet(name):
    return f"Hello, {name}!""#;

    fixture.did_open("file:///test.py", python_code)?;
    let _ = fixture.wait_for_diagnostics();

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

    fixture.send_message(hover_request)?;
    thread::sleep(Duration::from_millis(200));

    let hover_response = fixture.read_response().ok_or("no hover response")?;

    assert!(hover_response.contains("\"jsonrpc\":\"2.0\""));
    assert!(hover_response.contains("\"id\":2"));
    assert!(hover_response.contains("BSK-E0001"));
    assert!(hover_response.contains("Missing parameter type annotation"));
    Ok(())
}

#[test]
fn test_lsp_malformed_json_handling() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    fixture.send_message("{ invalid json }")?;

    let error_response = fixture.read_response().ok_or("no error response")?;

    assert!(error_response.contains("\"error\""));
    assert!(error_response.contains("-32700"));
    assert!(error_response.contains("Parse error"));
    Ok(())
}

#[test]
fn test_lsp_unknown_method_handling() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let unknown_method = r#"{
        "jsonrpc": "2.0",
        "id": 99,
        "method": "textDocument/unknownMethod",
        "params": {}
    }"#;

    fixture.send_message(unknown_method)?;
    thread::sleep(Duration::from_millis(100));

    let error_response = fixture.read_response().ok_or("no error response")?;

    assert!(error_response.contains("\"error\""));
    assert!(error_response.contains("-32601"));
    Ok(())
}

#[test]
fn test_lsp_concurrent_document_handling() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let code1 = r"def func1(x): pass";
    let code2 = r"def func2(y): return y";

    fixture.did_open("file:///doc1.py", code1)?;
    fixture.did_open("file:///doc2.py", code2)?;

    let response1 = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics for doc1")?;
    let response2 = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics for doc2")?;

    assert!(response1.contains("file:///doc1.py"));
    assert!(response2.contains("file:///doc2.py"));
    assert!(response1.contains("BSK-E0001"));
    assert!(response2.contains("BSK-E0001"));
    Ok(())
}

#[test]
fn test_lsp_large_file_handling() -> TestResult<()> {
    let fixture = LspTestFixture::new()?;
    fixture.initialize()?;

    let mut large_code = String::new();
    for i in 0..50 {
        use std::fmt::Write as _;
        let _ = writeln!(large_code, "def func{i}(x): return x");
    }

    fixture.did_open("file:///large.py", &large_code)?;

    let diagnostics_response = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    assert!(diagnostics_response.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diagnostics_response.matches("BSK-E0001").count() >= 50);
    assert!(diagnostics_response.matches("BSK-E0002").count() >= 50);
    Ok(())
}
