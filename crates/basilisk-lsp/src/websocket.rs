//! Implements [LSPARCH-INVOKE]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-INVOKE
//! WebSocket transport for the Basilisk LSP server.
//!
//! Bridges WebSocket frames (one JSON-RPC message per frame, no headers)
//! with tower-lsp's expected `Content-Length`-framed byte streams using
//! an in-memory `DuplexStream` pair.
//!
//! Invalid JSON frames are rejected at the bridge layer: a JSON-RPC
//! `-32700` parse-error response is synthesised and returned to the
//! client without touching tower-lsp (which would otherwise shut down
//! the connection on a parse error).

use std::io;

use futures_util::stream::{self, StreamExt as _};
use futures_util::SinkExt as _;
use tokio::io::{
    AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader, DuplexStream,
};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{header::ORIGIN, StatusCode};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;
use tower_lsp::{LspService, Server};
use tracing::{error, info, warn};

use crate::server::LspServer;

/// Buffer size for the in-memory `DuplexStream` pipe (64 KiB).
const DUPLEX_BUFFER_SIZE: usize = 64 * 1024;

/// Upper bound on the total bytes of a client's handshake request headers.
/// A native editor handshake is well under 1 KiB; bounding it stops a client
/// from forcing unbounded buffering during the handshake. [LSPARCH-INVOKE]
const MAX_HANDSHAKE_HEADER_BYTES: usize = 8 * 1024;

/// Upper bound on a single inbound WebSocket message (one JSON-RPC payload).
/// Generous for real LSP traffic yet caps a localhost peer from exhausting
/// memory with a giant frame; an oversized message closes the connection.
const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

/// The reason a client handshake must be refused, or `None` when it may open.
///
/// A web page must never drive the localhost LSP with the user's filesystem
/// authority: a cross-origin browser WebSocket always carries an `Origin`
/// header and native editor clients never do, so any request presenting
/// `Origin` is refused. The total header size is bounded to cap handshake
/// buffering. Implements [LSPARCH-INVOKE].
fn handshake_rejection_reason(request: &Request) -> Option<&'static str> {
    if request.headers().contains_key(ORIGIN) {
        return Some("Origin header is not permitted");
    }
    let header_bytes: usize = request
        .headers()
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len())
        .sum();
    (header_bytes > MAX_HANDSHAKE_HEADER_BYTES)
        .then_some("handshake headers exceed the permitted size")
}

/// Build a `400 Bad Request` handshake rejection carrying a short reason.
fn reject_handshake(reason: &'static str) -> ErrorResponse {
    let mut response = ErrorResponse::new(Some(reason.to_owned()));
    *response.status_mut() = StatusCode::BAD_REQUEST;
    response
}

/// Handshake callback for `accept_hdr_async_with_config`: open the socket
/// unless [`handshake_rejection_reason`] refuses it.
#[expect(
    clippy::result_large_err,
    reason = "tungstenite's Callback trait fixes the return type as \
              Result<Response, ErrorResponse>; ErrorResponse is the crate's own \
              http::Response and cannot be boxed without breaking the signature"
)]
fn ws_handshake_guard(request: &Request, response: Response) -> Result<Response, ErrorResponse> {
    match handshake_rejection_reason(request) {
        Some(reason) => Err(reject_handshake(reason)),
        None => Ok(response),
    }
}

/// Inject capabilities that `lsp-types 0.94` does not expose in
/// `ServerCapabilities` but that the server does handle.
///
/// Currently adds `typeHierarchyProvider: true`.
///
/// This is a no-op for messages that are not `initialize` responses.
fn inject_missing_capabilities(body: &str) -> String {
    let Ok(mut msg) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.to_owned();
    };

    // Only patch initialize responses that carry `result.capabilities`.
    let Some(caps) = msg
        .get_mut("result")
        .and_then(|r| r.get_mut("capabilities"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return body.to_owned();
    };

    let _ = caps
        .entry("typeHierarchyProvider")
        .or_insert(serde_json::Value::Bool(true));

    // Serialization of a valid `Value` never fails.
    serde_json::to_string(&msg).unwrap_or_else(|_| body.to_owned())
}

/// JSON-RPC parse-error response body (null id, code -32700).
const PARSE_ERROR_BODY: &str =
    r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}"#;

/// Convert an arbitrary error into `io::Error`.
fn ws_err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

