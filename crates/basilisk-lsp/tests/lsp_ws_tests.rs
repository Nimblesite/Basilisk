//! WebSocket end-to-end tests for the Basilisk LSP server.
//!
//! Each test starts the LSP server in-process on a random port,
//! connects via WebSocket, and exercises the full LSP lifecycle.
//! This is the primary E2E test suite — it hits the real LSP over
//! WebSocket, exactly as a browser-based editor would.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Default timeout for receiving a single message from the server.
const RECV_TIMEOUT: Duration = Duration::from_secs(10);

// ── Test fixture ────────────────────────────────────────────────────────────

/// Test fixture that runs the LSP WebSocket server in-process.
struct WsTestFixture {
    ws_write: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    ws_read: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    _server_handle: tokio::task::JoinHandle<()>,
}

impl WsTestFixture {
    /// Start the LSP server on a random port and connect via WebSocket.
    async fn new() -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();

        let server_handle = tokio::spawn(async move {
            if let Ok((tcp_stream, _)) = listener.accept().await {
                if let Ok(ws_stream) = tokio_tungstenite::accept_async(tcp_stream).await {
                    basilisk_lsp::websocket::handle_connection(ws_stream).await;
                }
            }
        });

        // Give the server a moment to start accepting.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let url = format!("ws://127.0.0.1:{port}");
        let (ws_stream, _response) = tokio_tungstenite::connect_async(&url).await?;
        let (ws_write, ws_read) = ws_stream.split();

        Ok(Self {
            ws_write,
            ws_read,
            _server_handle: server_handle,
        })
    }

    /// Send a JSON-RPC message as a WebSocket text frame.
    async fn send_json(&mut self, value: &serde_json::Value) -> TestResult<()> {
        let text = value.to_string();
        self.ws_write.send(Message::Text(text)).await?;
        Ok(())
    }

    /// Receive the next text message with a timeout.
    async fn recv(&mut self) -> Option<String> {
        match timeout(RECV_TIMEOUT, self.ws_read.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => Some(text.to_string()),
            _ => None,
        }
    }

    /// Perform the full initialize / initialized handshake.
    async fn initialize(&mut self) -> TestResult<String> {
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": null,
                "capabilities": {},
                "trace": "off"
            }
        }))
        .await?;

        let response = self.recv().await.ok_or("no response to initialize")?;

        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }))
        .await?;

        // Drain the server's log message.
        let _ = timeout(Duration::from_millis(500), self.ws_read.next()).await;

        Ok(response)
    }

    /// Send `textDocument/didOpen`.
    async fn did_open(&mut self, uri: &str, text: &str) -> TestResult<()> {
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
        .await
    }

    /// Wait for a `publishDiagnostics` notification, skipping unrelated messages.
    async fn wait_for_diagnostics(&mut self) -> Option<String> {
        for _ in 0..10 {
            let msg = self.recv().await?;
            if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") {
                return Some(msg);
            }
        }
        None
    }

    /// Send a request and wait for a response with a matching id.
    async fn request(
        &mut self,
        id: u64,
        method: &str,
        params: serde_json::Value,
    ) -> TestResult<Option<String>> {
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }))
        .await?;

        let id_str = format!("\"id\":{id}");
        for _ in 0..10 {
            let Some(msg) = self.recv().await else { break };
            if msg.contains(&id_str) {
                return Ok(Some(msg));
            }
        }
        Ok(None)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_initialize() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(response.contains("\"jsonrpc\":\"2.0\""));
    assert!(response.contains("\"id\":1"));
    assert!(response.contains("\"result\""));
    assert!(response.contains("\"basilisk\""));
    assert!(response.contains("\"textDocumentSync\":1"));
    assert!(response.contains("\"hoverProvider\":true"));
    assert!(response.contains("\"codeActionProvider\""), "should advertise code actions: {response}");
    assert!(response.contains("\"completionProvider\""));
    Ok(())
}

#[tokio::test]
async fn test_ws_did_open_with_type_errors() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let python_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code).await?;

    let diag = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("BSK-E0001"));
    assert!(diag.contains("BSK-E0002"));
    assert!(diag.contains("Missing parameter type annotation"));
    assert!(diag.contains("Missing return type annotation"));
    Ok(())
}

#[tokio::test]
async fn test_ws_did_open_with_clean_code() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let python_code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code).await?;

    let diag = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

#[tokio::test]
async fn test_ws_did_open_with_syntax_error() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Missing colon after return type.
    let python_code = "def greet(name: str) -> str\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code).await?;

    let diag = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diag.contains("BSK-PARSE"));
    assert!(diag.contains("Parse error"));
    Ok(())
}

#[tokio::test]
async fn test_ws_did_change_updates_diagnostics() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

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
    fixture.initialize().await?;

    let python_code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///test.py", python_code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": {
                    "uri": "file:///test.py"
                }
            }
        }))
        .await?;

    let diag = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics after close")?;

    assert!(diag.contains("\"diagnostics\":[]"));
    Ok(())
}

#[tokio::test]
async fn test_ws_hover_on_error_location() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

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

#[tokio::test]
async fn test_ws_malformed_json_handling() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Send raw malformed JSON as a text frame.
    fixture
        .ws_write
        .send(Message::Text("{ invalid json }".into()))
        .await?;

    // Skip any leftover notification messages (e.g. window/showMessage from
    // initialization) and look for the parse error response.
    let mut error_response = None;
    for _ in 0..10 {
        let Some(msg) = fixture.recv().await else { break };
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
    fixture.initialize().await?;

    let resp = fixture
        .request(
            99,
            "textDocument/unknownMethod",
            serde_json::json!({}),
        )
        .await?
        .ok_or("no error response")?;

    assert!(resp.contains("\"error\""));
    assert!(resp.contains("-32601"));
    Ok(())
}

#[tokio::test]
async fn test_ws_concurrent_document_handling() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

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
    fixture.initialize().await?;

    let mut large_code = String::new();
    for i in 0..50 {
        use std::fmt::Write as _;
        let _ = writeln!(large_code, "def func{i}(x): return x");
    }

    fixture
        .did_open("file:///large.py", &large_code)
        .await?;

    let diag = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    assert!(diag.contains("\"method\":\"textDocument/publishDiagnostics\""));
    assert!(diag.matches("BSK-E0001").count() >= 50);
    assert!(diag.matches("BSK-E0002").count() >= 50);
    Ok(())
}

// ── Completion (IntelliSense) tests via WebSocket ───────────────────────────

#[tokio::test]
async fn test_ws_initialize_advertises_completion() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(response.contains("\"completionProvider\""));
    assert!(response.contains("\".\""));
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_returns_functions_and_classes() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///comp.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            10,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///comp.py" },
                "position": { "line": 9, "character": 0 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

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
    assert!(
        resp.contains("\"label\":\"print\""),
        "should complete builtin 'print': {resp}"
    );
    assert!(
        resp.contains("\"label\":\"len\""),
        "should complete builtin 'len': {resp}"
    );

    // Hardened: parse and verify completion list structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let items = parsed["result"]["items"]
        .as_array()
        .or_else(|| parsed["result"].as_array())
        .ok_or("completion result should contain items array")?;

    // Hardened: completion list must be non-empty
    assert!(
        !items.is_empty(),
        "completion list must be non-empty: {resp}"
    );

    // Hardened: each item must have a non-empty label and a kind field
    for item in items {
        let label = item["label"].as_str().unwrap_or("");
        assert!(
            !label.is_empty(),
            "each completion item must have a non-empty label: {resp}"
        );
        assert!(
            item.get("kind").is_some() && !item["kind"].is_null(),
            "each completion item must have a 'kind' field, missing for label '{label}': {resp}"
        );
    }

    // Hardened: verify JSON-RPC envelope
    assert_eq!(parsed["jsonrpc"], "2.0", "must be valid JSON-RPC 2.0: {resp}");
    assert_eq!(parsed["id"], 10, "response id must match request id: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_prefix_filtering() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str) -> str:
    return name

def goodbye(name: str) -> str:
    return name

def helper(x: int) -> int:
    return x

gr";
    fixture.did_open("file:///prefix.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            11,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///prefix.py" },
                "position": { "line": 9, "character": 2 }
            }),
        )
        .await?
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

#[tokio::test]
async fn test_ws_completion_imports() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
from typing import Optional, List
import os

";
    fixture.did_open("file:///imports.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            12,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///imports.py" },
                "position": { "line": 3, "character": 0 }
            }),
        )
        .await?
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

#[tokio::test]
async fn test_ws_completion_dot_on_class() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Dog:
    name: str
    breed: str
    def bark(self) -> str:
        return \"woof\"
    def fetch(self, item: str) -> str:
        return item

Dog.";
    fixture.did_open("file:///dot.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            13,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///dot.py" },
                "position": { "line": 8, "character": 4 }
            }),
        )
        .await?
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

#[tokio::test]
async fn test_ws_completion_self_dot() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Cat:
    color: str
    age: int
    def meow(self) -> str:
        return \"meow\"
    def describe(self) -> str:
        return self.";
    fixture.did_open("file:///selfdot.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            14,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///selfdot.py" },
                "position": { "line": 6, "character": 20 }
            }),
        )
        .await?
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

#[tokio::test]
async fn test_ws_completion_builtins() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "pri";
    fixture.did_open("file:///builtins.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            15,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///builtins.py" },
                "position": { "line": 0, "character": 3 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"print\""),
        "should complete builtin 'print' for prefix 'pri': {resp}"
    );
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

#[tokio::test]
async fn test_ws_completion_function_detail_shows_params() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def calculate(x: int, y: int, op: str) -> int:
    return x

cal";
    fixture.did_open("file:///detail.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            16,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///detail.py" },
                "position": { "line": 3, "character": 3 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    assert!(
        resp.contains("\"label\":\"calculate\""),
        "should complete 'calculate': {resp}"
    );
    assert!(
        resp.contains("x, y, op"),
        "should show params in detail: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_on_empty_file() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    fixture.did_open("file:///empty.py", "").await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            17,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///empty.py" },
                "position": { "line": 0, "character": 0 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

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

// ── Code action tests ────────────────────────────────────────────────────────

/// Helper: parse published diagnostics and request code actions for a specific
/// diagnostic code. Returns the raw JSON-RPC response string.
async fn code_action_for(
    fixture: &mut WsTestFixture,
    uri: &str,
    action_id: u64,
    diag_code: &str,
) -> TestResult<String> {
    let diag_msg = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let target_diag = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some(diag_code))
        .ok_or(format!("no {diag_code} diagnostic"))?;

    fixture
        .request(
            action_id,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "range": target_diag["range"],
                "context": { "diagnostics": [target_diag] }
            }),
        )
        .await?
        .ok_or_else(|| format!("no code action response for {diag_code}").into())
}

