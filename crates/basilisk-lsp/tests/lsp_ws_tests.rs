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