/// Read WebSocket text frames and write them as `Content-Length`-framed
/// bytes into the tower-lsp input stream.
///
/// Invalid JSON frames are NOT forwarded to tower-lsp.  Instead a
/// parse-error string is pushed onto `inject_tx` so the outbound task
/// can return it to the client directly.
async fn ws_to_lsp(
    mut ws_read: impl stream::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
    mut lsp_input: DuplexStream,
    inject_tx: mpsc::Sender<String>,
) -> io::Result<()> {
    while let Some(msg_result) = ws_read.next().await {
        let msg = msg_result.map_err(|err| ws_err(format!("ws read: {err}")))?;
        match msg {
            Message::Text(text) => {
                // Validate JSON before forwarding; synthesise a parse error for bad input.
                if serde_json::from_str::<serde_json::Value>(&text).is_err() {
                    let _ = inject_tx.send(PARSE_ERROR_BODY.to_owned()).await;
                    continue;
                }
                let header = format!("Content-Length: {}\r\n\r\n", text.len());
                lsp_input.write_all(header.as_bytes()).await?;
                lsp_input.write_all(text.as_bytes()).await?;
                lsp_input.flush().await?;
            }
            Message::Close(_) => break,
            // Ignore binary, ping, pong frames.
            _ => {}
        }
    }
    Ok(())
}

/// Read one `Content-Length`-framed message body from `reader`.
///
/// Returns `None` on EOF or any read / encoding error.
async fn read_lsp_body(reader: &mut BufReader<DuplexStream>) -> Option<String> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => return None,
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse().ok();
        }
    }
    let length = content_length?;
    let mut body = vec![0u8; length];
    let _ = reader.read_exact(&mut body).await.ok()?;
    String::from_utf8(body).ok()
}

/// Convert a `DuplexStream` of `Content-Length`-framed LSP output into a
/// `Stream` of message body strings.
fn lsp_output_stream(lsp_output: DuplexStream) -> impl stream::Stream<Item = String> {
    stream::unfold(BufReader::new(lsp_output), |mut reader| async move {
        read_lsp_body(&mut reader).await.map(|body| (body, reader))
    })
}

/// Forward messages from either the LSP server output or the error-injection
/// channel to the WebSocket client.
///
/// Using `stream::select` ensures both sources are polled concurrently
/// without cancelling either mid-message.
async fn lsp_to_ws(
    lsp_output: DuplexStream,
    inject_rx: mpsc::Receiver<String>,
    mut ws_write: impl futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
) -> io::Result<()> {
    let lsp_stream = lsp_output_stream(lsp_output);
    // Convert tokio mpsc Receiver to a Stream via unfold.
    let inject_stream = stream::unfold(inject_rx, |mut rx| async move {
        rx.recv().await.map(|text| (text, rx))
    });
    let mut merged = Box::pin(stream::select(lsp_stream, inject_stream));

    while let Some(text) = merged.next().await {
        let patched = inject_missing_capabilities(&text);
        ws_write
            .send(Message::Text(patched.into()))
            .await
            .map_err(|err| ws_err(format!("ws write: {err}")))?;
    }
    Ok(())
}

/// Handle a single WebSocket connection by bridging it to a fresh
/// tower-lsp `Server` instance.
///
/// Three concurrent tasks run via `tokio::select!`:
/// 1. `ws_to_lsp` — reads WS frames, validates JSON, writes `Content-Length`-framed bytes
/// 2. `lsp_to_ws` — reads `Content-Length`-framed bytes + injected errors, writes WS frames
/// 3. `tower_lsp::Server::serve` — the LSP server itself
pub async fn handle_connection(
    ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
) {
    let (ws_write, ws_read) = ws_stream.split();

    let (lsp_input_writer, lsp_input_reader) = tokio::io::duplex(DUPLEX_BUFFER_SIZE);
    let (lsp_output_writer, lsp_output_reader) = tokio::io::duplex(DUPLEX_BUFFER_SIZE);

    let (service, socket) = LspService::build(LspServer::new)
        .custom_method(
            basilisk_common::configuration_editor::SNAPSHOT,
            LspServer::configuration_snapshot,
        )
        .custom_method(
            basilisk_common::configuration_editor::PREVIEW,
            LspServer::preview_configuration_change,
        )
        .custom_method(
            basilisk_common::configuration_editor::APPLY,
            LspServer::apply_configuration_change,
        )
        .custom_method(
            basilisk_common::configuration_editor::OCCURRENCES,
            LspServer::rule_occurrences,
        )
        .custom_method(
            basilisk_common::configuration_editor::TYPESHED_ACTION,
            LspServer::typeshed_action,
        )
        .custom_method(
            basilisk_common::configuration_editor::TYPESHED_DOCUMENT,
            LspServer::typeshed_document,
        )
        .finish();

    // Channel for injecting synthesised responses (e.g. parse errors) into
    // the outbound stream without going through tower-lsp.
    let (inject_tx, inject_rx) = mpsc::channel(16);

    let lsp_server = Server::new(lsp_input_reader, lsp_output_writer, socket).serve(service);
    let inbound = ws_to_lsp(ws_read, lsp_input_writer, inject_tx);
    let outbound = lsp_to_ws(lsp_output_reader, inject_rx, ws_write);

    tokio::select! {
        () = lsp_server => {}
        result = inbound => {
            if let Err(err) = result {
                error!(%err, "ws inbound bridge error");
            }
        }
        result = outbound => {
            if let Err(err) = result {
                error!(%err, "ws outbound bridge error");
            }
        }
    }
}