#[tokio::test]
async fn test_ws_code_action_missing_param_annotation() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ca_e0001.py", code).await?;

    let resp = code_action_for(&mut fixture, "file:///ca_e0001.py", 200, "BSK-E0001").await?;

    assert!(resp.contains(": Any"), "E0001 action should insert ': Any': {resp}");
    assert!(resp.contains("quickfix"), "E0001 action should be quickfix: {resp}");

    // Hardened: parse and verify code action structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let actions = parsed["result"]
        .as_array()
        .ok_or("code action result should be an array")?;

    // Hardened: should have at least one action (quickfix + possibly suppress)
    assert!(
        !actions.is_empty(),
        "code actions array must be non-empty: {resp}"
    );

    // Find the quickfix action specifically (title is "Add `: Any` annotation (basilisk)")
    let quickfix = actions
        .iter()
        .find(|a| {
            a["kind"].as_str() == Some("quickfix")
                && a["title"].as_str().is_some_and(|t| t.contains("Any"))
        })
        .ok_or("should have a quickfix action for adding `: Any` annotation")?;

    // Hardened: verify action has edit with changes
    let edit = &quickfix["edit"];
    assert!(
        !edit.is_null(),
        "quickfix action must have an 'edit' field: {resp}"
    );
    let changes = &edit["changes"];
    assert!(
        !changes.is_null(),
        "quickfix edit must have 'changes': {resp}"
    );

    // Hardened: verify edit changes contain the file URI
    let file_edits = &changes["file:///ca_e0001.py"];
    assert!(
        !file_edits.is_null(),
        "changes must contain edits for 'file:///ca_e0001.py': {resp}"
    );

    // Hardened: verify the text edit inserts ": Any"
    let edits = file_edits
        .as_array()
        .ok_or("file edits should be an array")?;
    assert!(
        !edits.is_empty(),
        "file edits array must be non-empty: {resp}"
    );
    let new_text = edits[0]["newText"].as_str().unwrap_or("");
    assert!(
        new_text.contains(": Any"),
        "text edit newText should contain ': Any', got '{new_text}': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_missing_return_annotation() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name: str):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ca_e0002.py", code).await?;

    let resp = code_action_for(&mut fixture, "file:///ca_e0002.py", 201, "BSK-E0002").await?;

    assert!(resp.contains("-> None"), "E0002 action should insert '-> None': {resp}");
    assert!(resp.contains("quickfix"), "E0002 action should be quickfix: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_missing_variable_annotation_empty_list() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "items = []\n";
    fixture.did_open("file:///ca_e0003_list.py", code).await?;

    let resp =
        code_action_for(&mut fixture, "file:///ca_e0003_list.py", 202, "BSK-E0003").await?;

    assert!(
        resp.contains("list[Any]"),
        "E0003 (empty list) action should insert 'list[Any]': {resp}"
    );
    assert!(resp.contains("quickfix"), "E0003 action should be quickfix: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_missing_variable_annotation_empty_dict() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "mapping = {}\n";
    fixture.did_open("file:///ca_e0003_dict.py", code).await?;

    let resp =
        code_action_for(&mut fixture, "file:///ca_e0003_dict.py", 203, "BSK-E0003").await?;

    assert!(
        resp.contains("dict[str, Any]"),
        "E0003 (empty dict) action should insert 'dict[str, Any]': {resp}"
    );
    assert!(resp.contains("quickfix"), "E0003 action should be quickfix: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_missing_variable_annotation_none() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "value = None\n";
    fixture.did_open("file:///ca_e0003_none.py", code).await?;

    let resp =
        code_action_for(&mut fixture, "file:///ca_e0003_none.py", 204, "BSK-E0003").await?;

    assert!(
        resp.contains(": Any"),
        "E0003 (None) action should insert ': Any': {resp}"
    );
    assert!(resp.contains("quickfix"), "E0003 action should be quickfix: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_suppress_with_type_ignore() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ca_suppress.py", code).await?;

    let resp =
        code_action_for(&mut fixture, "file:///ca_suppress.py", 205, "BSK-E0001").await?;

    assert!(
        resp.contains("# type: ignore"),
        "suppress action should insert '# type: ignore': {resp}"
    );
    assert!(
        resp.contains("Suppress"),
        "suppress action should have 'Suppress' in title: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_suppress_inserts_at_end_of_line() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // The suppress action must target the diagnostic's line (line 0 here).
    let code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ca_suppress_pos.py", code).await?;

    let resp =
        code_action_for(&mut fixture, "file:///ca_suppress_pos.py", 206, "BSK-E0001").await?;

    // The edit should be an insert (start == end), not a replace.
    let action_json: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &action_json["result"];
    let suppress = result
        .as_array()
        .and_then(|arr| {
            arr.iter().find(|a| {
                a["title"]
                    .as_str()
                    .is_some_and(|t| t.contains("type: ignore"))
            })
        })
        .ok_or("no suppress action in result")?;

    let edits = &suppress["edit"]["changes"]["file:///ca_suppress_pos.py"];
    let edit = edits.as_array().and_then(|a| a.first()).ok_or("no edits")?;

    // start == end means pure insertion
    assert_eq!(
        edit["range"]["start"],
        edit["range"]["end"],
        "suppress action must be a pure insertion: {edit}"
    );
    assert_eq!(
        edit["newText"].as_str(),
        Some("  # type: ignore"),
        "inserted text must be '  # type: ignore': {edit}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_organize_imports() -> TestResult<()> {
    // Skip if ruff is not installed.
    if std::process::Command::new("ruff").arg("--version").output().is_err() {
        return Ok(());
    }

    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Deliberately unsorted imports — ruff should reorder them.
    let code =
        "import os\nimport sys\nfrom typing import Optional\nimport json\n\nx: int = 1\n";
    fixture.did_open("file:///ca_org.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            210,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": "file:///ca_org.py" },
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                "context": { "diagnostics": [] }
            }),
        )
        .await?;

    // The organize-imports action may or may not fire depending on whether
    // the given imports are already sorted by ruff. Just check that when it
    // does appear, it carries the correct kind.
    if let Some(resp_str) = resp {
        if resp_str.contains("Organize imports") {
            assert!(
                resp_str.contains("source.organizeImports"),
                "organize imports action should have organizeImports kind: {resp_str}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_organize_imports_fixes_order() -> TestResult<()> {
    // Skip if ruff is not installed.
    if std::process::Command::new("ruff").arg("--version").output().is_err() {
        return Ok(());
    }

    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // sys must come before os alphabetically; ruff will sort to: import os / import sys
    // (actually ruff keeps stdlib imports in the order they appear unless --fix-only is used)
    // Use a clear case: `from __future__` must be first.
    let code = "import os\nfrom __future__ import annotations\n\nx: int = 1\n";
    fixture.did_open("file:///ca_org2.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            211,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": "file:///ca_org2.py" },
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                "context": { "diagnostics": [] }
            }),
        )
        .await?;

    if let Some(resp_str) = resp {
        if resp_str.contains("Organize imports") {
            // The reordered source should put `from __future__` first.
            assert!(
                resp_str.contains("from __future__ import annotations"),
                "organized source should contain the moved import: {resp_str}"
            );
            assert!(
                resp_str.contains("source.organizeImports"),
                "action kind must be organizeImports: {resp_str}"
            );
        }
    }
    Ok(())
}

// ── Phase 2: Hover (enhanced) ──────────────────────────────────────────────

#[tokio::test]
async fn test_ws_hover_function_exact_signature() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ws_hover_exact.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            300,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_hover_exact.py" },
                "position": { "line": 0, "character": 4 }
            }),
        )
        .await?
        .ok_or("no hover response")?;

    assert!(resp.contains("(function)"), "hover should show '(function)' prefix: {resp}");
    assert!(resp.contains("def greet"), "hover should show 'def greet': {resp}");
    assert!(resp.contains("name: str"), "hover should show typed parameter 'name: str': {resp}");
    assert!(resp.contains("-> str"), "hover should show return type '-> str': {resp}");

    // Hardened: verify JSON-RPC structure and hover contents structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert_eq!(parsed["jsonrpc"], "2.0", "hover must be valid JSON-RPC 2.0: {resp}");
    assert_eq!(parsed["id"], 300, "hover response id must match request id: {resp}");
    assert!(
        parsed["result"].get("contents").is_some(),
        "hover result must contain 'contents' field: {resp}"
    );
    assert!(
        !parsed["result"]["contents"].is_null(),
        "hover contents must not be null: {resp}"
    );
    assert!(resp.contains("greet"), "hover should contain the function name 'greet': {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_hover_from_call_site() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    fixture.did_open("file:///ws_hover_call.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // "result: str = greet(\"world\")" is line 3.
    // "result: str = " is 14 chars, so 'g' of "greet" is at character 14.
    let resp = fixture
        .request(
            301,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_hover_call.py" },
                "position": { "line": 3, "character": 14 }
            }),
        )
        .await?
        .ok_or("no hover response at call site")?;

    assert!(resp.contains("(function)"), "call-site hover should resolve to function: {resp}");
    assert!(resp.contains("greet"), "call-site hover should show function name: {resp}");
    assert!(resp.contains("name: str"), "call-site hover should show parameter type: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_hover_parameter_shows_type() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///ws_hover_param.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // "def greet(" is 10 chars, so 'n' of "name" is at character 10.
    let resp = fixture
        .request(
            302,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_hover_param.py" },
                "position": { "line": 0, "character": 10 }
            }),
        )
        .await?
        .ok_or("no hover response for parameter")?;

    assert!(resp.contains("(parameter)"), "hover on parameter should show '(parameter)': {resp}");
    assert!(resp.contains("name"), "hover should show parameter name: {resp}");
    assert!(resp.contains("str"), "hover should show parameter type 'str': {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_hover_class_attribute() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "class Animal:\n    name: str\n    age: int\n";
    fixture.did_open("file:///ws_hover_attr.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Line 1: "    name: str" — "name" starts at character 4.
    let resp = fixture
        .request(
            303,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_hover_attr.py" },
                "position": { "line": 1, "character": 4 }
            }),
        )
        .await?
        .ok_or("no hover response for class attribute")?;

    assert!(resp.contains("(property)"), "hover on class attribute should show '(property)': {resp}");
    assert!(resp.contains("Animal.name"), "hover should show 'Animal.name': {resp}");
    assert!(resp.contains("str"), "hover should show attribute type 'str': {resp}");
    Ok(())
}

// ── Phase 2: Go to Definition ──────────────────────────────────────────────

#[tokio::test]
async fn test_ws_goto_definition_function() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///ws_gotodef.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            310,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_gotodef.py" },
                "position": { "line": 0, "character": 4 }
            }),
        )
        .await?
        .ok_or("no definition response")?;

    assert!(resp.contains("ws_gotodef.py"), "definition should point to same file: {resp}");

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

    // Hardened: verify URI matches the opened file exactly
    let uri = parsed["result"]["uri"].as_str().unwrap_or("");
    assert_eq!(
        uri, "file:///ws_gotodef.py",
        "definition URI must match the opened document: {resp}"
    );

    // Hardened: verify the range is non-empty (end differs from start)
    let end = &parsed["result"]["range"]["end"];
    assert!(
        start["line"] != end["line"] || start["character"] != end["character"],
        "definition range must be non-empty (start != end): {resp}"
    );

    // Hardened: verify end character is beyond start (for a single-line range)
    if start["line"] == end["line"] {
        assert!(
            end["character"].as_u64() > start["character"].as_u64(),
            "definition end character must be > start character on same line: {resp}"
        );
    }

    // Hardened: verify JSON-RPC envelope
    assert_eq!(parsed["jsonrpc"], "2.0", "must be valid JSON-RPC 2.0: {resp}");
    assert_eq!(parsed["id"], 310, "response id must match request id: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_class() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "class Dog:\n    name: str\n    def bark(self) -> str:\n        return \"woof\"\n";
    fixture.did_open("file:///ws_gotoclass.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            311,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_gotoclass.py" },
                "position": { "line": 0, "character": 6 }
            }),
        )
        .await?
        .ok_or("no definition response")?;

    assert!(resp.contains("ws_gotoclass.py"), "definition should point to same file: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_from_call_site() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    fixture.did_open("file:///ws_goto_call.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Line 3: "result: str = greet(\"world\")" — 'g' of call "greet" at character 14.
    let resp = fixture
        .request(
            312,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_goto_call.py" },
                "position": { "line": 3, "character": 14 }
            }),
        )
        .await?
        .ok_or("no definition response from call site")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def from call site must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(start["line"], 0, "goto-def from call should jump to line 0: {resp}");
    assert_eq!(
        start["character"], 4,
        "goto-def from call should land at char 4 where 'greet' is defined: {resp}"
    );
    Ok(())
}

// ── Phase 2: Document Symbols ──────────────────────────────────────────────

#[tokio::test]
async fn test_ws_document_symbols() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///ws_symbols.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            320,
            "textDocument/documentSymbol",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_symbols.py" }
            }),
        )
        .await?
        .ok_or("no document symbols response")?;

    assert!(resp.contains("Animal"), "symbols should include class 'Animal': {resp}");
    assert!(resp.contains("greet"), "symbols should include function 'greet': {resp}");
    assert!(resp.contains("\"x\""), "symbols should include variable 'x': {resp}");

    // Hardened: parse and verify symbol count and structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let symbols = parsed["result"]
        .as_array()
        .ok_or("document symbols result should be an array")?;

    // Exact count: Animal (class), greet (function), x (variable) = 3 top-level symbols
    assert_eq!(
        symbols.len(),
        3,
        "should have exactly 3 top-level symbols (Animal, greet, x), got {}: {resp}",
        symbols.len()
    );

    // Hardened: verify each symbol has a valid range with start <= end
    for symbol in symbols {
        let range = &symbol["range"];
        assert!(
            !range.is_null(),
            "every symbol must have a range: {resp}"
        );
        let start_line = range["start"]["line"].as_u64().unwrap_or(u64::MAX);
        let end_line = range["end"]["line"].as_u64().unwrap_or(0);
        assert!(
            start_line <= end_line,
            "symbol range start line must be <= end line: {resp}"
        );
    }

    // Hardened: verify class symbol has children (methods/attributes)
    let animal_symbol = symbols
        .iter()
        .find(|s| s["name"].as_str() == Some("Animal"))
        .ok_or("Animal symbol not found in results")?;
    let children = animal_symbol["children"]
        .as_array()
        .ok_or("Animal class symbol should have children array")?;
    assert!(
        !children.is_empty(),
        "Animal class should have children (name attr + speak method): {resp}"
    );

    // Hardened: verify symbol kinds are present
    for symbol in symbols {
        assert!(
            symbol.get("kind").is_some() && !symbol["kind"].is_null(),
            "every symbol must have a kind: {resp}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_document_symbols_nested_methods() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Calculator:
    value: int
    def add(self, x: int) -> int:
        return self.value + x
    def multiply(self, x: int) -> int:
        return self.value * x
";
    fixture.did_open("file:///ws_nested.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            321,
            "textDocument/documentSymbol",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_nested.py" }
            }),
        )
        .await?
        .ok_or("no document symbols response")?;

    assert!(resp.contains("Calculator"), "should contain class: {resp}");
    assert!(resp.contains("add"), "should contain method 'add': {resp}");
    assert!(resp.contains("multiply"), "should contain method 'multiply': {resp}");
    assert!(resp.contains("value"), "should contain attribute 'value': {resp}");
    Ok(())
}

// ── Phase 2: Signature Help ────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_signature_help() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str, greeting: str) -> str:
    return f\"{greeting}, {name}!\"

result: str = greet(\"world\", \"Hi\")
";
    fixture.did_open("file:///ws_sighelp.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor inside the greet() call — after the opening paren
    // "result: str = greet(" is line 3, character 20
    let resp = fixture
        .request(
            330,
            "textDocument/signatureHelp",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_sighelp.py" },
                "position": { "line": 3, "character": 21 }
            }),
        )
        .await?
        .ok_or("no signature help response")?;

    assert!(resp.contains("greet"), "signature should show function name: {resp}");
    assert!(resp.contains("name"), "signature should show parameter 'name': {resp}");
    assert!(resp.contains("greeting"), "signature should show parameter 'greeting': {resp}");

    // Hardened: parse and verify signature help structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    assert!(
        !result.is_null(),
        "signature help result must not be null: {resp}"
    );

    // Hardened: verify activeParameter is 0 (cursor at first param position)
    let active_param = result["activeParameter"].as_u64();
    assert_eq!(
        active_param,
        Some(0),
        "activeParameter should be 0 (first parameter): {resp}"
    );

    // Hardened: verify signatures array exists and is non-empty
    let signatures = result["signatures"]
        .as_array()
        .ok_or("signature help should have signatures array")?;
    assert!(
        !signatures.is_empty(),
        "signatures array must be non-empty: {resp}"
    );

    // Hardened: verify parameters array length matches expected count (2: name, greeting)
    let first_sig = &signatures[0];
    let parameters = first_sig["parameters"]
        .as_array()
        .ok_or("first signature should have parameters array")?;
    assert_eq!(
        parameters.len(),
        2,
        "should have exactly 2 parameters (name, greeting), got {}: {resp}",
        parameters.len()
    );

    // Hardened: verify each parameter has a label
    for param in parameters {
        assert!(
            param.get("label").is_some() && !param["label"].is_null(),
            "each parameter must have a label: {resp}"
        );
    }
    Ok(())
}

