//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    // Shared harness methods are each used by SOME but not every test binary;
    // `mod lsp_stdio` compiles into each binary independently, so a helper
    // unused by one is not dead across the suite.
    dead_code
)]
//! Shared harness for end-to-end tests that drive the **real compiled
//! `basilisk` binary** as an LSP server over stdio.
//!
//! Unlike the in-process WebSocket fixture (`basilisk-lsp/tests/lsp/
//! ws_test_common.rs`), this harness exercises the production entry point
//! (`basilisk lsp` → `run_server()`), including its runtime construction —
//! required for bugs that only manifest in the real process (thread stack
//! sizes, PATH hermeticity, process exit behaviour).
//!
//! The server is spawned with `PATH` pointing at an empty directory, so the
//! binary must never need an external tool ([LSPFMT-DECISION]).

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique, process-scoped temp directory path (not created).
pub fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}_{}_{}",
        std::process::id(),
        DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

/// An LSP server subprocess speaking stdio JSON-RPC, spawned with a PATH on
/// which no external binary exists.
pub struct LspProcess {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: i64,
    pub last_capabilities: Value,
}

impl Drop for LspProcess {
    fn drop(&mut self) {
        // Shut the server down via the LSP protocol and wait for a clean
        // exit: the binary may be coverage-instrumented, and a hard kill
        // truncates its .profraw into corrupt, unmergeable profile data.
        // Everything here is best-effort — never panic in Drop.
        for body in [
            r#"{"jsonrpc":"2.0","id":999999,"method":"shutdown","params":null}"#,
            r#"{"jsonrpc":"2.0","method":"exit","params":null}"#,
        ] {
            let framed = format!("Content-Length: {}\r\n\r\n{body}", body.len());
            let _ = self.stdin.write_all(framed.as_bytes());
        }
        let _ = self.stdin.flush();
        for _ in 0..500 {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => break,
            }
        }
        // Unresponsive after 5s — kill as a last resort.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl LspProcess {
    /// Spawn `basilisk lsp` with PATH set to an empty directory, then run the
    /// initialize handshake.
    pub fn start() -> Self {
        Self::start_with(None, &json!(null))
    }

    /// Like [`Self::start`], with a workspace root and/or initializationOptions.
    pub fn start_with(root: Option<&std::path::Path>, initialization_options: &Value) -> Self {
        let empty_path_dir = unique_temp_dir("bsk_lsp_stdio_path");
        std::fs::create_dir_all(&empty_path_dir).expect("create empty PATH dir");

        let mut child = Command::new(env!("CARGO_BIN_EXE_basilisk"))
            .arg("lsp")
            .env("PATH", &empty_path_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn basilisk lsp");

        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");
        let mut lsp = Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            next_id: 1,
            last_capabilities: Value::Null,
        };

        let root_uri = root.map(|p| format!("file://{}", p.to_string_lossy()));
        let init_result = lsp.request(
            "initialize",
            &json!({
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {},
                "initializationOptions": initialization_options,
                "trace": "off"
            }),
        );
        assert!(
            init_result.get("capabilities").is_some(),
            "initialize must return capabilities: {init_result}"
        );
        lsp.last_capabilities = init_result["capabilities"].clone();
        lsp.notify("initialized", &json!({}));
        lsp
    }

    /// Send one framed JSON-RPC message.
    fn send(&mut self, message: &Value) {
        let body = message.to_string();
        let framed = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        self.stdin
            .write_all(framed.as_bytes())
            .expect("write to server stdin");
        self.stdin.flush().expect("flush server stdin");
    }

    pub fn notify(&mut self, method: &str, params: &Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Send a request and block until its response arrives, skipping
    /// server-initiated notifications. Panics after 60s without a response.
    pub fn request(&mut self, method: &str, params: &Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));

        let deadline = Instant::now() + Duration::from_mins(1);
        while Instant::now() < deadline {
            let message = self.read_message();
            if message.get("id").and_then(Value::as_i64) == Some(id) {
                assert!(
                    message.get("error").is_none(),
                    "request {method} returned an error: {message}"
                );
                return message["result"].clone();
            }
        }
        panic!("no response to {method} within 60s");
    }

    /// Block until the server sends the named notification, skipping every
    /// other message. Panics if the deadline passes or the server dies first.
    pub fn wait_for_notification(&mut self, method: &str, timeout: Duration) -> Value {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let message = self.read_message();
            if message.get("method").and_then(Value::as_str) == Some(method) {
                return message;
            }
        }
        panic!("no {method} notification within {timeout:?}");
    }

    /// Read a single Content-Length framed message from the server.
    fn read_message(&mut self) -> Value {
        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .expect("read header line from server");
            if read == 0 {
                // Give the OS a moment to reap the child so the panic can
                // report HOW the server died (e.g. SIGABRT after a stack
                // overflow), not just that stdout closed.
                std::thread::sleep(Duration::from_millis(200));
                let status = self.child.try_wait().ok().flatten();
                panic!("server closed stdout before responding (process status: {status:?})");
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                content_length = value.trim().parse().expect("Content-Length value");
            }
        }
        let mut body = vec![0_u8; content_length];
        self.reader
            .read_exact(&mut body)
            .expect("read message body from server");
        serde_json::from_slice(&body).expect("server sent valid JSON")
    }

    pub fn did_open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            &json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "python",
                    "version": 1,
                    "text": text
                }
            }),
        );
    }

    pub fn code_actions(&mut self, uri: &str) -> Value {
        self.request(
            "textDocument/codeAction",
            &json!({
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 }
                },
                "context": { "diagnostics": [] }
            }),
        )
    }
}
