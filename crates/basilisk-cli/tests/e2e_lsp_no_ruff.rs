//! Tests for [LSPFMT-ENGINE] / [LSPFMT-IMPORTS]. See docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! Formatting and import hygiene must be self-contained in the `basilisk`
//! binary ([LSPFMT-DECISION]): no external `ruff` executable is ever spawned.
//!
//! Every test here launches the real compiled binary as an LSP server over
//! stdio with a `PATH` pointing at an empty directory, so **no `ruff` (or any
//! other tool) is findable**. Before the fix for #254/#261 the server shelled
//! out to `ruff` and silently no-opped in exactly this environment; these
//! tests pin the in-process behavior.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// An LSP server subprocess speaking stdio JSON-RPC, spawned with a PATH on
/// which no `ruff` binary exists.
struct LspProcess {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: i64,
    last_capabilities: Value,
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
    fn start() -> Self {
        Self::start_with(None, &json!(null))
    }

    /// Like [`Self::start`], with a workspace root and/or initializationOptions.
    fn start_with(root: Option<&std::path::Path>, initialization_options: &Value) -> Self {
        let empty_path_dir = std::env::temp_dir().join(format!(
            "bsk_no_ruff_path_{}_{}",
            std::process::id(),
            DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
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

    fn notify(&mut self, method: &str, params: &Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Send a request and block until its response arrives, skipping
    /// server-initiated notifications. Panics after 60s without a response.
    fn request(&mut self, method: &str, params: &Value) -> Value {
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

    /// Read a single Content-Length framed message from the server.
    fn read_message(&mut self) -> Value {
        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .expect("read header line from server");
            assert!(read > 0, "server closed stdout before responding");
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

    fn did_open(&mut self, uri: &str, text: &str) {
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

    fn code_actions(&mut self, uri: &str) -> Value {
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

/// Find the first code action whose title contains `needle` and return the
/// full new text of its single whole-document edit for `uri`.
fn action_new_text(actions: &Value, needle: &str, uri: &str) -> Option<String> {
    let action = actions
        .as_array()?
        .iter()
        .find(|a| a["title"].as_str().is_some_and(|t| t.contains(needle)))?;
    let edits = &action["edit"]["changes"][uri];
    Some(edits.as_array()?.first()?["newText"].as_str()?.to_owned())
}

// ── #254: formatting must work with no ruff binary anywhere ─────────────────

#[test]
fn formatting_works_with_no_ruff_binary_on_path() {
    let mut lsp = LspProcess::start();
    let uri = "file:///no_ruff_fmt.py";
    lsp.did_open(uri, "x=1\ny  =   'two'\n");

    let result = lsp.request(
        "textDocument/formatting",
        &json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    );

    let edits = result
        .as_array()
        .unwrap_or_else(|| panic!("formatting silently no-opped without ruff on PATH: {result}"));
    let new_text = edits[0]["newText"].as_str().expect("newText string");
    assert_eq!(
        new_text, "x = 1\ny = \"two\"\n",
        "embedded formatter must produce ruff-format output"
    );
}

// ── #261: import cleanup must work with no ruff binary anywhere ─────────────

#[test]
fn organize_imports_works_with_no_ruff_binary_on_path() {
    let mut lsp = LspProcess::start();
    let uri = "file:///no_ruff_org.py";
    // Unsorted: stdlib out of order, __future__ not first, third-party mixed in.
    lsp.did_open(
        uri,
        "import sys\nimport requests\nimport os\nfrom __future__ import annotations\n\nprint(os, sys, requests, annotations)\n",
    );

    let actions = lsp.code_actions(uri);
    let new_text = action_new_text(&actions, "Organize imports", uri).unwrap_or_else(|| {
        panic!("organize-imports action missing without ruff on PATH: {actions}")
    });

    // isort semantics: __future__ first, stdlib section, then third-party,
    // one blank line between sections. [LSPFMT-IMPORTS]
    assert_eq!(
        new_text,
        "from __future__ import annotations\n\nimport os\nimport sys\n\nimport requests\n\nprint(os, sys, requests, annotations)\n",
        "organize imports must sort with isort semantics"
    );
}

#[test]
fn split_multi_import_works_with_no_ruff_binary_on_path() {
    let mut lsp = LspProcess::start();
    let uri = "file:///no_ruff_split.py";
    lsp.did_open(uri, "import sys, os\n\nprint(os, sys)\n");

    let actions = lsp.code_actions(uri);
    let new_text = action_new_text(&actions, "multiple imports", uri).unwrap_or_else(|| {
        panic!("split-multi-import action missing without ruff on PATH: {actions}")
    });

    // Ruff E401 parity: one statement per module, original order kept.
    assert_eq!(
        new_text, "import sys\nimport os\n\nprint(os, sys)\n",
        "split must produce one import statement per module"
    );
}

#[test]
fn range_formatting_works_with_no_ruff_binary_on_path() {
    let mut lsp = LspProcess::start();
    let uri = "file:///no_ruff_range.py";
    // Only line 0 is selected; line 2's bad spacing must stay untouched.
    lsp.did_open(uri, "x=1\n\ny  =   2\n");

    let result = lsp.request(
        "textDocument/rangeFormatting",
        &json!({
            "textDocument": { "uri": uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 3 }
            },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    );

    let edits = result.as_array().unwrap_or_else(|| {
        panic!("range formatting silently no-opped without ruff on PATH: {result}")
    });
    let new_text = edits[0]["newText"].as_str().expect("newText string");
    assert_eq!(new_text, "x = 1", "selection must be ruff-formatted");
    assert_eq!(
        edits[0]["range"]["end"]["line"].as_i64(),
        Some(0),
        "the edit must not reach past the selected line: {edits:?}"
    );
}

#[test]
fn formatting_respects_tool_ruff_format_options() {
    // A workspace whose pyproject.toml opts into single quotes.
    let root = std::env::temp_dir().join(format!(
        "bsk_no_ruff_ws_{}_{}",
        std::process::id(),
        DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create workspace root");
    std::fs::write(
        root.join("pyproject.toml"),
        "[tool.ruff]\nline-length = 100\n\n[tool.ruff.format]\nquote-style = \"single\"\n",
    )
    .expect("write pyproject.toml");

    let mut lsp = LspProcess::start_with(Some(&root), &json!(null));
    let uri = "file:///no_ruff_opts.py";
    lsp.did_open(uri, "x = \"double\"\n");

    let result = lsp.request(
        "textDocument/formatting",
        &json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    );

    let edits = result
        .as_array()
        .unwrap_or_else(|| panic!("[tool.ruff.format] quote-style was ignored: {result}"));
    assert_eq!(
        edits[0]["newText"].as_str(),
        Some("x = 'double'\n"),
        "quote-style = single must produce single quotes"
    );
}

#[test]
fn formatter_none_setting_disables_formatting_capabilities() {
    // [LSPFMT-CONFIG]: `basilisk.formatter = "none"` — the server must not
    // advertise formatting, and must answer null if asked anyway.
    let mut lsp = LspProcess::start_with(None, &json!({ "formatter": "none" }));
    assert_eq!(
        lsp.last_capabilities.get("documentFormattingProvider"),
        None,
        "formatter=none must not advertise documentFormattingProvider: {}",
        lsp.last_capabilities
    );
    assert_eq!(
        lsp.last_capabilities.get("documentRangeFormattingProvider"),
        None,
        "formatter=none must not advertise documentRangeFormattingProvider"
    );

    let uri = "file:///no_ruff_none.py";
    lsp.did_open(uri, "x=1\n");
    let result = lsp.request(
        "textDocument/formatting",
        &json!({
            "textDocument": { "uri": uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    );
    assert!(result.is_null(), "formatter=none must not format: {result}");
}

#[test]
fn formatting_capabilities_advertised_by_default() {
    // [LSPFMT-CAPABILITIES]: whole-document AND range formatting by default.
    let lsp = LspProcess::start();
    assert_eq!(
        lsp.last_capabilities.get("documentFormattingProvider"),
        Some(&json!(true)),
        "documentFormattingProvider must be advertised: {}",
        lsp.last_capabilities
    );
    assert_eq!(
        lsp.last_capabilities.get("documentRangeFormattingProvider"),
        Some(&json!(true)),
        "documentRangeFormattingProvider must be advertised (Format Selection)"
    );
}

#[test]
fn expand_wildcard_import_works_with_no_ruff_binary_on_path() {
    let mut lsp = LspProcess::start();
    let uri = "file:///no_ruff_wild.py";
    // `join` and `basename` are used but bound nowhere except the wildcard.
    lsp.did_open(
        uri,
        "from os.path import *\n\nprint(join(\"a\", basename(\"b\")))\n",
    );

    let actions = lsp.code_actions(uri);
    let new_text = action_new_text(&actions, "Expand wildcard", uri).unwrap_or_else(|| {
        panic!("expand-wildcard action missing without ruff on PATH: {actions}")
    });

    // The wildcard is replaced by the names the file actually uses from it.
    assert_eq!(
        new_text, "from os.path import basename, join\n\nprint(join(\"a\", basename(\"b\")))\n",
        "wildcard must expand to the used names, sorted"
    );
}