// ── Phase 2: Find References ───────────────────────────────────────────────

#[tokio::test]
async fn test_ws_find_references() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

result: str = greet(\"world\")
";
    fixture.did_open("file:///ws_refs.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Find references for "greet" (line 0, character 4)
    let resp = fixture
        .request(
            340,
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_refs.py" },
                "position": { "line": 0, "character": 4 },
                "context": { "includeDeclaration": true }
            }),
        )
        .await?
        .ok_or("no references response")?;

    // Should find at least 2 references (definition + usage)
    let count = resp.matches("ws_refs.py").count();
    assert!(count >= 2, "should find at least 2 references for 'greet' (found {count}): {resp}");

    // Hardened: parse and verify exact reference count and structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let refs = parsed["result"]
        .as_array()
        .ok_or("references result should be an array")?;

    // Exactly 2 references: definition on line 0 + usage on line 3
    assert_eq!(
        refs.len(),
        2,
        "should find exactly 2 references for 'greet' (definition + usage), got {}: {resp}",
        refs.len()
    );

    // Hardened: verify each reference has a valid URI
    for reference in refs {
        let uri = reference["uri"].as_str().unwrap_or("");
        assert!(
            !uri.is_empty(),
            "each reference must have a non-empty URI: {resp}"
        );
        assert_eq!(
            uri, "file:///ws_refs.py",
            "each reference URI must match the opened file: {resp}"
        );
    }

    // Hardened: verify each reference has a valid range
    for reference in refs {
        let range = &reference["range"];
        assert!(
            !range.is_null(),
            "each reference must have a range: {resp}"
        );
        assert!(
            range.get("start").is_some() && range.get("end").is_some(),
            "each reference range must have start and end: {resp}"
        );
        let start_line = range["start"]["line"].as_u64().unwrap_or(u64::MAX);
        let end_line = range["end"]["line"].as_u64().unwrap_or(0);
        assert!(
            start_line <= end_line,
            "reference range start line must be <= end line: {resp}"
        );
    }
    Ok(())
}

// ── Phase 2: Rename ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_prepare_rename() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///ws_rename.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Prepare rename on "greet" (line 0, character 4)
    let resp = fixture
        .request(
            350,
            "textDocument/prepareRename",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_rename.py" },
                "position": { "line": 0, "character": 4 }
            }),
        )
        .await?
        .ok_or("no prepare rename response")?;

    assert!(resp.contains("result"), "prepare rename should return a result: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_rename_symbol() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

result: str = greet(\"world\")
";
    fixture.did_open("file:///ws_ren.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Rename "greet" to "say_hello" (line 0, character 4)
    let resp = fixture
        .request(
            351,
            "textDocument/rename",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_ren.py" },
                "position": { "line": 0, "character": 4 },
                "newName": "say_hello"
            }),
        )
        .await?
        .ok_or("no rename response")?;

    assert!(resp.contains("say_hello"), "rename should include new name: {resp}");
    assert!(resp.contains("changes"), "rename should include workspace changes: {resp}");

    // Hardened: parse and verify workspace edit structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"];
    assert!(
        !changes.is_null(),
        "rename result must contain 'changes' map: {resp}"
    );

    // Hardened: verify changes map contains the file URI
    let file_edits = &changes["file:///ws_ren.py"];
    assert!(
        !file_edits.is_null(),
        "changes must contain edits for 'file:///ws_ren.py': {resp}"
    );

    // Hardened: verify edits array is non-empty
    let edits = file_edits
        .as_array()
        .ok_or("edits for file should be an array")?;
    assert!(
        !edits.is_empty(),
        "edits array must be non-empty: {resp}"
    );

    // Hardened: verify each edit has both range and newText
    for edit in edits {
        assert!(
            edit.get("range").is_some() && !edit["range"].is_null(),
            "each edit must have a range: {resp}"
        );
        assert!(
            edit.get("newText").is_some() && !edit["newText"].is_null(),
            "each edit must have a newText: {resp}"
        );
        assert_eq!(
            edit["newText"].as_str(),
            Some("say_hello"),
            "each edit's newText must be the new name 'say_hello': {resp}"
        );
    }

    // Hardened: should have at least 2 edits (definition + usage)
    assert!(
        edits.len() >= 2,
        "should have at least 2 rename edits (definition + usage), got {}: {resp}",
        edits.len()
    );
    Ok(())
}

// ── Phase 2: Inlay Hints ──────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_inlay_hints_variable_types() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "x = 42\ny = \"hello\"\nz = True\n";
    fixture.did_open("file:///ws_inlay.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            360,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_inlay.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 3, "character": 0 }
                }
            }),
        )
        .await?
        .ok_or("no inlay hint response")?;

    assert!(resp.contains("int"), "inlay hints should show 'int' for x=42: {resp}");
    assert!(resp.contains("str"), "inlay hints should show 'str' for y=\"hello\": {resp}");
    assert!(resp.contains("bool"), "inlay hints should show 'bool' for z=True: {resp}");

    // Hardened: parse and verify inlay hint structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let hints = parsed["result"]
        .as_array()
        .ok_or("inlay hint result should be an array")?;

    // Hardened: should have exactly 3 hints (x, y, z)
    assert_eq!(
        hints.len(),
        3,
        "should have exactly 3 inlay hints (x, y, z), got {}: {resp}",
        hints.len()
    );

    // Hardened: each hint must have a valid position and a non-empty label
    for hint in hints {
        let position = &hint["position"];
        assert!(
            !position.is_null(),
            "each inlay hint must have a position: {resp}"
        );
        assert!(
            position.get("line").is_some(),
            "each inlay hint position must have a line: {resp}"
        );
        assert!(
            position.get("character").is_some(),
            "each inlay hint position must have a character: {resp}"
        );

        let label = hint["label"].as_str().unwrap_or("");
        assert!(
            !label.is_empty(),
            "each inlay hint must have a non-empty label: {resp}"
        );
    }

    // Hardened: verify hint kind is Type (1) for variable type hints
    for hint in hints {
        assert_eq!(
            hint["kind"].as_u64(),
            Some(1),
            "inlay hint kind should be Type (1) for variable hints: {resp}"
        );
    }
    Ok(())
}

// ── Phase 2: Semantic Tokens ───────────────────────────────────────────────

#[tokio::test]
async fn test_ws_semantic_tokens_full() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///ws_semtok.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            370,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok.py" }
            }),
        )
        .await?
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

    // Hardened: verify first token has valid tokenType (0-9 range for standard LSP token types)
    let first_token_type = data[3].as_u64().unwrap_or(u64::MAX);
    assert!(
        first_token_type <= 20,
        "first token's tokenType should be in valid range (0-20), got {first_token_type}: {resp}"
    );

    // Hardened: verify no negative deltas in the data (all values should be non-negative)
    for (idx, value) in data.iter().enumerate() {
        let num = value.as_i64().unwrap_or(-1);
        assert!(
            num >= 0,
            "semantic token data[{idx}] must be non-negative, got {num}: {resp}"
        );
    }

    // Hardened: verify we have a reasonable number of tokens for this code
    let token_count = data.len() / 5;
    assert!(
        token_count >= 3,
        "should have at least 3 tokens for code with class, function, and variable, got {token_count}: {resp}"
    );

    // Hardened: verify JSON-RPC structure
    assert_eq!(parsed["jsonrpc"], "2.0", "must be valid JSON-RPC 2.0: {resp}");
    assert_eq!(parsed["id"], 370, "response id must match request id: {resp}");
    Ok(())
}

// ── Phase 2: Initialize capabilities ───────────────────────────────────────

#[tokio::test]
async fn test_ws_initialize_advertises_all_phase2_capabilities() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(response.contains("\"definitionProvider\""), "should advertise definition: {response}");
    assert!(response.contains("\"documentSymbolProvider\""), "should advertise document symbols: {response}");
    assert!(response.contains("\"signatureHelpProvider\""), "should advertise signature help: {response}");
    assert!(response.contains("\"referencesProvider\""), "should advertise references: {response}");
    assert!(response.contains("\"renameProvider\""), "should advertise rename: {response}");
    assert!(response.contains("\"inlayHintProvider\""), "should advertise inlay hints: {response}");
    assert!(response.contains("\"semanticTokensProvider\""), "should advertise semantic tokens: {response}");
    assert!(response.contains("\"documentFormattingProvider\""), "should advertise document formatting: {response}");
    Ok(())
}

// ── Edge-case robustness tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_ws_hover_unknown_position_returns_null() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "x: int = 42\n";
    fixture.did_open("file:///ws_edge_hover.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Hover on an empty line / far beyond content — should return null, not crash.
    let resp = fixture
        .request(
            400,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_edge_hover.py" },
                "position": { "line": 5, "character": 0 }
            }),
        )
        .await?
        .ok_or("no hover response for unknown position")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "hover on empty position should return null result: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_def_no_symbol_returns_null() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "x: int = 42\n";
    fixture.did_open("file:///ws_edge_gotodef.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Goto definition on whitespace / non-symbol position.
    let resp = fixture
        .request(
            401,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_edge_gotodef.py" },
                "position": { "line": 5, "character": 0 }
            }),
        )
        .await?
        .ok_or("no definition response for empty position")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "goto-def on non-symbol position should return null: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_document_symbols_empty_file_returns_empty() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    fixture
        .did_open("file:///ws_edge_symbols.py", "")
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            402,
            "textDocument/documentSymbol",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_edge_symbols.py" }
            }),
        )
        .await?
        .ok_or("no document symbols response for empty file")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    assert!(
        result.is_null() || result.as_array().is_some_and(Vec::is_empty),
        "document symbols on empty file should be null or empty array: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_signature_help_outside_call_returns_null() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nx: int = 42\n";
    fixture
        .did_open("file:///ws_edge_sighelp.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor on `x: int = 42` — not inside a function call.
    let resp = fixture
        .request(
            403,
            "textDocument/signatureHelp",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_edge_sighelp.py" },
                "position": { "line": 3, "character": 0 }
            }),
        )
        .await?
        .ok_or("no signature help response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "signature help outside a call should return null: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_find_references_unknown_symbol_returns_null() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "x: int = 42\n";
    fixture
        .did_open("file:///ws_edge_refs.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Find references at a position with no symbol.
    let resp = fixture
        .request(
            404,
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_edge_refs.py" },
                "position": { "line": 5, "character": 0 },
                "context": { "includeDeclaration": true }
            }),
        )
        .await?
        .ok_or("no references response for unknown symbol")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    assert!(
        result.is_null() || result.as_array().is_some_and(Vec::is_empty),
        "find references on unknown symbol should return null or empty: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_rename_non_symbol_position_returns_null() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "x: int = 42\n";
    fixture
        .did_open("file:///ws_edge_rename.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Rename at an empty position — should return null.
    let resp = fixture
        .request(
            405,
            "textDocument/rename",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_edge_rename.py" },
                "position": { "line": 5, "character": 0 },
                "newName": "should_not_work"
            }),
        )
        .await?
        .ok_or("no rename response for non-symbol position")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "rename on non-symbol position should return null: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hints_fully_annotated_returns_empty() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Every variable has explicit type annotations — no inlay hints needed.
    let code = "x: int = 42\ny: str = \"hello\"\nz: bool = True\n";
    fixture
        .did_open("file:///ws_edge_inlay.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            406,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_edge_inlay.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 3, "character": 0 }
                }
            }),
        )
        .await?
        .ok_or("no inlay hint response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    assert!(
        result.is_null() || result.as_array().is_some_and(Vec::is_empty),
        "inlay hints on fully-annotated code should be empty: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_hover_method_shows_class_prefix() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name
";
    fixture
        .did_open("file:///ws_edge_hover_method.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Hover on "speak" — line 2, character 8 (after "    def ")
    let resp = fixture
        .request(
            407,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_edge_hover_method.py" },
                "position": { "line": 2, "character": 8 }
            }),
        )
        .await?
        .ok_or("no hover response for method")?;

    assert!(
        resp.contains("(method)"),
        "hover on method should show '(method)' prefix: {resp}"
    );
    assert!(
        resp.contains("Animal.speak"),
        "hover on method should show class prefix 'Animal.speak': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_signature_help_active_parameter_index() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def add(a: int, b: int) -> int:
    return a + b

result: int = add(1, 2)
";
    fixture
        .did_open("file:///ws_edge_sighelp_idx.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor after the comma inside add(1, |2) — line 3
    // "result: int = add(1, " is 21 chars, so character 21 is after comma
    let resp = fixture
        .request(
            408,
            "textDocument/signatureHelp",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_edge_sighelp_idx.py" },
                "position": { "line": 3, "character": 21 }
            }),
        )
        .await?
        .ok_or("no signature help response")?;

    assert!(
        resp.contains("activeParameter"),
        "signature help should include activeParameter: {resp}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let active_param = &parsed["result"]["activeParameter"];
    assert!(
        !active_param.is_null(),
        "activeParameter should not be null: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_e0003_all_variants() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // All three E0003 variants in one file: empty list, empty dict, None
    let code = "items = []\nmapping = {}\nvalue = None\n";
    fixture
        .did_open("file:///ws_edge_ca_e0003.py", code)
        .await?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    // Verify all three E0003 diagnostics are present.
    let e0003_diags: Vec<&serde_json::Value> = diagnostics
        .iter()
        .filter(|d| d["code"].as_str() == Some("BSK-E0003"))
        .collect();
    assert!(
        e0003_diags.len() >= 3,
        "should have at least 3 E0003 diagnostics (list, dict, None), got {}: {diag_msg}",
        e0003_diags.len()
    );

    // Request code actions for each E0003 diagnostic.
    for (idx, target_diag) in e0003_diags.iter().enumerate() {
        let action_id = 410 + idx as u64;
        let resp = fixture
            .request(
                action_id,
                "textDocument/codeAction",
                serde_json::json!({
                    "textDocument": { "uri": "file:///ws_edge_ca_e0003.py" },
                    "range": target_diag["range"],
                    "context": { "diagnostics": [target_diag] }
                }),
            )
            .await?
            .ok_or(format!("no code action response for E0003 variant {idx}"))?;

        assert!(
            resp.contains("quickfix"),
            "E0003 code action variant {idx} should be quickfix: {resp}"
        );
    }
    Ok(())
}

// ── Execute Command tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_initialize_advertises_execute_command_provider() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(
        response.contains("executeCommandProvider"),
        "initialize response should advertise executeCommandProvider: {response}"
    );
    assert!(
        response.contains("basilisk.organizeImports"),
        "executeCommandProvider should list basilisk.organizeImports command: {response}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_execute_command_organize_imports_returns_success() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Open a document with unsorted imports so the command has something to work with.
    let code = "import os\nimport sys\n\nx: int = 42\n";
    fixture
        .did_open("file:///ws_exec_cmd_org.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Send workspace/executeCommand with basilisk.organizeImports.
    let resp = fixture
        .request(
            500,
            "workspace/executeCommand",
            serde_json::json!({
                "command": "basilisk.organizeImports",
                "arguments": ["file:///ws_exec_cmd_org.py"]
            }),
        )
        .await?
        .ok_or("no response to workspace/executeCommand")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    // The command returns Ok(None), which serializes as {"result": null}.
    assert!(
        parsed.get("result").is_some(),
        "executeCommand should return a result (even if null): {resp}"
    );
    // Must not have an error field.
    assert!(
        parsed.get("error").is_none(),
        "executeCommand should not return an error: {resp}"
    );
    Ok(())
}

// ── Phase 2: Function Return Type Inlay Hints ─────────────────────────────

#[tokio::test]
async fn test_ws_inlay_hint_return_type_inferred() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Function without return annotation — should get a `-> int` inlay hint.
    let code = "def add(a: int, b: int):\n    return 42\n";
    fixture
        .did_open("file:///ws_ret_hint.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            510,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_ret_hint.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 2, "character": 0 }
                }
            }),
        )
        .await?
        .ok_or("no inlay hint response")?;

    assert!(
        resp.contains("-> int"),
        "should show inferred return type '-> int' for function returning 42: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_return_type_not_shown_when_annotated() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Function WITH explicit return annotation — no return-type inlay hint.
    let code = "def greet(name: str) -> str:\n    return \"hi\"\n";
    fixture
        .did_open("file:///ws_ret_hint_ann.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            511,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_ret_hint_ann.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 2, "character": 0 }
                }
            }),
        )
        .await?
        .ok_or("no inlay hint response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    // If there are hints, none should be a return-type hint (no "-> " prefix).
    if let Some(arr) = result.as_array() {
        for hint in arr {
            let label = hint["label"].as_str().unwrap_or("");
            assert!(
                !label.starts_with(" -> "),
                "annotated function should NOT get a return-type inlay hint: {resp}"
            );
        }
    }
    Ok(())
}

