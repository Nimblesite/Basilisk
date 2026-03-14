#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Shared test infrastructure for WebSocket LSP E2E tests.
//!
//! Every WS test file imports this module via `mod ws_test_common;` to get
//! the fixture, type alias, and common helper functions.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

// Re-export shared helpers from the test-utils crate.
pub use basilisk_test_utils::{assert_valid_range, extract_diagnostic, TestResult};

/// Default timeout for receiving a single message from the server.
pub const RECV_TIMEOUT: Duration = Duration::from_secs(10);

// ── Test fixture ────────────────────────────────────────────────────────────

/// Test fixture that runs the LSP WebSocket server in-process.
pub struct WsTestFixture {
    /// The write half of the WebSocket connection.
    pub ws_write: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    /// The read half of the WebSocket connection.
    pub ws_read: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    _server_handle: tokio::task::JoinHandle<()>,
}

impl WsTestFixture {
    /// Start the LSP server on a random port and connect via WebSocket.
    ///
    /// # Errors
    ///
    /// Returns an error if binding, accepting, or connecting fails.
    pub async fn new() -> TestResult<Self> {
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
    ///
    /// # Errors
    ///
    /// Returns an error if the WebSocket send fails.
    pub async fn send_json(&mut self, value: &serde_json::Value) -> TestResult<()> {
        let text = value.to_string();
        self.ws_write.send(Message::Text(text)).await?;
        Ok(())
    }

    /// Receive the next text message with a timeout.
    pub async fn recv(&mut self) -> Option<String> {
        match timeout(RECV_TIMEOUT, self.ws_read.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => Some(text.clone()),
            _ => None,
        }
    }

    /// Perform the full initialize / initialized handshake.
    ///
    /// # Errors
    ///
    /// Returns an error if sending or receiving the handshake messages fails.
    pub async fn initialize(&mut self) -> TestResult<String> {
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
    ///
    /// # Errors
    ///
    /// Returns an error if sending the notification fails.
    pub async fn did_open(&mut self, uri: &str, text: &str) -> TestResult<()> {
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
    pub async fn wait_for_diagnostics(&mut self) -> Option<String> {
        for _ in 0..10 {
            let msg = self.recv().await?;
            if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") {
                return Some(msg);
            }
        }
        None
    }

    /// Send a `textDocument/didClose` notification for `uri`.
    ///
    /// # Errors
    ///
    /// Returns an error if sending the notification fails.
    pub async fn did_close(&mut self, uri: &str) -> TestResult<()> {
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": { "uri": uri }
            }
        }))
        .await
    }

    /// Send a request and wait for a response with a matching id.
    ///
    /// # Errors
    ///
    /// Returns an error if sending the request fails.
    pub async fn request(
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

// ── Shared helper functions ─────────────────────────────────────────────────

/// Initialize + open a file + wait for diagnostics. Returns the fixture.
///
/// # Errors
///
/// Returns an error if initialization, opening, or diagnostics retrieval fails.
pub async fn open_and_diagnose(uri: &str, code: &str) -> TestResult<(WsTestFixture, String)> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;
    fixture.did_open(uri, code).await?;
    let diag = fixture
        .wait_for_diagnostics()
        .await
        .ok_or("no diagnostics published")?;
    Ok((fixture, diag))
}

/// Initialize + open a file + wait for diagnostics + send a request. Returns the response.
///
/// # Errors
///
/// Returns an error if any step of the open-diagnose-request pipeline fails.
pub async fn open_and_request(
    uri: &str,
    code: &str,
    request_id: u64,
    method: &str,
    params: serde_json::Value,
) -> TestResult<(WsTestFixture, String)> {
    let (mut fixture, _diag) = open_and_diagnose(uri, code).await?;
    let resp = fixture
        .request(request_id, method, params)
        .await?
        .ok_or(format!("no response to {method}"))?;
    Ok((fixture, resp))
}

