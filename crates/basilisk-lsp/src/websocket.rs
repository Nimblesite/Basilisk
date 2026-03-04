//! WebSocket transport for the Basilisk LSP server.
//!
//! Bridges WebSocket frames (one JSON-RPC message per frame, no headers)
//! with tower-lsp's expected `Content-Length`-framed byte streams using
//! an in-memory `DuplexStream` pair.

use std::io;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, DuplexStream};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tower_lsp::{LspService, Server};

use crate::server::LspServer;

/// Buffer size for the in-memory `DuplexStream` pipe (64 KiB).
const DUPLEX_BUFFER_SIZE: usize = 64 * 1024;

/// Convert an arbitrary error into `io::Error`.
fn ws_err(msg: impl Into<String>) -> io::Error {
    io::Error::other(msg.into())
}

/// Read WebSocket text frames and write them as `Content-Length`-framed
/// bytes into the tower-lsp input stream.
///
/// Each WS text message is a single JSON-RPC message body. This function
/// prepends the `Content-Length: N\r\n\r\n` header that tower-lsp expects.
async fn ws_to_lsp(
    mut ws_read: impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
    mut lsp_input: DuplexStream,
) -> io::Result<()> {
    while let Some(msg_result) = ws_read.next().await {
        let msg = msg_result.map_err(|err| ws_err(format!("ws read: {err}")))?;
        match msg {
            Message::Text(text) => {
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

/// Read `Content-Length`-framed output from tower-lsp and send each
/// JSON-RPC message body as a WebSocket text frame.
async fn lsp_to_ws(
    lsp_output: DuplexStream,
    mut ws_write: impl SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
) -> io::Result<()> {
    let mut reader = BufReader::new(lsp_output);

    loop {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                return Ok(()); // EOF — server finished
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break; // End of headers
            }
            if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                content_length = rest.trim().parse().ok();
            }
        }

        let Some(length) = content_length else {
            return Err(ws_err("missing Content-Length in LSP output"));
        };

        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).await?;

        let text =
            String::from_utf8(body).map_err(|err| ws_err(format!("lsp body not utf-8: {err}")))?;

        ws_write
            .send(Message::Text(text))
            .await
            .map_err(|err| ws_err(format!("ws write: {err}")))?;
    }
}

/// Handle a single WebSocket connection by bridging it to a fresh
/// tower-lsp `Server` instance.
///
/// Three concurrent tasks run via `tokio::select!`:
/// 1. `ws_to_lsp` — reads WS frames, writes `Content-Length`-framed bytes
/// 2. `lsp_to_ws` — reads `Content-Length`-framed bytes, writes WS frames
/// 3. `tower_lsp::Server::serve` — the LSP server itself
pub async fn handle_connection(
    ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
) {
    let (ws_write, ws_read) = ws_stream.split();

    let (lsp_input_writer, lsp_input_reader) = tokio::io::duplex(DUPLEX_BUFFER_SIZE);
    let (lsp_output_writer, lsp_output_reader) = tokio::io::duplex(DUPLEX_BUFFER_SIZE);

    let (service, socket) = LspService::new(LspServer::new);

    let lsp_server = Server::new(lsp_input_reader, lsp_output_writer, socket).serve(service);
    let inbound = ws_to_lsp(ws_read, lsp_input_writer);
    let outbound = lsp_to_ws(lsp_output_reader, ws_write);

    tokio::select! {
        () = lsp_server => {}
        result = inbound => {
            if let Err(err) = result {
                eprintln!("ws inbound bridge error: {err}");
            }
        }
        result = outbound => {
            if let Err(err) = result {
                eprintln!("ws outbound bridge error: {err}");
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
    eprintln!("Basilisk LSP WebSocket server listening on ws://127.0.0.1:{port}");

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