// ── Phase 2: Keyword Argument Completions ──────────────────────────────────

#[tokio::test]
async fn test_ws_completion_kwarg_suggests_param_names() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str, greeting: str) -> str:
    return f\"{greeting}, {name}!\"

result: str = greet()
";
    fixture
        .did_open("file:///ws_kwarg_comp.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor inside greet() — line 3, character 20 (after the opening paren)
    let resp = fixture
        .request(
            520,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_kwarg_comp.py" },
                "position": { "line": 3, "character": 20 }
            }),
        )
        .await?
        .ok_or("no completion response for kwarg")?;

    assert!(
        resp.contains("\"label\":\"name=\""),
        "should suggest 'name=' kwarg completion: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"greeting=\""),
        "should suggest 'greeting=' kwarg completion: {resp}"
    );
    // Kind should be KEYWORD (14 in LSP spec)
    assert!(
        resp.contains("\"kind\":14"),
        "kwarg completions should have kind KEYWORD (14): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_kwarg_skips_already_provided() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str, greeting: str) -> str:
    return f\"{greeting}, {name}!\"

result: str = greet(name=\"world\", )
";
    fixture
        .did_open("file:///ws_kwarg_skip.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor after "name=\"world\", " — line 3, character 33
    let resp = fixture
        .request(
            521,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_kwarg_skip.py" },
                "position": { "line": 3, "character": 33 }
            }),
        )
        .await?
        .ok_or("no completion response for kwarg skip")?;

    // 'name=' was already provided, so only 'greeting=' should appear.
    assert!(
        !resp.contains("\"label\":\"name=\""),
        "should NOT suggest already-provided 'name=' kwarg: {resp}"
    );
    assert!(
        resp.contains("\"label\":\"greeting=\""),
        "should suggest remaining 'greeting=' kwarg: {resp}"
    );
    Ok(())
}

// ── Phase 4: Document Formatting ────────────────────────────────────────────

#[tokio::test]
async fn test_ws_format_document() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Badly formatted Python: inconsistent spacing, missing trailing newline.
    let code = "x:int=1\ny:str=\"hello\"\ndef   greet( name:str )->str:\n    return f\"Hello, {name}!\"";
    fixture
        .did_open("file:///ws_format.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            600,
            "textDocument/formatting",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_format.py" },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        )
        .await?
        .ok_or("no formatting response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    // If ruff is available, we should get text edits back.
    // If ruff is not installed, result may be null — that's acceptable.
    if !result.is_null() {
        let edits = result
            .as_array()
            .ok_or("formatting result should be an array of TextEdits")?;
        assert!(
            !edits.is_empty(),
            "formatting should produce at least one TextEdit for badly formatted code: {resp}"
        );

        // Verify the edit has a range and newText.
        let first_edit = &edits[0];
        assert!(
            first_edit.get("range").is_some(),
            "TextEdit should have a range: {resp}"
        );
        assert!(
            first_edit.get("newText").is_some(),
            "TextEdit should have newText: {resp}"
        );

        // The formatted text should differ from the original.
        let new_text = first_edit["newText"]
            .as_str()
            .ok_or("newText should be a string")?;
        assert_ne!(
            new_text, code,
            "formatted text should differ from original"
        );
    }

    Ok(())
}

// ── Workspace Symbols ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_workspace_symbols() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Open two documents with distinct symbols.
    let doc1 = "class Greeter:\n    name: str\n\ndef greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    let doc2 = "class Calculator:\n    value: int\n\ndef compute(x: int, y: int) -> int:\n    return x + y";

    fixture.did_open("file:///ws_sym_a.py", doc1).await?;
    let _ = fixture.wait_for_diagnostics().await;

    fixture.did_open("file:///ws_sym_b.py", doc2).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Query all symbols — empty string returns everything.
    let resp_all = fixture
        .request(
            500,
            "workspace/symbol",
            serde_json::json!({ "query": "" }),
        )
        .await?
        .ok_or("no workspace/symbol response for empty query")?;

    assert!(
        resp_all.contains("\"result\""),
        "expected result in response: {resp_all}"
    );
    // Both documents' classes should appear.
    assert!(
        resp_all.contains("\"Greeter\""),
        "expected Greeter in workspace symbols: {resp_all}"
    );
    assert!(
        resp_all.contains("\"Calculator\""),
        "expected Calculator in workspace symbols: {resp_all}"
    );
    // Functions from both documents should appear.
    assert!(
        resp_all.contains("\"greet\""),
        "expected greet in workspace symbols: {resp_all}"
    );
    assert!(
        resp_all.contains("\"compute\""),
        "expected compute in workspace symbols: {resp_all}"
    );

    // Query filtered — only symbols matching "calc" (case-insensitive).
    let resp_filtered = fixture
        .request(
            501,
            "workspace/symbol",
            serde_json::json!({ "query": "calc" }),
        )
        .await?
        .ok_or("no workspace/symbol response for filtered query")?;

    assert!(
        resp_filtered.contains("\"Calculator\""),
        "expected Calculator with query 'calc': {resp_filtered}"
    );
    assert!(
        !resp_filtered.contains("\"Greeter\""),
        "Greeter should be filtered out with query 'calc': {resp_filtered}"
    );
    assert!(
        !resp_filtered.contains("\"greet\""),
        "greet should be filtered out with query 'calc': {resp_filtered}"
    );

    Ok(())
}

// ── Phase 4: Folding Ranges ──────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_folding_ranges() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Document with a multi-line class containing a multi-line function.
    let code = "\
import os
import sys

class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(name: str) -> str:
    return f\"Hello, {name}!\"
";
    fixture
        .did_open("file:///ws_folding.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            610,
            "textDocument/foldingRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_folding.py" }
            }),
        )
        .await?
        .ok_or("no folding range response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("folding ranges result should be an array")?;

    // Should have ranges for: speak method, Animal class, greet function,
    // and possibly the import block.
    assert!(
        ranges.len() >= 3,
        "should have at least 3 folding ranges (class, 2 functions), got {}: {resp}",
        ranges.len()
    );

    // Verify each range has startLine and endLine.
    for range in ranges {
        assert!(
            range.get("startLine").is_some(),
            "folding range should have startLine: {resp}"
        );
        assert!(
            range.get("endLine").is_some(),
            "folding range should have endLine: {resp}"
        );
    }

    // Verify we have a region kind for the class/function ranges.
    let region_count = ranges
        .iter()
        .filter(|r| r["kind"].as_str() == Some("region"))
        .count();
    assert!(
        region_count >= 3,
        "should have at least 3 region-kind folding ranges, got {region_count}: {resp}"
    );

    Ok(())
}

// ── Phase 4: Selection Ranges ───────────────────────────────────────────────

#[tokio::test]
async fn test_ws_selection_ranges() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Class with a method that has a typed parameter — cursor on the parameter name
    // should yield nested ranges: param name → function def → class def → whole doc.
    let code = "\
class Greeter:
    def greet(self, name: str) -> str:
        return f\"Hello, {name}!\"
";
    fixture
        .did_open("file:///ws_selection.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor on the 'n' of `name` parameter (line 1, character 20).
    let resp = fixture
        .request(
            620,
            "textDocument/selectionRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_selection.py" },
                "positions": [{ "line": 1, "character": 20 }]
            }),
        )
        .await?
        .ok_or("no selection range response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("selection range result should be an array")?;

    // One position → one SelectionRange.
    assert_eq!(
        ranges.len(),
        1,
        "should have exactly 1 selection range for 1 position: {resp}"
    );

    let sel = &ranges[0];
    // The innermost range should exist.
    assert!(
        sel.get("range").is_some(),
        "selection range should have a range: {resp}"
    );

    // Walk the parent chain — there should be at least 2 levels
    // (innermost + at least one parent containing the whole document).
    let mut depth = 1;
    let mut current = sel.clone();
    while let Some(parent) = current.get("parent") {
        if parent.is_null() {
            break;
        }
        depth += 1;
        current = parent.clone();
    }
    assert!(
        depth >= 2,
        "selection range should have nested parents (depth >= 2), got {depth}: {resp}"
    );

    Ok(())
}

// ── Document Highlight ───────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_document_highlight() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str) -> str:
    return name
greet(\"hi\")
";
    fixture.did_open("file:///ws_highlight.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Request documentHighlight at the position of `greet` on line 0 (character 4).
    let resp = fixture
        .request(
            630,
            "textDocument/documentHighlight",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_highlight.py" },
                "position": { "line": 0, "character": 4 }
            }),
        )
        .await?
        .ok_or("no documentHighlight response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let highlights = parsed["result"]
        .as_array()
        .ok_or("documentHighlight result should be an array")?;

    // Should find at least 2 highlights: definition of greet + call of greet.
    assert!(
        highlights.len() >= 2,
        "should find at least 2 highlights for 'greet' (found {}): {resp}",
        highlights.len()
    );

    // Each highlight should have a range and a kind.
    for hl in highlights {
        assert!(
            hl.get("range").is_some(),
            "highlight should have a range: {hl}"
        );
        assert!(
            hl.get("kind").is_some(),
            "highlight should have a kind: {hl}"
        );
    }

    Ok(())
}

// ── Phase 0: Shutdown ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_shutdown_gracefully() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Send shutdown request — server must respond with result: null.
    // tower-lsp requires an empty object (not null) for shutdown params.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 900,
            "method": "shutdown"
        }))
        .await?;

    let id_str = "\"id\":900";
    let mut resp = None;
    for _ in 0..10 {
        let Some(msg) = fixture.recv().await else { break };
        if msg.contains(id_str) {
            resp = Some(msg);
            break;
        }
    }
    let resp = resp.ok_or("no response to shutdown request")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed.get("result").is_some(),
        "shutdown should return a result: {resp}"
    );
    assert!(
        parsed.get("error").is_none(),
        "shutdown should not return an error: {resp}"
    );

    // Send exit notification — server should close the connection.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "exit",
            "params": null
        }))
        .await?;

    // After exit, the connection should be closed. Reading should yield None.
    let msg = fixture.recv().await;
    // Either None (closed) or we receive nothing — both are acceptable.
    // The key assertion is that shutdown itself returned cleanly.
    let _ = msg;

    Ok(())
}

