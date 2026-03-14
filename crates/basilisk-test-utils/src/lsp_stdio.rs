//! Stdio-based LSP test fixture.
//!
//! Spawns a `basilisk lsp` child process and communicates via JSON-RPC
//! over stdin/stdout. Used by both standard LSP E2E tests and Zed
//! extension E2E tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

use crate::source::basilisk_binary;
use crate::TestResult;

/// Timeout for reading a single LSP message.
pub const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Test fixture that manages a `basilisk lsp` child process.
///
/// Consolidates the common infrastructure shared by the standard LSP
/// E2E tests and the Zed extension E2E tests.
pub struct LspStdioFixture {
    /// The child process running the LSP server.
    pub child: Child,
    /// Stdin handle for sending JSON-RPC messages to the server.
    pub stdin: ChildStdin,
    /// Channel receiver for messages parsed from stdout.
    pub responses: Receiver<String>,
    /// Auto-incrementing request ID counter.
    pub next_id: i64,
}

impl LspStdioFixture {
    /// Spawn the LSP server and start background reader threads.
    ///
    /// # Errors
    /// Returns an error if the server process fails to spawn.
    pub fn new() -> TestResult<Self> {
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

        // Background reader for stdout: parse LSP frames.
        let _ = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                let mut content_length: Option<usize> = None;
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        if content_length.is_some() {
                            break;
                        }
                        continue;
                    }
                    if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                        content_length = rest.trim().parse().ok();
                    }
                }
                let Some(length) = content_length else {
                    continue;
                };
                let mut buf = vec![0u8; length];
                if reader.read_exact(&mut buf).is_err() {
                    return;
                }
                if let Ok(body) = String::from_utf8(buf) {
                    if tx.send(body).is_err() {
                        return;
                    }
                }
            }
        });

        // Drain stderr to console.
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
            next_id: 1,
        })
    }

    /// Send a JSON-RPC message.
    ///
    /// # Errors
    /// Returns an error if writing to stdin fails.
    pub fn send_json(&mut self, value: &serde_json::Value) -> TestResult<()> {
        let body = value.to_string();
        let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        self.stdin.write_all(frame.as_bytes())?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Read the next message (with timeout).
    #[must_use]
    pub fn recv(&self) -> Option<String> {
        self.responses.recv_timeout(READ_TIMEOUT).ok()
    }

    /// Allocate the next request ID.
    pub fn next_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Perform the standard initialize / initialized handshake.
    ///
    /// # Errors
    /// Returns an error if the handshake fails or no response is received.
    pub fn initialize(&mut self) -> TestResult<String> {
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

        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }))?;

        // Drain the server's log message.
        let _ = self.responses.recv_timeout(Duration::from_millis(500));

        Ok(response)
    }

    /// Initialize with Zed-style `initializationOptions` (workspaceRoot).
    ///
    /// This is exactly what `language_server_initialization_options()` sends.
    ///
    /// # Errors
    /// Returns an error if the handshake fails or no response is received.
    pub fn initialize_zed_style(&mut self) -> TestResult<String> {
        let id = self.next_id();
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": null,
                "capabilities": {},
                "initializationOptions": {
                    "workspaceRoot": "/tmp/basilisk-zed-test"
                },
                "trace": "off"
            }
        }))?;

        // The server may send log/notification messages before the init
        // response. Search by ID to find the actual response.
        let id_str = format!("\"id\":{id}");
        let mut response = None;
        for _ in 0..20 {
            let Some(msg) = self.recv() else { break };
            if msg.contains(&id_str) {
                response = Some(msg);
                break;
            }
        }
        let response = response.ok_or("no response to initialize")?;

        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }))?;

        // Drain any log messages from initialization.
        let _ = self.responses.recv_timeout(Duration::from_millis(500));
        let _ = self.responses.recv_timeout(Duration::from_millis(500));

        Ok(response)
    }

    /// Send `textDocument/didOpen`.
    ///
    /// # Errors
    /// Returns an error if writing to stdin fails.
    pub fn did_open(&mut self, uri: &str, text: &str) -> TestResult<()> {
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
    #[must_use]
    pub fn wait_for_diagnostics(&self) -> Option<String> {
        for _ in 0..10 {
            let msg = self.recv()?;
            if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") {
                return Some(msg);
            }
        }
        None
    }

    /// Send a request with an explicit ID and wait for the matching response.
    ///
    /// Returns `None` if the response is not received within the timeout.
    ///
    /// # Errors
    /// Returns an error if writing the request fails.
    #[allow(clippy::needless_pass_by_value)]
    pub fn send_request(
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
        }))?;

        let id_str = format!("\"id\":{id}");
        for _ in 0..10 {
            let Some(msg) = self.recv() else { break };
            if msg.contains(&id_str) {
                return Ok(Some(msg));
            }
        }
        Ok(None)
    }

    /// Send a request with auto-incremented ID and return parsed JSON.
    ///
    /// # Errors
    /// Returns an error if the request fails or no response is received.
    pub fn request(
        &mut self,
        method: &str,
        params: &serde_json::Value,
    ) -> TestResult<serde_json::Value> {
        let id = self.next_id();
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }))?;

        let id_str = format!("\"id\":{id}");
        for _ in 0..20 {
            let Some(msg) = self.recv() else {
                return Err("timeout waiting for response".into());
            };
            if msg.contains(&id_str) {
                return Ok(serde_json::from_str(&msg)?);
            }
        }
        Err(format!("no response found for id {id}").into())
    }

    /// Send a `textDocument/completion` request and wait for the response.
    ///
    /// # Errors
    /// Returns an error if the request fails.
    pub fn request_completion(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        request_id: u64,
    ) -> TestResult<Option<String>> {
        self.send_request(
            request_id,
            "textDocument/completion",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
        )
    }
}

impl Drop for LspStdioFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