/// Start the Basilisk LSP server listening for WebSocket connections.
///
/// Binds to `127.0.0.1:{port}` and accepts connections indefinitely.
/// Each connection gets its own `LspServer` instance.
///
/// # Errors
///
/// Returns an `io::Error` if the TCP listener fails to bind or accept.
pub async fn run_server_ws(port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    info!(port, "Basilisk LSP WebSocket server listening");

    // Bound per-message size so a localhost peer cannot exhaust memory with a
    // single giant frame; tungstenite closes the connection past the limit.
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));

    loop {
        let (tcp_stream, _addr) = listener.accept().await?;
        // Run the handshake AND the connection in a spawned task so the accept
        // loop is never blocked by one client's handshake — a slow, oversized, or
        // rejected handshake must not stall (or DoS) acceptance of the others. A
        // rejected or malformed handshake is logged and dropped; only a
        // listener-level accept error propagates and stops the server.
        drop(tokio::spawn(async move {
            match tokio_tungstenite::accept_hdr_async_with_config(
                tcp_stream,
                ws_handshake_guard,
                Some(ws_config),
            )
            .await
            {
                Ok(ws_stream) => handle_connection(ws_stream).await,
                Err(err) => warn!(%err, "rejected WebSocket handshake"),
            }
        }));
    }
}

/// Start the WebSocket LSP server on the given port, blocking.
///
/// Synchronous entry point matching `run_server()` for stdio, on the same
/// analysis-sized stacks ([LSPARCH-ARCH-STACK], GitHub #278).
///
/// # Errors
///
/// Returns an `io::Error` if the Tokio runtime or TCP listener fails.
pub fn run_server_ws_blocking(port: u16) -> io::Result<()> {
    crate::runtime::block_on_with_analysis_stack("basilisk-lsp-ws", move || run_server_ws(port))
}

// Pure-function unit tests (no networking, so they belong here rather than in the
// integration binary that hosts the real-server tests — see tests/websocket_transport.rs).
#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only: unwrap is acceptable in unit tests"
)]
mod unit_tests {
    use super::*;

    #[test]
    fn inject_capabilities_adds_type_hierarchy_to_initialize_result() {
        let body = r#"{"result":{"capabilities":{"hoverProvider":true}}}"#;
        let patched = inject_missing_capabilities(body);
        assert!(
            patched.contains(r#""typeHierarchyProvider":true"#),
            "an initialize result must gain typeHierarchyProvider: {patched}"
        );
        assert!(patched.contains(r#""hoverProvider":true"#), "{patched}");
    }

    #[test]
    fn inject_capabilities_leaves_invalid_and_non_initialize_bodies_untouched() {
        assert_eq!(inject_missing_capabilities("not json"), "not json");
        let no_caps = r#"{"result":{"other":1}}"#;
        assert_eq!(inject_missing_capabilities(no_caps), no_caps);
    }

    #[test]
    fn inject_capabilities_preserves_an_existing_type_hierarchy_flag() {
        // `entry().or_insert` must not overwrite a value the server already set.
        let body = r#"{"result":{"capabilities":{"typeHierarchyProvider":false}}}"#;
        let patched = inject_missing_capabilities(body);
        assert!(
            patched.contains(r#""typeHierarchyProvider":false"#),
            "existing flag must survive: {patched}"
        );
    }

    #[test]
    fn handshake_guard_admits_native_and_refuses_origin_and_oversized_headers() {
        let native = Request::builder().uri("/").body(()).unwrap();
        assert_eq!(handshake_rejection_reason(&native), None);

        let browser = Request::builder()
            .uri("/")
            .header("origin", "https://attacker.example")
            .body(())
            .unwrap();
        assert_eq!(
            handshake_rejection_reason(&browser),
            Some("Origin header is not permitted")
        );

        let oversized = Request::builder()
            .uri("/")
            .header("x-padding", "a".repeat(MAX_HANDSHAKE_HEADER_BYTES + 1))
            .body(())
            .unwrap();
        assert_eq!(
            handshake_rejection_reason(&oversized),
            Some("handshake headers exceed the permitted size")
        );
    }

    #[test]
    fn reject_handshake_is_a_400_carrying_the_reason() {
        let response = reject_handshake("nope");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.body().as_deref(), Some("nope"));
    }
}