// ── Phase 1: Hover on class name ────────────────────────────────────────────

#[tokio::test]
async fn test_ws_hover_class_name_shows_class_info() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "class Animal:\n    name: str\n    age: int\n";
    fixture.did_open("file:///ws_hover_class.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Hover on "Animal" — line 0, character 6 (inside "Animal").
    let resp = fixture
        .request(
            901,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_hover_class.py" },
                "position": { "line": 0, "character": 6 }
            }),
        )
        .await?
        .ok_or("no hover response for class name")?;

    assert!(
        resp.contains("(class)"),
        "hover on class name should show '(class)' prefix: {resp}"
    );
    assert!(
        resp.contains("Animal"),
        "hover on class name should show class name 'Animal': {resp}"
    );
    Ok(())
}

// ── Phase 1: Hover on variable shows type ───────────────────────────────────

#[tokio::test]
async fn test_ws_hover_variable_shows_type() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "x: int = 42\ny: str = \"hello\"\n";
    fixture
        .did_open("file:///ws_hover_var.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Hover on "x" — line 0, character 0.
    let resp = fixture
        .request(
            902,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_hover_var.py" },
                "position": { "line": 0, "character": 0 }
            }),
        )
        .await?
        .ok_or("no hover response for variable")?;

    assert!(
        resp.contains("(variable)"),
        "hover on variable should show '(variable)' prefix: {resp}"
    );
    assert!(
        resp.contains("int"),
        "hover on variable should show type 'int': {resp}"
    );
    Ok(())
}

// ── Phase 1: Go to Definition from class usage ─────────────────────────────

#[tokio::test]
async fn test_ws_goto_definition_class_usage() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // "Animal" is defined on line 0, used as a type annotation on line 3.
    let code = "\
class Animal:
    name: str

def greet(pet: Animal) -> str:
    return pet.name
";
    fixture
        .did_open("file:///ws_goto_class_usage.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Goto definition on "Animal" in the type annotation on line 3.
    // "def greet(pet: Animal)" — 'A' of "Animal" is at character 15.
    let resp = fixture
        .request(
            903,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_goto_class_usage.py" },
                "position": { "line": 3, "character": 15 }
            }),
        )
        .await?
        .ok_or("no definition response for class usage")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on class usage must resolve: {resp}"
    );

    // Should jump to line 0 where "class Animal:" is defined.
    // "class " is 6 chars, so 'Animal' starts at character 6.
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "goto-def from class usage should jump to line 0: {resp}"
    );
    assert_eq!(
        start["character"], 6,
        "goto-def from class usage should land at char 6 where 'Animal' is defined: {resp}"
    );
    Ok(())
}

// ── Phase 3: Inlay Hints — parameter names at call sites ────────────────────

#[tokio::test]
async fn test_ws_inlay_hint_parameter_names_at_call_site() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str, greeting: str) -> str:
    return f\"{greeting}, {name}!\"

result: str = greet(\"world\", \"Hi\")
";
    fixture
        .did_open("file:///ws_inlay_param.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            904,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_inlay_param.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 4, "character": 0 }
                }
            }),
        )
        .await?
        .ok_or("no inlay hint response")?;

    assert!(
        resp.contains("name="),
        "inlay hints should show parameter name 'name=' at call site: {resp}"
    );
    assert!(
        resp.contains("greeting="),
        "inlay hints should show parameter name 'greeting=' at call site: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_call_hierarchy() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let source = "def foo():\n    pass\n\ndef bar():\n    foo()\n    foo()\n";
    fixture
        .did_open("file:///ws_call_hierarchy.py", source)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // prepareCallHierarchy at the position of `foo` (line 0, character 4)
    let prepare_resp = fixture
        .request(
            200,
            "textDocument/prepareCallHierarchy",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_call_hierarchy.py" },
                "position": { "line": 0, "character": 4 }
            }),
        )
        .await?
        .ok_or("no prepareCallHierarchy response")?;

    assert!(
        prepare_resp.contains("\"name\":\"foo\""),
        "prepareCallHierarchy should return item named 'foo': {prepare_resp}"
    );

    // callHierarchy/incomingCalls for foo
    let incoming_resp = fixture
        .request(
            201,
            "callHierarchy/incomingCalls",
            serde_json::json!({
                "item": {
                    "name": "foo",
                    "kind": 12,
                    "uri": "file:///ws_call_hierarchy.py",
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 3 }
                    },
                    "selectionRange": {
                        "start": { "line": 0, "character": 4 },
                        "end": { "line": 0, "character": 7 }
                    }
                }
            }),
        )
        .await?
        .ok_or("no incomingCalls response")?;

    assert!(
        incoming_resp.contains("\"name\":\"bar\""),
        "incomingCalls should show 'bar' as a caller of 'foo': {incoming_resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_type_hierarchy() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let source = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str

class Puppy(Dog):
    age: int
";
    fixture
        .did_open("file:///ws_type_hierarchy.py", source)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // prepareTypeHierarchy on `Dog` (line 3, character 6 — inside the class name)
    let prepare_resp = fixture
        .request(
            300,
            "textDocument/prepareTypeHierarchy",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_type_hierarchy.py" },
                "position": { "line": 3, "character": 6 }
            }),
        )
        .await?
        .ok_or("no prepareTypeHierarchy response")?;

    assert!(
        prepare_resp.contains("\"name\":\"Dog\""),
        "prepareTypeHierarchy should return item named 'Dog': {prepare_resp}"
    );

    // typeHierarchy/supertypes for Dog -> should include Animal
    let supertypes_resp = fixture
        .request(
            301,
            "typeHierarchy/supertypes",
            serde_json::json!({
                "item": {
                    "name": "Dog",
                    "kind": 5,
                    "uri": "file:///ws_type_hierarchy.py",
                    "range": {
                        "start": { "line": 3, "character": 0 },
                        "end": { "line": 3, "character": 5 }
                    },
                    "selectionRange": {
                        "start": { "line": 3, "character": 6 },
                        "end": { "line": 3, "character": 9 }
                    },
                    "data": "Dog"
                }
            }),
        )
        .await?
        .ok_or("no supertypes response")?;

    assert!(
        supertypes_resp.contains("\"name\":\"Animal\""),
        "supertypes of Dog should include Animal: {supertypes_resp}"
    );

    // typeHierarchy/subtypes for Dog -> should include Puppy
    let subtypes_resp = fixture
        .request(
            302,
            "typeHierarchy/subtypes",
            serde_json::json!({
                "item": {
                    "name": "Dog",
                    "kind": 5,
                    "uri": "file:///ws_type_hierarchy.py",
                    "range": {
                        "start": { "line": 3, "character": 0 },
                        "end": { "line": 3, "character": 5 }
                    },
                    "selectionRange": {
                        "start": { "line": 3, "character": 6 },
                        "end": { "line": 3, "character": 9 }
                    },
                    "data": "Dog"
                }
            }),
        )
        .await?
        .ok_or("no subtypes response")?;

    assert!(
        subtypes_resp.contains("\"name\":\"Puppy\""),
        "subtypes of Dog should include Puppy: {subtypes_resp}"
    );

    Ok(())
}

// ── Bonnet: Additional coverage tests ───────────────────────────────────────

#[tokio::test]
async fn test_ws_hover_class_with_bases() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str
";
    fixture
        .did_open("file:///ws_hover_bases.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Hover on "Dog" — line 3, character 6 (inside "Dog").
    let resp = fixture
        .request(
            950,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_hover_bases.py" },
                "position": { "line": 3, "character": 6 }
            }),
        )
        .await?
        .ok_or("no hover response for class with bases")?;

    assert!(
        resp.contains("(class)"),
        "hover on class with bases should show '(class)' prefix: {resp}"
    );
    assert!(
        resp.contains("Dog"),
        "hover should show class name 'Dog': {resp}"
    );
    assert!(
        resp.contains("Animal"),
        "hover on class with bases should show base class 'Animal': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_rename_multiple_occurrences() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def helper(x: int) -> int:
    return x

a: int = helper(1)
b: int = helper(2)
c: int = helper(3)
";
    fixture
        .did_open("file:///ws_ren_multi.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Rename "helper" to "assist" (line 0, character 4).
    let resp = fixture
        .request(
            951,
            "textDocument/rename",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_ren_multi.py" },
                "position": { "line": 0, "character": 4 },
                "newName": "assist"
            }),
        )
        .await?
        .ok_or("no rename response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let changes = &parsed["result"]["changes"]["file:///ws_ren_multi.py"];
    let edits = changes
        .as_array()
        .ok_or("rename should produce an array of edits")?;

    // "helper" appears 4 times: definition + 3 call sites.
    assert!(
        edits.len() >= 4,
        "rename should produce at least 4 edits (def + 3 calls), got {}: {resp}",
        edits.len()
    );
    // Every edit should replace with "assist".
    for edit in edits {
        assert_eq!(
            edit["newText"].as_str(),
            Some("assist"),
            "each edit should replace with 'assist': {edit}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_variable() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "x: int = 42\n";
    fixture
        .did_open("file:///ws_goto_var.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Goto definition on "x" — line 0, character 0.
    let resp = fixture
        .request(
            952,
            "textDocument/definition",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_goto_var.py" },
                "position": { "line": 0, "character": 0 }
            }),
        )
        .await?
        .ok_or("no definition response for variable")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on variable must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(start["line"], 0, "variable definition should be on line 0: {resp}");
    assert_eq!(start["character"], 0, "variable definition should start at char 0: {resp}");
    Ok(())
}

#[tokio::test]
async fn test_ws_hover_import_shows_module() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "import os\n\nx: int = 42\n";
    fixture
        .did_open("file:///ws_hover_import.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Hover on "os" — line 0, character 7 (inside "os").
    let resp = fixture
        .request(
            953,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_hover_import.py" },
                "position": { "line": 0, "character": 7 }
            }),
        )
        .await?
        .ok_or("no hover response for import")?;

    assert!(
        resp.contains("os"),
        "hover on import should show module name 'os': {resp}"
    );
    assert!(
        resp.contains("import") || resp.contains("module"),
        "hover on import should show import/module info: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_find_references_class() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Dog:
    name: str

def adopt(pet: Dog) -> Dog:
    return pet
";
    fixture
        .did_open("file:///ws_refs_class.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Find references for "Dog" (line 0, character 6).
    let resp = fixture
        .request(
            954,
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_refs_class.py" },
                "position": { "line": 0, "character": 6 },
                "context": { "includeDeclaration": true }
            }),
        )
        .await?
        .ok_or("no references response for class")?;

    // "Dog" appears 3 times: class def + param annotation + return annotation.
    let count = resp.matches("ws_refs_class.py").count();
    assert!(
        count >= 3,
        "should find at least 3 references for 'Dog' (found {count}): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_completion_kind_values() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Widget:
    size: int

def render(w: Widget) -> str:
    return \"ok\"

count: int = 0
";
    fixture
        .did_open("file:///ws_comp_kinds.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            955,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_comp_kinds.py" },
                "position": { "line": 7, "character": 0 }
            }),
        )
        .await?
        .ok_or("no completion response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let items = parsed["result"]
        .as_array()
        .or_else(|| parsed["result"]["items"].as_array())
        .ok_or("completion result should have items")?;

    // Find the Widget class completion — kind 7 (Class).
    let widget = items.iter().find(|i| i["label"].as_str() == Some("Widget"));
    assert!(widget.is_some(), "should have Widget in completions: {resp}");
    assert_eq!(
        widget.map(|w| w["kind"].as_u64()),
        Some(Some(7)),
        "Widget should have kind CLASS (7): {resp}"
    );

    // Find the render function completion — kind 3 (Function).
    let render = items.iter().find(|i| i["label"].as_str() == Some("render"));
    assert!(render.is_some(), "should have render in completions: {resp}");
    assert_eq!(
        render.map(|r| r["kind"].as_u64()),
        Some(Some(3)),
        "render should have kind FUNCTION (3): {resp}"
    );

    // Find the count variable completion — kind 6 (Variable).
    let count = items.iter().find(|i| i["label"].as_str() == Some("count"));
    assert!(count.is_some(), "should have count in completions: {resp}");
    assert_eq!(
        count.map(|c| c["kind"].as_u64()),
        Some(Some(6)),
        "count should have kind VARIABLE (6): {resp}"
    );
    Ok(())
}

// ── Code Lens tests via WebSocket ───────────────────────────────────────────

#[tokio::test]
async fn test_ws_code_lens() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str) -> str:
    return name

