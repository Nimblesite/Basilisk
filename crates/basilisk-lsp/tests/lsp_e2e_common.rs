//! Shared test infrastructure for stdio-based LSP E2E tests.
//!
//! Each test file imports this module via `mod lsp_e2e_common;` to get
//! the fixture, type alias, and helper functions.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

pub type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Default timeout for reading a single LSP message.
pub const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Path to the pre-built basilisk binary.
///
/// Derives the target directory from the test executable's own location,
/// which works regardless of whether `cargo test` or `cargo llvm-cov`
/// (which uses a different `--target-dir`) invoked us.
pub fn basilisk_binary() -> String {
    // The test binary lives under <target-dir>/debug/deps/...
    // We want <target-dir>/debug/basilisk
    if let Ok(exe) = std::env::current_exe() {
        if let Some(debug_dir) = exe.parent().and_then(|deps| deps.parent()) {
            let candidate = debug_dir.join("basilisk");
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    // Fallback to the original hardcoded path.
    format!("{}/../../target/debug/basilisk", env!("CARGO_MANIFEST_DIR"))
}

/// Test fixture that manages a `basilisk lsp` child process.
pub struct LspTestFixture {
    pub child: Child,
    pub stdin: ChildStdin,
    pub responses: Receiver<String>,
}

impl LspTestFixture {
    /// Spawn the LSP server and start the background reader thread.
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

        // Background reader for stdout: parse LSP frames and push bodies into the channel.
        let _ = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                let mut content_length: Option<usize> = None;

                // Read headers until the blank line separator.
                // Skip leading blank lines that may appear between messages.
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return; // EOF — server exited
                    }
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        if content_length.is_some() {
                            break; // genuine header terminator
                        }
                        continue; // stray blank line before headers — skip
                    }
                    if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                        content_length = rest.trim().parse().ok();
                    }
                }

                let Some(length) = content_length else {
                    continue; // no Content-Length yet — keep reading
                };
                let mut buf = vec![0u8; length];
                if reader.read_exact(&mut buf).is_err() {
                    return;
                }
                if let Ok(body) = String::from_utf8(buf) {
                    if tx.send(body).is_err() {
                        return; // receiver dropped
                    }
                }
            }
        });

        // Background reader for stderr: print to console for debugging
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
        })
    }

    /// Send a `serde_json::Value` as an LSP frame.
    pub fn send_json(&mut self, value: &serde_json::Value) -> TestResult<()> {
        let body = value.to_string();
        let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        self.stdin.write_all(frame.as_bytes())?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Read the next message from the server (with timeout).
    pub fn recv(&self) -> Option<String> {
        self.responses.recv_timeout(READ_TIMEOUT).ok()
    }

    /// Perform the full initialize / initialized handshake.
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

        // Complete the handshake with the required `initialized` notification.
        self.send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }))?;

        // Drain the server's "Basilisk LSP initialized" log message.
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
}

impl Drop for LspTestFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── Shared helper functions ─────────────────────────────────────────────────

/// Send an LSP request and wait for the response matching the given id.
#[allow(clippy::needless_pass_by_value)]
pub fn send_request(
    fixture: &mut LspTestFixture,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> TestResult<Option<String>> {
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    }))?;

    let id_str = format!("\"id\":{id}");
    for _ in 0..10 {
        let Some(msg) = fixture.recv() else { break };
        if msg.contains(&id_str) {
            return Ok(Some(msg));
        }
    }
    Ok(None)
}

/// Helper: send a `textDocument/completion` request and wait for the response.
pub fn request_completion(
    fixture: &mut LspTestFixture,
    uri: &str,
    line: u32,
    character: u32,
    request_id: u64,
) -> TestResult<Option<String>> {
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        }
    }))?;

    let id_str = format!("\"id\":{request_id}");
    for _ in 0..10 {
        let Some(msg) = fixture.recv() else { break };
        if msg.contains(&id_str) {
            return Ok(Some(msg));
        }
    }
    Ok(None)
}
