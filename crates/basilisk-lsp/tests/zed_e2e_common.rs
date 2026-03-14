//! Shared test infrastructure for Zed extension E2E tests.
//!
//! Each Zed test file imports this module via `mod zed_e2e_common;` to get
//! the fixture, type alias, and helper functions.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

pub use basilisk_common::commands;
pub use basilisk_test_utils::TestResult;

use basilisk_test_utils::basilisk_binary;

/// Timeout for reading a single LSP message.
pub const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Test fixture that manages a `basilisk lsp` child process.
///
/// Mirrors the exact spawn-and-communicate pattern that the Zed extension uses
/// (binary + "lsp" arg, stdio JSON-RPC).
pub struct ZedLspFixture {
    pub child: Child,
    pub stdin: ChildStdin,
    pub responses: Receiver<String>,
    pub next_id: i64,
}

impl ZedLspFixture {
    /// Spawn the LSP server exactly as the Zed extension would.
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
    pub fn send_json(&mut self, value: &serde_json::Value) -> TestResult<()> {
        let body = value.to_string();
        let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        self.stdin.write_all(frame.as_bytes())?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Read the next message (with timeout).
    pub fn recv(&self) -> Option<String> {
        self.responses.recv_timeout(READ_TIMEOUT).ok()
    }

    /// Allocate the next request ID.
    pub fn next_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Initialize with Zed-style `initializationOptions` (workspaceRoot).
    ///
    /// This is exactly what `language_server_initialization_options()` sends.
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
    pub fn wait_for_diagnostics(&self) -> Option<String> {
        for _ in 0..10 {
            let msg = self.recv()?;
            if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") {
                return Some(msg);
            }
        }
        None
    }

    /// Send a request and wait for the response with the matching ID.
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
}

impl Drop for ZedLspFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