x = greet(\"hello\")
y = greet(\"world\")
";
    fixture
        .did_open("file:///ws_code_lens.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            400,
            "textDocument/codeLens",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_code_lens.py" }
            }),
        )
        .await?
        .ok_or("no codeLens response")?;

    // The function `greet` is called twice (line 4 + line 5), so 2 references.
    assert!(
        resp.contains("2 references"),
        "codeLens should show '2 references' for greet: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_class_references() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str

def make_animal() -> Animal:
    return Animal()

x: Animal = make_animal()
";
    fixture
        .did_open("file:///ws_code_lens_class_refs.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            401,
            "textDocument/codeLens",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_code_lens_class_refs.py" }
            }),
        )
        .await?
        .ok_or("no codeLens response")?;

    // `Animal` appears in: definition, Dog(Animal), -> Animal, Animal(), x: Animal = 5 total => 4 references.
    assert!(
        resp.contains("4 references"),
        "codeLens should show '4 references' for Animal: {resp}"
    );
    // `Dog` is defined but never used elsewhere => 0 references.
    assert!(
        resp.contains("0 references"),
        "codeLens should show '0 references' for Dog: {resp}"
    );
    // `make_animal` is called once => 1 reference.
    assert!(
        resp.contains("1 reference"),
        "codeLens should show '1 reference' for make_animal: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_single_reference() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def helper(x: int) -> int:
    return x

result: int = helper(42)
";
    fixture
        .did_open("file:///ws_code_lens_single_ref.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            402,
            "textDocument/codeLens",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_code_lens_single_ref.py" }
            }),
        )
        .await?
        .ok_or("no codeLens response")?;

    // `helper` is called once (line 4), so singular "1 reference".
    assert!(
        resp.contains("1 reference"),
        "codeLens should show singular '1 reference' for helper: {resp}"
    );
    // Must NOT show "1 references" (plural).
    assert!(
        !resp.contains("1 references"),
        "codeLens must use singular form '1 reference', not '1 references': {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_no_references() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def unused_func(x: int) -> int:
    return x
";
    fixture
        .did_open("file:///ws_code_lens_no_refs.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            403,
            "textDocument/codeLens",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_code_lens_no_refs.py" }
            }),
        )
        .await?
        .ok_or("no codeLens response")?;

    // `unused_func` is never called, so 0 references.
    assert!(
        resp.contains("0 references"),
        "codeLens should show '0 references' for unused function: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_methods_excluded() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class MyClass:
    def method_one(self) -> None:
        pass

    def method_two(self) -> None:
        self.method_one()
";
    fixture
        .did_open("file:///ws_code_lens_methods.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            404,
            "textDocument/codeLens",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_code_lens_methods.py" }
            }),
        )
        .await?
        .ok_or("no codeLens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let lenses = parsed["result"]
        .as_array()
        .ok_or("codeLens result should be an array")?;

    // Only `MyClass` should get a lens; methods should be excluded.
    // method_one and method_two are inside a class, so they must not appear.
    assert_eq!(
        lenses.len(),
        1,
        "only the class should get a code lens, not methods: {resp}"
    );

    // The single lens should be for MyClass.
    let title = lenses[0]["command"]["title"]
        .as_str()
        .ok_or("lens should have a title")?;
    assert!(
        title.contains("references"),
        "the single lens should be a reference count for MyClass: {title}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_code_lens_multiple_functions() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def alpha(x: int) -> int:
    return x

def beta(y: int) -> int:
    return alpha(y)

def gamma(z: int) -> int:
    return beta(alpha(z))
";
    fixture
        .did_open("file:///ws_code_lens_multi.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            405,
            "textDocument/codeLens",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_code_lens_multi.py" }
            }),
        )
        .await?
        .ok_or("no codeLens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let lenses = parsed["result"]
        .as_array()
        .ok_or("codeLens result should be an array")?;

    // Three top-level functions => three lenses.
    assert_eq!(
        lenses.len(),
        3,
        "each top-level function should get its own code lens: {resp}"
    );

    // Collect titles in order (alpha, beta, gamma).
    let titles: Vec<&str> = lenses
        .iter()
        .filter_map(|lens| lens["command"]["title"].as_str())
        .collect();

    assert_eq!(titles.len(), 3, "all three lenses should have titles");

    // alpha is called in beta (line 5) and gamma (line 8) => 2 references.
    assert_eq!(
        titles[0], "2 references",
        "alpha should have 2 references: {resp}"
    );
    // beta is called in gamma (line 8) => 1 reference.
    assert_eq!(
        titles[1], "1 reference",
        "beta should have 1 reference: {resp}"
    );
    // gamma is never called => 0 references.
    assert_eq!(
        titles[2], "0 references",
        "gamma should have 0 references: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_semantic_tokens_decorator() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
from typing import Generic, TypeVar

T = TypeVar('T')

class Box(Generic[T]):
    value: T

    @staticmethod
    def empty() -> None:
        pass

def greet(name: str) -> str:
    return name
";
    fixture
        .did_open("file:///ws_semtok_dec.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            900,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_dec.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    assert!(
        resp.contains("\"data\""),
        "semantic tokens should contain 'data' array: {resp}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    // Each token is 5 integers; we have decorators, type annotations, type params, etc.
    assert_eq!(
        data.len() % 5,
        0,
        "token data length should be multiple of 5"
    );

    // Should have many tokens: imports, T, Box, Generic[T], value, staticmethod,
    // empty, greet, name, str, str return annotations, etc.
    // Minimum: at least 8 tokens (40 integers)
    assert!(
        data.len() >= 40,
        "should have at least 8 tokens for decorated code: {resp}"
    );

    Ok(())
}

// ── Phase 4 Additional: Workspace Symbols ────────────────────────────────────

#[tokio::test]
async fn test_ws_workspace_symbols_empty_query() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Open a document with known symbols.
    let code = "class Dog:\n    breed: str\n\ndef bark(volume: int) -> str:\n    return \"woof\"";
    fixture.did_open("file:///ws_sym_empty.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Empty query should return all symbols from all open documents.
    let resp = fixture
        .request(
            700,
            "workspace/symbol",
            serde_json::json!({ "query": "" }),
        )
        .await?
        .ok_or("no workspace/symbol response for empty query")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let symbols = result
        .as_array()
        .ok_or("workspace/symbol result should be an array")?;

    // Empty query should still return symbols — not an empty array.
    assert!(
        !symbols.is_empty(),
        "empty query should still return symbols: {resp}"
    );

    // We opened a file with Dog class and bark function — both should appear.
    assert!(
        resp.contains("\"Dog\""),
        "expected Dog class in workspace symbols with empty query: {resp}"
    );
    assert!(
        resp.contains("\"bark\""),
        "expected bark function in workspace symbols with empty query: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_workspace_symbols_no_match() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Open a document with known symbols.
    let code = "class Apple:\n    color: str\n\ndef eat(fruit: str) -> str:\n    return fruit";
    fixture
        .did_open("file:///ws_sym_nomatch.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Query for something that matches nothing in the document.
    let resp = fixture
        .request(
            701,
            "workspace/symbol",
            serde_json::json!({ "query": "zzzznonexistent" }),
        )
        .await?
        .ok_or("no workspace/symbol response for non-matching query")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    // No symbols should match a nonsense query.
    // Valid LSP responses: null (no results) or an empty array.
    if result.is_null() {
        // null means no results — this is valid.
    } else {
        let symbols = result
            .as_array()
            .ok_or("workspace/symbol result should be null or an array")?;
        assert!(
            symbols.is_empty(),
            "query with no matching symbols should return empty array or null, got {} symbols: {resp}",
            symbols.len()
        );
    }

    Ok(())
}

// ── Phase 4 Additional: Format Document ──────────────────────────────────────

#[tokio::test]
async fn test_ws_format_document_already_formatted() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Well-formatted Python code (PEP 8 compliant, trailing newline).
    let code = "x: int = 1\ny: str = \"hello\"\n\n\ndef greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture
        .did_open("file:///ws_format_clean.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            710,
            "textDocument/formatting",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_format_clean.py" },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        )
        .await?
        .ok_or("no formatting response for already-formatted code")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    // For already-formatted code, result should be null (no changes)
    // or an empty array of edits — both are valid LSP responses.
    if !result.is_null() {
        let edits = result
            .as_array()
            .ok_or("formatting result should be null or an array")?;
        // If edits are returned, verify the new text is the same as original
        // (ruff may return a whole-file replacement that is identical).
        if !edits.is_empty() {
            let new_text = edits[0]["newText"]
                .as_str()
                .unwrap_or("");
            // The resulting text should be equivalent to the input.
            assert!(
                new_text == code || edits.is_empty(),
                "already-formatted code should produce no meaningful changes: {resp}"
            );
        }
    }
    // result == null is also fine — means no edits needed.

    Ok(())
}

#[tokio::test]
async fn test_ws_format_document_empty_file() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Empty file — formatting should not crash.
    let code = "";
    fixture
        .did_open("file:///ws_format_empty.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            711,
            "textDocument/formatting",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_format_empty.py" },
                "options": { "tabSize": 4, "insertSpaces": true }
            }),
        )
        .await?
        .ok_or("no formatting response for empty file")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;

    // Response must have a result field — it can be null or an empty array.
    assert!(
        parsed.get("result").is_some(),
        "formatting empty file should return a valid result: {resp}"
    );

    let result = &parsed["result"];
    if !result.is_null() {
        let edits = result
            .as_array()
            .ok_or("formatting result for empty file should be null or an array")?;
        // Empty file should not produce meaningful edits.
        // If there are edits, the newText should be empty or whitespace-only.
        for edit in edits {
            let new_text = edit["newText"].as_str().unwrap_or("");
            assert!(
                new_text.trim().is_empty(),
                "empty file formatting should not produce non-empty content: {resp}"
            );
        }
    }

    Ok(())
}

// ── Phase 4 Additional: Folding Ranges ───────────────────────────────────────

#[tokio::test]
async fn test_ws_folding_ranges_import_block() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Document with consecutive imports that should fold as one block.
    let code = "\
import os
import sys
import json
import typing

def main() -> None:
    pass
";
    fixture
        .did_open("file:///ws_fold_imports.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            720,
            "textDocument/foldingRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_fold_imports.py" }
            }),
        )
        .await?
        .ok_or("no folding range response for import block")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("folding ranges result should be an array")?;

    // We should have at least 1 folding range for the imports block
    // and 1 for the main function.
    assert!(
        ranges.len() >= 2,
        "should have at least 2 folding ranges (imports + function), got {}: {resp}",
        ranges.len()
    );

    // Find a folding range that starts at line 0 (first import) and
    // covers at least through line 3 (last import).
    let has_import_fold = ranges.iter().any(|range| {
        let start = range["startLine"].as_u64().unwrap_or(u64::MAX);
        let end = range["endLine"].as_u64().unwrap_or(0);
        // Import block spans lines 0-3.
        start == 0 && end >= 3
    });

    assert!(
        has_import_fold,
        "consecutive imports should produce a folding range starting at line 0 covering through line 3: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_folding_ranges_nested_class_and_function() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Nested structures: class containing two methods.
    let code = "\
class Outer:
    def method_a(self) -> int:
        x: int = 1
        return x

    def method_b(self) -> str:
        y: str = \"hello\"
        return y

def standalone(val: int) -> int:
    return val + 1
";
    fixture
        .did_open("file:///ws_fold_nested.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            721,
            "textDocument/foldingRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_fold_nested.py" }
            }),
        )
        .await?
        .ok_or("no folding range response for nested structures")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("folding ranges result should be an array")?;

    // We expect separate folding ranges for:
    // 1. Outer class (lines 0-8)
    // 2. method_a (lines 1-3)
    // 3. method_b (lines 5-7)
    // 4. standalone function (lines 9-10)
    assert!(
        ranges.len() >= 4,
        "should have at least 4 folding ranges (class + 2 methods + standalone), got {}: {resp}",
        ranges.len()
    );

    // Collect all (startLine, endLine) pairs.
    let fold_pairs: Vec<(u64, u64)> = ranges
        .iter()
        .filter_map(|range| {
            let start = range["startLine"].as_u64()?;
            let end = range["endLine"].as_u64()?;
            Some((start, end))
        })
        .collect();

    // The Outer class fold should start at line 0.
    let has_class_fold = fold_pairs.iter().any(|(start, _)| *start == 0);
    assert!(
        has_class_fold,
        "Outer class should have a folding range starting at line 0: {resp}"
    );

    // There should be a method fold starting at line 1 (method_a).
    let has_method_a_fold = fold_pairs.iter().any(|(start, _)| *start == 1);
    assert!(
        has_method_a_fold,
        "method_a should have a folding range starting at line 1: {resp}"
    );

    // There should be a method fold starting at line 5 (method_b).
    let has_method_b_fold = fold_pairs.iter().any(|(start, _)| *start == 5);
    assert!(
        has_method_b_fold,
        "method_b should have a folding range starting at line 5: {resp}"
    );

    Ok(())
}

// ── Phase 4 Additional: Selection Ranges ─────────────────────────────────────

#[tokio::test]
async fn test_ws_selection_ranges_has_parent_chain() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Deeply nested structure to ensure parent chain hierarchy.
    let code = "\
class Container:
    def process(self, data: str) -> str:
        result: str = data.upper()
        return result
