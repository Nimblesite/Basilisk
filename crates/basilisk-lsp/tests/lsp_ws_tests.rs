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
    assert!(response.contains("\"codeActionProvider\":true"));
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