/// Parse published diagnostics and request code actions for a specific
/// diagnostic code. Returns the raw JSON-RPC response string.
///
/// # Errors
///
/// Returns an error if diagnostics parsing or the code action request fails.
pub async fn code_action_for(
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

/// Parse semantic token data into Vec of (deltaLine, deltaStart, length, tokenType, modifiers).
#[must_use]
pub fn parse_semantic_tokens(data: &[serde_json::Value]) -> Vec<Vec<u64>> {
    data.chunks(5)
        .map(|chunk| chunk.iter().map(|v| v.as_u64().unwrap_or(0)).collect())
        .collect()
}

/// Assert that semantic token data is well-formed (multiple of 5, non-negative values).
///
/// # Panics
///
/// Panics if the token data length is not a multiple of 5 or contains negative values.
pub fn assert_valid_semantic_token_data(data: &[serde_json::Value], resp: &str) {
    assert_eq!(
        data.len() % 5,
        0,
        "token data length should be multiple of 5"
    );
    for (idx, value) in data.iter().enumerate() {
        let num = value.as_i64().unwrap_or(-1);
        assert!(
            num >= 0,
            "semantic token data[{idx}] must be non-negative, got {num}: {resp}"
        );
    }
}

/// Create a uniquely-named temporary directory.
#[must_use]
pub fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    std::env::temp_dir().join(format!("{prefix}_{nanos}"))
}

/// Initialize the server with a `rootUri` and analysis mode.
///
/// # Errors
///
/// Returns an error if sending or receiving the handshake messages fails.
pub async fn initialize_with_root(
    fixture: &mut WsTestFixture,
    root_uri: &str,
    analysis_mode: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {},
                "trace": "off",
                "initializationOptions": {
                    "analysisMode": analysis_mode
                }
            }
        }))
        .await?;

    let response = fixture.recv().await.ok_or("no response to initialize")?;

    fixture
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }))
        .await?;

    Ok(response)
}

/// Helper for hover requests: opens a file and sends `textDocument/hover`.
///
/// # Errors
///
/// Returns an error if opening or the hover request fails.
pub async fn hover_at(
    uri: &str,
    code: &str,
    line: u32,
    character: u32,
    request_id: u64,
) -> TestResult<String> {
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        request_id,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        }),
    )
    .await?;
    Ok(resp)
}

/// Helper for semantic token requests.
///
/// # Errors
///
/// Returns an error if opening or the semantic tokens request fails.
pub async fn semantic_tokens_for(
    uri: &str,
    code: &str,
    request_id: u64,
) -> TestResult<(String, Vec<Vec<u64>>)> {
    let (_fixture, resp) = open_and_request(
        uri,
        code,
        request_id,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": uri }
        }),
    )
    .await?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;
    assert_valid_semantic_token_data(data, &resp);
    let tokens = parse_semantic_tokens(data);
    Ok((resp, tokens))
}

/// Helper for diagnostic rule tests: opens code, checks that `rule_code` fires.
///
/// # Errors
///
/// Returns an error if opening or diagnostics retrieval fails.
///
/// # Panics
///
/// Panics if the diagnostic message does not contain any of the expected keywords.
pub async fn assert_rule_fires(
    uri: &str,
    code: &str,
    rule_code: &str,
    message_keywords: &[&str],
) -> TestResult<()> {
    let (_fixture, raw) = open_and_diagnose(uri, code).await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;
    let diag = extract_diagnostic(&json, rule_code).ok_or(format!("{rule_code} not fired"))?;
    assert_valid_range(diag, rule_code);
    if !message_keywords.is_empty() {
        let msg = diag["message"].as_str().unwrap_or("").to_lowercase();
        assert!(
            message_keywords.iter().any(|kw| msg.contains(kw)),
            "message should mention one of {message_keywords:?}: {diag}"
        );
    }
    Ok(())
}

/// Helper for diagnostic rule tests: opens code, checks that `rule_code` does NOT fire.
///
/// # Errors
///
/// Returns an error if opening or diagnostics retrieval fails.
///
/// # Panics
///
/// Panics if the given rule code is unexpectedly present in the diagnostics.
pub async fn assert_rule_clean(uri: &str, code: &str, rule_code: &str) -> TestResult<()> {
    let (_fixture, raw) = open_and_diagnose(uri, code).await?;
    let json: serde_json::Value = serde_json::from_str(&raw)?;
    assert!(
        extract_diagnostic(&json, rule_code).is_none(),
        "clean code should not fire {rule_code}: {raw}"
    );
    Ok(())
}