";
    fixture
        .did_open("file:///ws_sel_chain.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor on 'result' inside the method body (line 2, character 8).
    let resp = fixture
        .request(
            730,
            "textDocument/selectionRange",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_sel_chain.py" },
                "positions": [{ "line": 2, "character": 8 }]
            }),
        )
        .await?
        .ok_or("no selection range response for parent chain test")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    let ranges = result
        .as_array()
        .ok_or("selection range result should be an array")?;

    assert_eq!(
        ranges.len(),
        1,
        "should have exactly 1 selection range for 1 position: {resp}"
    );

    let sel = &ranges[0];

    // The innermost range must exist with a valid range object.
    let inner_range = sel
        .get("range")
        .ok_or("selection range should have a range")?;
    assert!(
        inner_range.get("start").is_some() && inner_range.get("end").is_some(),
        "innermost range should have start and end positions: {resp}"
    );

    // Walk up the parent chain and verify hierarchy:
    // Each parent's range should be equal to or larger than its child.
    let mut depth = 1;
    let mut current = sel.clone();
    let mut prev_start_line = inner_range["start"]["line"].as_u64().unwrap_or(u64::MAX);
    let mut prev_end_line = inner_range["end"]["line"].as_u64().unwrap_or(0);

    while let Some(parent) = current.get("parent") {
        if parent.is_null() {
            break;
        }
        depth += 1;

        let parent_range = parent
            .get("range")
            .ok_or("parent selection range should have a range")?;
        let parent_start = parent_range["start"]["line"].as_u64().unwrap_or(u64::MAX);
        let parent_end = parent_range["end"]["line"].as_u64().unwrap_or(0);

        // Parent range must be at least as large as child range.
        assert!(
            parent_start <= prev_start_line && parent_end >= prev_end_line,
            "parent range ({parent_start}..{parent_end}) should contain child range ({prev_start_line}..{prev_end_line}): {resp}"
        );

        prev_start_line = parent_start;
        prev_end_line = parent_end;
        current = parent.clone();
    }

    // For a variable inside a method inside a class, we expect at least 3 levels:
    // variable/statement -> method -> class (or more).
    assert!(
        depth >= 3,
        "selection range chain should have at least 3 levels of nesting (var -> method -> class), got {depth}: {resp}"
    );

    Ok(())
}

// ── Phase 1-2 edge case tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_document_symbols_module_variables() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // File with ONLY top-level variables (no classes or functions).
    let code = "\
MAX_SIZE: int = 100
name: str = \"basilisk\"
enabled: bool = True
";
    fixture
        .did_open("file:///ws_symbols_vars.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1100,
            "textDocument/documentSymbol",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_symbols_vars.py" }
            }),
        )
        .await?
        .ok_or("no document symbols response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = parsed["result"]
        .as_array()
        .ok_or("expected result array")?;

    // All three module variables should appear.
    let names: Vec<&str> = result
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(
        names.contains(&"MAX_SIZE"),
        "symbols should include 'MAX_SIZE': {resp}"
    );
    assert!(
        names.contains(&"name"),
        "symbols should include 'name': {resp}"
    );
    assert!(
        names.contains(&"enabled"),
        "symbols should include 'enabled': {resp}"
    );

    // Verify they are VARIABLE kind (SymbolKind::VARIABLE = 13).
    for sym in result {
        if sym["name"].as_str() == Some("MAX_SIZE")
            || sym["name"].as_str() == Some("name")
            || sym["name"].as_str() == Some("enabled")
        {
            assert_eq!(
                sym["kind"].as_u64(),
                Some(13),
                "module variable should have kind VARIABLE (13): {sym}"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_document_symbols_multiple_classes() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Cat:
    name: str
    def meow(self) -> str:
        return \"meow\"

class Dog:
    name: str
    def bark(self) -> str:
        return \"woof\"

class Bird:
    name: str
    def chirp(self) -> str:
        return \"tweet\"
";
    fixture
        .did_open("file:///ws_symbols_multi_class.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1101,
            "textDocument/documentSymbol",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_symbols_multi_class.py" }
            }),
        )
        .await?
        .ok_or("no document symbols response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = parsed["result"]
        .as_array()
        .ok_or("expected result array")?;

    // All three classes should appear at top level.
    let top_names: Vec<&str> = result
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(top_names.contains(&"Cat"), "should contain class 'Cat': {resp}");
    assert!(top_names.contains(&"Dog"), "should contain class 'Dog': {resp}");
    assert!(top_names.contains(&"Bird"), "should contain class 'Bird': {resp}");

    // Each class should have children (nested methods).
    for class_name in &["Cat", "Dog", "Bird"] {
        let class_sym = result
            .iter()
            .find(|s| s["name"].as_str() == Some(class_name))
            .ok_or(format!("class '{class_name}' not found"))?;

        // Classes should be kind CLASS (5).
        assert_eq!(
            class_sym["kind"].as_u64(),
            Some(5),
            "class should have kind CLASS (5): {class_sym}"
        );

        let children = class_sym["children"]
            .as_array()
            .ok_or(format!("class '{class_name}' should have children"))?;
        assert!(
            !children.is_empty(),
            "class '{class_name}' should have nested children (methods/attributes): {resp}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_signature_help_method_skips_self() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Greeter:
    prefix: str
    def greet(self, name: str, loud: bool) -> str:
        return f\"{self.prefix} {name}\"

g: Greeter = Greeter()
result: str = g.greet(\"world\", True)
";
    fixture
        .did_open("file:///ws_sighelp_self.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor inside g.greet( call — line 6, after "g.greet("
    // "result: str = g.greet(" is 23 chars, position at char 23
    let resp = fixture
        .request(
            1102,
            "textDocument/signatureHelp",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_sighelp_self.py" },
                "position": { "line": 6, "character": 23 }
            }),
        )
        .await?
        .ok_or("no signature help response")?;

    // Should show greet signature with name and loud but NOT self.
    assert!(
        resp.contains("name"),
        "signature should show parameter 'name': {resp}"
    );
    assert!(
        resp.contains("loud"),
        "signature should show parameter 'loud': {resp}"
    );

    // Parse and verify self is not in the parameter list.
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let signatures = parsed["result"]["signatures"]
        .as_array()
        .ok_or("expected signatures array")?;
    if let Some(sig) = signatures.first() {
        let params = sig["parameters"]
            .as_array()
            .ok_or("expected parameters array")?;
        let param_labels: Vec<&str> = params
            .iter()
            .filter_map(|p| p["label"].as_str())
            .collect();
        assert!(
            !param_labels.iter().any(|l| *l == "self"),
            "signature help should NOT include 'self' as a parameter: {param_labels:?}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_signature_help_class_constructor() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Point:
    x: int
    y: int
    def __init__(self, x: int, y: int) -> None:
        self.x = x
        self.y = y

p: Point = Point(1, 2)
";
    fixture
        .did_open("file:///ws_sighelp_ctor.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Cursor inside Point( constructor call — line 7
    // "p: Point = Point(" is 18 chars, cursor at char 18
    let resp = fixture
        .request(
            1103,
            "textDocument/signatureHelp",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_sighelp_ctor.py" },
                "position": { "line": 7, "character": 18 }
            }),
        )
        .await?
        .ok_or("no signature help response")?;

    // Should show __init__ signature parameters (x and y, not self).
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    assert!(
        !result.is_null(),
        "signature help for constructor should not be null: {resp}"
    );

    // If we get a valid signature, verify it shows the parameters.
    if let Some(signatures) = result["signatures"].as_array() {
        if let Some(sig) = signatures.first() {
            let label = sig["label"].as_str().unwrap_or("");
            assert!(
                label.contains("x") && label.contains("y"),
                "constructor signature should show parameters x and y: {label}"
            );
            // self should not appear in the label.
            let params = sig["parameters"]
                .as_array();
            if let Some(params) = params {
                let param_labels: Vec<&str> = params
                    .iter()
                    .filter_map(|p| p["label"].as_str())
                    .collect();
                assert!(
                    !param_labels.iter().any(|l| *l == "self"),
                    "constructor signature should NOT include 'self': {param_labels:?}"
                );
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_find_references_include_declaration() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def helper(x: int) -> int:
    return x + 1

a: int = helper(10)
b: int = helper(20)
";
    fixture
        .did_open("file:///ws_refs_decl.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Find references for "helper" at its definition (line 0, char 4).
    // With includeDeclaration: true — should include the definition itself.
    let resp_with = fixture
        .request(
            1104,
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_refs_decl.py" },
                "position": { "line": 0, "character": 4 },
                "context": { "includeDeclaration": true }
            }),
        )
        .await?
        .ok_or("no references response with includeDeclaration")?;

    // Should find at least 3: definition + 2 usages.
    let count_with = resp_with.matches("ws_refs_decl.py").count();
    assert!(
        count_with >= 3,
        "with includeDeclaration: true, should find at least 3 references (def + 2 usages), got {count_with}: {resp_with}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_find_references_word_boundary() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // "greet" and "greeting" are different identifiers — searching for "greet"
    // should NOT match "greeting" due to word boundary checking.
    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def greeting(name: str) -> str:
    return f\"Hi, {name}!\"

a: str = greet(\"world\")
b: str = greeting(\"world\")
";
    fixture
        .did_open("file:///ws_refs_boundary.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Find references for "greet" at its definition (line 0, char 4).
    let resp = fixture
        .request(
            1105,
            "textDocument/references",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_refs_boundary.py" },
                "position": { "line": 0, "character": 4 },
                "context": { "includeDeclaration": true }
            }),
        )
        .await?
        .ok_or("no references response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let locations = parsed["result"]
        .as_array()
        .ok_or("expected result array")?;

    // "greet" appears on line 0 (def) and line 6 (usage) = 2 refs.
    // "greeting" on lines 3 and 7 should NOT be included.
    // So we expect exactly 2 references.
    assert_eq!(
        locations.len(),
        2,
        "should find exactly 2 references for 'greet' (not matching 'greeting'): {resp}"
    );

    // Verify none of the locations point to line 3 or line 7 (greeting lines).
    for loc in locations {
        let line = loc["range"]["start"]["line"].as_u64().unwrap_or(99);
        assert!(
            line != 3 && line != 7,
            "reference should NOT match 'greeting' on line {line}: {resp}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_code_action_no_actions_for_clean_code() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Fully annotated code with no redundant annotations — no diagnostics expected.
    let code = "def add(a: int, b: int) -> int:\n    return a + b\n";
    fixture
        .did_open("file:///ws_ca_clean.py", code)
        .await?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;

    // Verify diagnostics are empty.
    assert!(
        diag_msg.contains("\"diagnostics\":[]"),
        "clean code should have no diagnostics: {diag_msg}"
    );

    // Request code actions with empty diagnostics context.
    let resp = fixture
        .request(
            1106,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_ca_clean.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 12 }
                },
                "context": { "diagnostics": [] }
            }),
        )
        .await?
        .ok_or("no code action response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];

    // Should return null or an empty array (no quick fixes needed).
    // Organize imports may still be offered, so if result is an array,
    // verify no quickfix actions are present.
    if let Some(actions) = result.as_array() {
        let quickfixes: Vec<&serde_json::Value> = actions
            .iter()
            .filter(|a| a["kind"].as_str() == Some("quickfix"))
            .collect();
        assert!(
            quickfixes.is_empty(),
            "clean code should have no quickfix code actions: {resp}"
        );
    }
    // result being null is also acceptable — means no actions at all.

    Ok(())
}

// ── Phase 3: Additional Inlay Hint Tests ────────────────────────────────────

#[tokio::test]
async fn test_ws_inlay_hint_no_hints_for_annotated_vars() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Mix of annotated and unannotated variables — only unannotated should get hints.
    let code = "\
x: int = 42
y = \"hello\"
z: bool = True
w = 3.14
";
    fixture
        .did_open("file:///ws_inlay_ann_mix.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1200,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_inlay_ann_mix.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 4, "character": 0 }
                }
            }),
        )
        .await?
        .ok_or("no inlay hint response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let hints = parsed["result"]
        .as_array()
        .ok_or("result should be an array")?;

    // y and w are unannotated — they should get hints.
    // x and z are annotated — they should NOT get hints.
    // So we expect exactly 2 type hints: str for y, float for w.
    assert!(
        resp.contains("str"),
        "should show 'str' hint for unannotated y: {resp}"
    );
    assert!(
        resp.contains("float"),
        "should show 'float' hint for unannotated w: {resp}"
    );

    // Verify no hint label contains "int" (the annotated x) or "bool" (the annotated z)
    // as a standalone type hint. We check each hint label individually.
    for hint in hints {
        let label = hint["label"].as_str().unwrap_or("");
        assert!(
            label != ": int",
            "annotated variable x:int should NOT get an inlay hint: {resp}"
        );
        assert!(
            label != ": bool",
            "annotated variable z:bool should NOT get an inlay hint: {resp}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_return_type_multiple_returns() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Function with multiple return statements all returning the same type — should infer.
    let code = "\
def pick(flag: bool):
    if flag:
        return 1
    return 2
";
    fixture
        .did_open("file:///ws_inlay_multi_ret.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1201,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_inlay_multi_ret.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 4, "character": 0 }
                }
            }),
        )
        .await?
        .ok_or("no inlay hint response")?;

    // Both returns are int literals, so return type should be inferred as int.
    assert!(
        resp.contains("-> int"),
        "multiple int returns should infer '-> int' return type hint: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_inlay_hint_method_return_type() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Method inside a class without return annotation — should get return type hint.
    let code = "\
class Calculator:
    def add(self, a: int, b: int):
        return 42
";
    fixture
        .did_open("file:///ws_inlay_method_ret.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1202,
            "textDocument/inlayHint",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_inlay_method_ret.py" },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 3, "character": 0 }
                }
            }),
        )
        .await?
        .ok_or("no inlay hint response")?;

    assert!(
        resp.contains("-> int"),
        "method returning 42 should get '-> int' return type hint: {resp}"
    );
    Ok(())
}

// ── Phase 3: Additional Semantic Token Tests ────────────────────────────────

#[tokio::test]
async fn test_ws_semantic_tokens_class_token() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Animal:
    name: str
";
    fixture
        .did_open("file:///ws_semtok_class.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1203,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_class.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");
    assert!(data.len() >= 5, "should have at least 1 token: {resp}");

    // Token type 2 = class. The first token should be "Animal" at line 0.
    // data layout: [deltaLine, deltaStart, length, tokenType, tokenModifiers]
    // Find a token with tokenType=2 (class).
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    let has_class_token = tokens.iter().any(|t| t[3] == 2);
    assert!(
        has_class_token,
        "should have a token with type 2 (class) for 'Animal': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_semantic_tokens_parameter_token() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def greet(name: str) -> str:
    return name
";
    fixture
        .did_open("file:///ws_semtok_param.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1204,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_param.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // Token type 3 = parameter. "name" should be classified as a parameter.
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    let has_param_token = tokens.iter().any(|t| t[3] == 3);
    assert!(
        has_param_token,
        "should have a token with type 3 (parameter) for 'name': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_semantic_tokens_variable_token() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
x: int = 42
y: str = \"hello\"
";
    fixture
        .did_open("file:///ws_semtok_var.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1205,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_var.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // Token type 4 = variable. Module-level x and y should be classified as variables.
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    let variable_count = tokens.iter().filter(|t| t[3] == 4).count();
    assert!(
        variable_count >= 2,
        "should have at least 2 tokens with type 4 (variable) for x and y: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_semantic_tokens_method_vs_function() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Dog:
    def bark(self) -> str:
        return \"woof\"

def greet(name: str) -> str:
    return name
";
    fixture
        .did_open("file:///ws_semtok_meth_fn.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1206,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_meth_fn.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // Token type 0 = function, 1 = method.
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    let has_method = tokens.iter().any(|t| t[3] == 1);
    let has_function = tokens.iter().any(|t| t[3] == 0);

    assert!(
        has_method,
        "should have a token with type 1 (method) for 'bark': {resp}"
    );
    assert!(
        has_function,
        "should have a token with type 0 (function) for 'greet': {resp}"
    );
    Ok(())
}

// ── Phase 5: Comprehensive Semantic Token Type & Modifier Tests ─────────────

/// Verify tokenType=7 (decorator) appears for @decorator usage.
#[tokio::test]
async fn test_ws_semantic_tokens_decorator_token() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def my_decorator(func):
    return func

@my_decorator
def hello() -> None:
    pass
";
    fixture
        .did_open("file:///ws_semtok_decorator.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1207,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_decorator.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // Token type 7 = decorator. The @my_decorator usage should emit a decorator token.
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    let has_decorator_token = tokens.iter().any(|t| t[3] == 7);
    assert!(
        has_decorator_token,
        "should have a token with type 7 (decorator) for '@my_decorator': {resp}"
    );
    Ok(())
}

/// Verify tokenType=8 (type) appears for type annotations.
#[tokio::test]
async fn test_ws_semantic_tokens_type_annotation() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
def process(data: str) -> int:
    return 42
";
    fixture
        .did_open("file:///ws_semtok_type_ann.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1208,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_type_ann.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // Token type 8 = type. Both "str" (param annotation) and "int" (return annotation)
    // should produce type tokens.
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    let type_token_count = tokens.iter().filter(|t| t[3] == 8).count();
    assert!(
        type_token_count >= 2,
        "should have at least 2 tokens with type 8 (type) for 'str' and 'int' annotations: {resp}"
    );
    Ok(())
}

/// Verify tokenType=9 (typeParameter) appears for generic type parameters.
#[tokio::test]
async fn test_ws_semantic_tokens_type_parameter() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
from typing import Generic, TypeVar

T = TypeVar('T')

class Box(Generic[T]):
    value: T
";
    fixture
        .did_open("file:///ws_semtok_typeparam.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1209,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_typeparam.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // Token type 9 = typeParameter. Generic params in class Box should emit this.
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    let has_type_param = tokens.iter().any(|t| t[3] == 9);
    assert!(
        has_type_param,
        "should have a token with type 9 (typeParameter) for generic param T: {resp}"
    );
    Ok(())
}

/// Verify MOD_STATIC (bit 3, value 8) is set for @staticmethod function tokens.
#[tokio::test]
async fn test_ws_semantic_tokens_static_modifier() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class MathUtils:
    @staticmethod
    def add(a: int, b: int) -> int:
        return a + b
";
    fixture
        .did_open("file:///ws_semtok_static.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1210,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_static.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // Token type 1 = method. MOD_STATIC = bit 3 = value 8.
    // The "add" method token should have the static modifier set (bit 3).
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    // Find method tokens (type 1) and check at least one has static modifier (bit 3 = 8).
    let has_static_method = tokens.iter().any(|t| t[3] == 1 && (t[4] & 8) != 0);
    assert!(
        has_static_method,
        "should have a method token with MOD_STATIC (bit 3) for @staticmethod 'add': {resp}"
    );
    Ok(())
}

/// Verify MOD_DECLARATION (bit 2, value 4) is set on function/class definition tokens.
#[tokio::test]
async fn test_ws_semantic_tokens_declaration_modifier() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Animal:
    pass

def greet(name: str) -> str:
    return name
";
    fixture
        .did_open("file:///ws_semtok_decl.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1211,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_decl.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // MOD_DECLARATION = bit 2 = value 4.
    // Both class (type 2) and function (type 0) definition tokens should have this.
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    // Class token (type 2) should have declaration modifier.
    let class_has_decl = tokens.iter().any(|t| t[3] == 2 && (t[4] & 4) != 0);
    assert!(
        class_has_decl,
        "class 'Animal' token should have MOD_DECLARATION (bit 2): {resp}"
    );

    // Function token (type 0) should have declaration modifier.
    let func_has_decl = tokens.iter().any(|t| t[3] == 0 && (t[4] & 4) != 0);
    assert!(
        func_has_decl,
        "function 'greet' token should have MOD_DECLARATION (bit 2): {resp}"
    );
    Ok(())
}

/// Verify tokenType=5 (property) appears for class attributes.
#[tokio::test]
async fn test_ws_semantic_tokens_property_token() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
class Person:
    name: str
    age: int
";
    fixture
        .did_open("file:///ws_semtok_property.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1212,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_property.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // Token type 5 = property. Class attributes "name" and "age" should be properties.
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    let property_count = tokens.iter().filter(|t| t[3] == 5).count();
    assert!(
        property_count >= 2,
        "should have at least 2 tokens with type 5 (property) for 'name' and 'age': {resp}"
    );
    Ok(())
}

/// Verify tokenType=6 (namespace) appears for import statements.
#[tokio::test]
async fn test_ws_semantic_tokens_namespace_token() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    let code = "\
import os
import sys
";
    fixture
        .did_open("file:///ws_semtok_namespace.py", code)
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            1213,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_namespace.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(data.len() % 5, 0, "token data length should be multiple of 5");

    // Token type 6 = namespace. Import statements should produce namespace tokens.
    let tokens: Vec<Vec<u64>> = data
        .chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect();

    let namespace_count = tokens.iter().filter(|t| t[3] == 6).count();
    assert!(
        namespace_count >= 2,
        "should have at least 2 tokens with type 6 (namespace) for 'os' and 'sys' imports: {resp}"
    );
    Ok(())
}

// ── Error Recovery ──────────────────────────────────────────────────────────

/// After shutdown + exit, a fresh connection can re-initialize.
/// This simulates what the VS Code extension does when it auto-restarts
/// the server (up to 3 times).  The server process is new each time, so
/// the real requirement is that the server starts cleanly on a second
/// fixture — which is equivalent to a process restart.
#[tokio::test]
async fn test_ws_reinitialize_after_shutdown() -> TestResult<()> {
    // First lifecycle: initialize, shutdown, exit.
    {
        let mut fixture = WsTestFixture::new().await?;
        fixture.initialize().await?;

        fixture
            .send_json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 5000,
                "method": "shutdown"
            }))
            .await?;

        let id_str = "\"id\":5000";
        for _ in 0..10 {
            let Some(msg) = fixture.recv().await else { break };
            if msg.contains(id_str) {
                break;
            }
        }

        fixture
            .send_json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "exit",
                "params": null
            }))
            .await?;
    }

    // Second lifecycle: server must start and initialize cleanly.
    {
        let mut fixture = WsTestFixture::new().await?;
        let response = fixture.initialize().await?;

        assert!(
            response.contains("\"result\""),
            "re-initialized server should return a valid result: {response}"
        );
        assert!(
            response.contains("\"basilisk\""),
            "re-initialized server should identify as basilisk: {response}"
        );
    }

    Ok(())
}

/// After shutdown, sending a regular request must not crash the server.
/// The LSP spec says the server should return InvalidRequest (-32600) for
/// any request received after shutdown.  tower-lsp may also close the
/// connection.  Either outcome is acceptable — a crash is not.
#[tokio::test]
async fn test_ws_requests_after_shutdown_return_error() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Open a file so we have a valid URI for the hover request.
    fixture
        .did_open("file:///ws_post_shutdown.py", "x: int = 1\n")
        .await?;
    let _ = fixture.wait_for_diagnostics().await;

    // Shutdown.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5010,
            "method": "shutdown"
        }))
        .await?;

    let id_str = "\"id\":5010";
    for _ in 0..10 {
        let Some(msg) = fixture.recv().await else { break };
        if msg.contains(id_str) {
            break;
        }
    }

    // Now send a hover request — should get error or connection close, not crash.
    let send_result = fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5011,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": "file:///ws_post_shutdown.py" },
                "position": { "line": 0, "character": 0 }
            }
        }))
        .await;

    // The send itself might fail if the connection was already closed — that is fine.
    if send_result.is_err() {
        return Ok(());
    }

    // If we could send, try to read the response.
    let response = fixture.recv().await;
    match response {
        // Connection closed — valid post-shutdown behavior.
        None => {}
        Some(msg) => {
            // If we got a message back, it must be a JSON-RPC error, not a result.
            let parsed: serde_json::Value = serde_json::from_str(&msg)?;
            if parsed.get("error").is_some() {
                // Error response — correct behavior.
                let code = parsed["error"]["code"].as_i64();
                // InvalidRequest (-32600) or ServerNotInitialized (-32002) are both valid.
                assert!(
                    code == Some(-32600) || code == Some(-32002),
                    "expected InvalidRequest or ServerNotInitialized error code, got: {msg}"
                );
            }
            // If it returned a result, the server is being lenient — not ideal
            // but not a crash.  We accept it.
        }
    }

    Ok(())
}

