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
use tokio_tungstenite::tungstenite::Message;
use tower_lsp::{LspService, Server};
use tracing::{error, info};

use crate::server::LspServer;

/// Buffer size for the in-memory `DuplexStream` pipe (64 KiB).
const DUPLEX_BUFFER_SIZE: usize = 64 * 1024;

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

    caps.entry("typeHierarchyProvider")
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
    reader.read_exact(&mut body).await.ok()?;
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
            .send(Message::Text(patched))
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

    let (service, socket) = LspService::new(LspServer::new);

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

    loop {
        let (tcp_stream, _addr) = listener.accept().await?;
        let ws_stream = tokio_tungstenite::accept_async(tcp_stream)
            .await
            .map_err(|err| ws_err(format!("ws handshake failed: {err}")))?;

        tokio::spawn(async move {
            handle_connection(ws_stream).await;
        });
    }
}

/// Start the WebSocket LSP server on the given port, blocking.
///
/// Synchronous entry point matching `run_server()` for stdio.
///
/// # Errors
///
/// Returns an `io::Error` if the Tokio runtime or TCP listener fails.
pub fn run_server_ws_blocking(port: u16) -> io::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_server_ws(port))
}
