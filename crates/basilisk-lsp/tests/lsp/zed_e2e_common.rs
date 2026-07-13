//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Shared test infrastructure for Zed extension E2E tests.
//
// Each Zed test file imports this module via `mod zed_e2e_common;` to get
// the fixture, type alias, and helper functions.

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
    /// The LSP server child process.
    pub child: Child,
    /// Stdin handle for sending JSON-RPC messages.
    pub stdin: ChildStdin,
    /// Channel receiving parsed JSON-RPC response bodies.
    pub responses: Receiver<String>,
    /// Auto-incrementing request ID counter.
    pub next_id: i64,
    /// Temp workspace root opened during initialize, shipping a `pyproject.toml`
    /// whose `[tool.basilisk.rules]` opts into the annotation house rules (off
    /// by default — the default config is pure PEP conformance). Documents fall
    /// back to this root's config. No modes; configuration.
    /// See [CHKARCH-CONFIGURATION-ONLY].
    pub workspace_root: std::path::PathBuf,
}

impl ZedLspFixture {
    /// Spawn the LSP server exactly as the Zed extension would.
    ///
    /// # Errors
    /// Returns an error if the binary cannot be spawned or stdio handles are unavailable.
    pub fn new() -> TestResult<Self> {
        // Per-process sequence for unique temp workspace names.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

        // Create a temp workspace that opts into the annotation house rules so
        // documents (which fall back to the root's checker config) see them.
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let workspace_root =
            std::env::temp_dir().join(format!("bsk_zed_fixture_{}_{seq}", std::process::id()));
        std::fs::create_dir_all(&workspace_root)?;
        std::fs::write(
            workspace_root.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-E0001\" = \"error\"\n\"BSK-E0002\" = \"error\"\n",
        )?;

        Ok(Self {
            child,
            stdin,
            responses: rx,
            next_id: 1,
            workspace_root,
        })
    }

    /// Send a JSON-RPC message.
    ///
    /// # Errors
    /// Returns an error if writing to stdin or flushing fails.
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
    #[must_use]
    pub fn next_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Initialize with Zed-style `initializationOptions` (workspaceRoot).
    ///
    /// This is exactly what `language_server_initialization_options()` sends.
    ///
    /// # Errors
    /// Returns an error if writing the init request fails or no response is received.
    pub fn initialize_zed_style(&mut self) -> TestResult<String> {
        let id = self.next_id();
        // Point both rootUri and the Zed-style workspaceRoot option at the
        // configured temp workspace so documents resolve to a config with the
        // annotation house rules enabled. See [CHKARCH-CONFIGURATION-ONLY].
        let root_path = self.workspace_root.to_string_lossy().into_owned();
        let root_uri = format!("file://{root_path}");
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {},
                "initializationOptions": {
                    "workspaceRoot": root_path
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

    /// Send a request and wait for the response with the matching ID.
    ///
    /// # Errors
    /// Returns an error if writing the request fails or no matching response is received.
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
        let _ = std::fs::remove_dir_all(&self.workspace_root);
    }
}