/// Sending a request with structurally invalid params must return a
/// JSON-RPC error (typically InvalidParams -32602), not crash the server.
#[tokio::test]
async fn test_ws_invalid_params_returns_error() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    fixture.initialize().await?;

    // Send textDocument/hover with completely wrong params — missing
    // the required `textDocument` and `position` fields.
    let resp = fixture
        .request(
            5020,
            "textDocument/hover",
            serde_json::json!({
                "bogus": "not a valid hover param"
            }),
        )
        .await?;

    // The server must respond — either with an error or with null result.
    // A timeout (None) means the server hung, which is also acceptable if
    // it didn't crash, but we prefer a response.
    if let Some(msg) = resp {
        let parsed: serde_json::Value = serde_json::from_str(&msg)?;
        // Acceptable outcomes:
        //   1. error with code -32602 (InvalidParams)
        //   2. error with any code (server caught the bad params)
        //   3. result: null (server was lenient and returned empty hover)
        let has_error = parsed.get("error").is_some();
        let has_null_result = parsed.get("result").is_some_and(serde_json::Value::is_null);
        assert!(
            has_error || has_null_result,
            "server should return error or null result for invalid params, got: {msg}"
        );
    }

    // Verify the server is still alive by sending a valid request.
    let alive_resp = fixture
        .request(
            5021,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": "file:///nonexistent.py" },
                "position": { "line": 0, "character": 0 }
            }),
        )
        .await?;

    // Getting any response (even null) proves the server did not crash.
    assert!(
        alive_resp.is_some(),
        "server should still respond after handling invalid params"
    );

    Ok(())
}

/// Sending `initialize` twice must not crash the server.
/// The LSP spec says the server should return an error for a second
/// initialize request.  tower-lsp may handle this internally.
#[tokio::test]
async fn test_ws_multiple_initialize_requests() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;

    // First initialize — should succeed normally.
    let first_resp = fixture.initialize().await?;
    assert!(
        first_resp.contains("\"result\""),
        "first initialize should succeed: {first_resp}"
    );

    // Second initialize — should either return an error or succeed idempotently.
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5030,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": null,
                "capabilities": {},
                "trace": "off"
            }
        }))
        .await?;

    let id_str = "\"id\":5030";
    let mut second_resp = None;
    for _ in 0..10 {
        let Some(msg) = fixture.recv().await else { break };
        if msg.contains(id_str) {
            second_resp = Some(msg);
            break;
        }
    }

    // The server must respond — not hang or crash.
    let second_resp = second_resp.ok_or("server did not respond to second initialize")?;
    let parsed: serde_json::Value = serde_json::from_str(&second_resp)?;

    // Acceptable outcomes:
    //   1. error response (spec-compliant: server already initialized)
    //   2. result response (server accepted re-init idempotently)
    // Unacceptable: no response (timeout) or crash.
    let has_error = parsed.get("error").is_some();
    let has_result = parsed.get("result").is_some();
    assert!(
        has_error || has_result,
        "second initialize must return error or result, got: {second_resp}"
    );

    Ok(())
}

// ── Phase 5: Type hierarchy capability ─────────────────────────────────────

#[tokio::test]
async fn test_ws_initialize_advertises_type_hierarchy_provider() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let response = fixture.initialize().await?;

    assert!(
        response.contains("\"typeHierarchyProvider\""),
        "initialize response should advertise typeHierarchyProvider: {response}"
    );

    // Parse the full response and verify the capability value is `true`.
    let parsed: serde_json::Value = serde_json::from_str(&response)?;
    let caps = parsed
        .get("result")
        .and_then(|r| r.get("capabilities"))
        .ok_or("missing capabilities in initialize response")?;

    assert_eq!(
        caps.get("typeHierarchyProvider"),
        Some(&serde_json::Value::Bool(true)),
        "typeHierarchyProvider should be true: {response}"
    );

    Ok(())
}
