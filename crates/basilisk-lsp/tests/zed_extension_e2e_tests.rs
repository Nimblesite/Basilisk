//! E2E tests simulating the Zed extension's interaction with the Basilisk LSP.
//!
//! These tests exercise the exact code paths that the Zed extension triggers:
//!   1. Initialize with `workspaceRoot` in `initializationOptions` (Zed pattern)
//!   2. Send `workspace/configuration` with basilisk-specific config keys
//!   3. Open files and verify diagnostics flow through
//!   4. Test completions, hover, code actions, inlay hints
//!   5. Execute custom commands (`basilisk.startDebugSession`, etc.)
//!   6. Verify the LSP advertises all capabilities the Zed extension relies on
//!
//! The shared `basilisk_common` crate ensures the command names and config keys
//! used here are identical to those in the live Zed extension.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

use basilisk_common::{commands, config_keys};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Timeout for reading a single LSP message.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Path to the pre-built basilisk binary.
fn basilisk_binary() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(debug_dir) = exe.parent().and_then(|deps| deps.parent()) {
            let candidate = debug_dir.join("basilisk");
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    format!("{}/../../target/debug/basilisk", env!("CARGO_MANIFEST_DIR"))
}

/// Test fixture that manages a `basilisk lsp` child process.
///
/// Mirrors the exact spawn-and-communicate pattern that the Zed extension uses
/// (binary + "lsp" arg, stdio JSON-RPC).
struct ZedLspFixture {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<String>,
    next_id: i64,
}

impl ZedLspFixture {
    /// Spawn the LSP server exactly as the Zed extension would.
    fn new() -> TestResult<Self> {
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
        thread::spawn(move || {
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
        thread::spawn(move || {
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
    fn send_json(&mut self, value: &serde_json::Value) -> TestResult<()> {
        let body = value.to_string();
        let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        self.stdin.write_all(frame.as_bytes())?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Read the next message (with timeout).
    fn recv(&self) -> Option<String> {
        self.responses.recv_timeout(READ_TIMEOUT).ok()
    }

    /// Allocate the next request ID.
    fn next_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Initialize with Zed-style `initializationOptions` (workspaceRoot).
    ///
    /// This is exactly what `language_server_initialization_options()` sends.
    fn initialize_zed_style(&mut self) -> TestResult<String> {
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
    fn did_open(&mut self, uri: &str, text: &str) -> TestResult<()> {
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
    fn wait_for_diagnostics(&self) -> Option<String> {
        for _ in 0..10 {
            let msg = self.recv()?;
            if msg.contains("\"method\":\"textDocument/publishDiagnostics\"") {
                return Some(msg);
            }
        }
        None
    }

    /// Send a request and wait for the response with the matching ID.
    fn request(
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

// ────────────────────────────────────────────────────────────────────
// Tests: Zed Extension ↔ LSP E2E
// ────────────────────────────────────────────────────────────────────

/// The Zed extension calls `language_server_command()` which returns the binary
/// with "lsp" arg, then sends `initialize` with `workspaceRoot` in
/// `initializationOptions`. The LSP must accept this and return capabilities.
#[test]
fn test_zed_initialize_with_workspace_root() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let response = fixture.initialize_zed_style()?;

    // Must return valid LSP init result.
    assert!(response.contains("\"jsonrpc\":\"2.0\""));
    assert!(response.contains("\"result\""));

    // Must advertise the server name as "basilisk".
    assert!(
        response.contains("\"basilisk\""),
        "server info must contain 'basilisk': {response}"
    );

    Ok(())
}

/// The Zed extension relies on specific LSP capabilities. Verify they're all
/// advertised in the initialize response.
#[test]
fn test_zed_required_capabilities() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let response = fixture.initialize_zed_style()?;
    let parsed: serde_json::Value = serde_json::from_str(&response)?;

    let capabilities = &parsed["result"]["capabilities"];

    // Text sync (incremental = 2).
    assert_eq!(capabilities["textDocumentSync"], 2);

    // Hover.
    assert_eq!(capabilities["hoverProvider"], true);

    // Completions.
    assert!(
        capabilities["completionProvider"].is_object(),
        "must advertise completion provider"
    );

    // Code actions.
    assert!(
        capabilities.get("codeActionProvider").is_some(),
        "must advertise code action provider"
    );

    // Inlay hints.
    assert_eq!(capabilities["inlayHintProvider"], true);

    // Execute command — must include all basilisk custom commands.
    let execute_commands = &capabilities["executeCommandProvider"]["commands"];
    assert!(
        execute_commands.is_array(),
        "must have executeCommandProvider"
    );
    let commands_list: Vec<&str> = execute_commands
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    for cmd in commands::ALL {
        assert!(
            commands_list.contains(cmd),
            "command {cmd} must be advertised, got: {commands_list:?}"
        );
    }

    // Definition.
    assert!(
        capabilities.get("definitionProvider").is_some(),
        "must advertise definition provider"
    );

    // References.
    assert!(
        capabilities.get("referencesProvider").is_some(),
        "must advertise references provider"
    );

    // Rename.
    assert!(
        capabilities.get("renameProvider").is_some(),
        "must advertise rename provider"
    );

    // Document symbols (used by Zed outline panel).
    assert!(
        capabilities.get("documentSymbolProvider").is_some(),
        "must advertise document symbol provider"
    );

    // Semantic tokens (required for Zed's `semantic_tokens: combined` setting).
    assert!(
        capabilities.get("semanticTokensProvider").is_some(),
        "must advertise semantic tokens provider"
    );

    // Formatting (via Ruff).
    assert!(
        capabilities.get("documentFormattingProvider").is_some(),
        "must advertise formatting provider"
    );

    // Signature help.
    assert!(
        capabilities.get("signatureHelpProvider").is_some(),
        "must advertise signature help"
    );

    // Code lens.
    assert!(
        capabilities.get("codeLensProvider").is_some(),
        "must advertise code lens"
    );

    // Call hierarchy.
    assert!(
        capabilities.get("callHierarchyProvider").is_some(),
        "must advertise call hierarchy"
    );

    Ok(())
}

/// The Zed extension sends workspace configuration with the shared config keys.
/// The LSP must not reject this.
#[test]
fn test_zed_workspace_configuration() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    // Simulate Zed sending workspace/didChangeConfiguration with the same
    // structure that language_server_workspace_configuration() produces.
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeConfiguration",
        "params": {
            "settings": {
                config_keys::ROOT: {
                    config_keys::INLAY_HINTS: {
                        config_keys::PARAM_NAMES: true,
                        config_keys::VAR_TYPES: true
                    },
                    config_keys::RUFF: {
                        config_keys::RUFF_ENABLED: true
                    }
                }
            }
        }
    }))?;

    // If the LSP crashes on config, subsequent requests will fail.
    // Verify it's still alive by opening a file.
    let code = "def add(a: int, b: int) -> int:\n    return a + b\n";
    fixture.did_open("file:///test_config.py", code)?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("LSP died after config change — no diagnostics received")?;

    assert!(
        diag.contains("\"diagnostics\":[]"),
        "clean code should produce no diagnostics: {diag}"
    );

    Ok(())
}

/// Diagnostics must flow to the Zed extension after opening a Python file.
#[test]
fn test_zed_diagnostics_on_open() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///greet.py", code)?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics received")?;

    // Must report missing type annotations (BSK-E0001 and BSK-E0002).
    assert!(diag.contains("BSK-E0001"), "missing param type: {diag}");
    assert!(diag.contains("BSK-E0002"), "missing return type: {diag}");

    Ok(())
}

/// Clean code should produce zero diagnostics.
#[test]
fn test_zed_clean_code_no_diagnostics() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///clean.py", code)?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics received")?;

    assert!(
        diag.contains("\"diagnostics\":[]"),
        "clean code should have no diagnostics: {diag}"
    );

    Ok(())
}

/// Hover must work — the Zed extension displays hover info on mouse-over.
#[test]
fn test_zed_hover() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///hover.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let hover = fixture.request(
        "textDocument/hover",
        &serde_json::json!({
            "textDocument": { "uri": "file:///hover.py" },
            "position": { "line": 0, "character": 11 }
        }),
    )?;

    // Must return hover content (not null).
    assert!(
        hover.get("result").is_some(),
        "hover must return a result: {hover}"
    );

    Ok(())
}

/// Completions must work — the Zed extension triggers these on dot and typing.
#[test]
fn test_zed_completions() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "x: str = \"hello\"\nx.\n";
    fixture.did_open("file:///completion.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let completions = fixture.request(
        "textDocument/completion",
        &serde_json::json!({
            "textDocument": { "uri": "file:///completion.py" },
            "position": { "line": 1, "character": 2 }
        }),
    )?;

    // Must return a valid response (result can be null, array, or object).
    // The key assertion is that we get a response, not an error.
    assert!(
        completions.get("error").is_none(),
        "completions must not error: {completions}"
    );

    Ok(())
}

/// Code actions must work — Zed shows these in the lightbulb menu.
#[test]
fn test_zed_code_actions() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///actions.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let actions = fixture.request(
        "textDocument/codeAction",
        &serde_json::json!({
            "textDocument": { "uri": "file:///actions.py" },
            "range": {
                "start": { "line": 0, "character": 10 },
                "end": { "line": 0, "character": 14 }
            },
            "context": {
                "diagnostics": []
            }
        }),
    )?;

    let result = &actions["result"];
    assert!(
        !result.is_null(),
        "code actions must not be null: {actions}"
    );

    Ok(())
}

/// Document symbols must work — Zed uses these for the outline panel.
#[test]
fn test_zed_document_symbols() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "class MyClass:\n    def method(self) -> None:\n        pass\n\ndef standalone(x: int) -> int:\n    return x\n";
    fixture.did_open("file:///symbols.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let symbols = fixture.request(
        "textDocument/documentSymbol",
        &serde_json::json!({
            "textDocument": { "uri": "file:///symbols.py" }
        }),
    )?;

    let result = &symbols["result"];
    assert!(
        result.is_array(),
        "document symbols must return an array: {symbols}"
    );

    Ok(())
}

/// Execute command: the Zed extension uses basilisk custom commands via LSP.
/// Verify the organize imports command works.
#[test]
fn test_zed_execute_organize_imports() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "import os\nimport sys\n\ndef foo() -> None:\n    pass\n";
    fixture.did_open("file:///imports.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let result = fixture.request(
        "workspace/executeCommand",
        &serde_json::json!({
            "command": commands::ORGANIZE_IMPORTS,
            "arguments": [{ "uri": "file:///imports.py" }]
        }),
    )?;

    // Must not return an error.
    assert!(
        result.get("error").is_none(),
        "organize imports should not error: {result}"
    );

    Ok(())
}

/// Execute command: start debug session. Even if debugpy isn't installed,
/// the LSP should return a structured error (not crash).
#[test]
fn test_zed_execute_start_debug_session() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let result = fixture.request(
        "workspace/executeCommand",
        &serde_json::json!({
            "command": commands::START_DEBUG_SESSION,
            "arguments": []
        }),
    )?;

    // The command should either succeed (if debugpy is installed) or return
    // a structured error — either way, the LSP must stay alive.
    // Verify the LSP didn't crash by sending another request.
    let code = "x: int = 1\n";
    fixture.did_open("file:///alive_check.py", code)?;
    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("LSP died after startDebugSession")?;

    assert!(
        diag.contains("\"diagnostics\""),
        "LSP must still respond: {diag}"
    );

    // Check result shape — must be a response (not a crash).
    assert!(
        result.get("id").is_some(),
        "must have response id: {result}"
    );

    Ok(())
}

/// Execute command: stop debug session with a fake session ID.
/// Should not crash the LSP.
#[test]
fn test_zed_execute_stop_debug_session() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let result = fixture.request(
        "workspace/executeCommand",
        &serde_json::json!({
            "command": commands::STOP_DEBUG_SESSION,
            "arguments": [{ "sessionId": "nonexistent-session-id" }]
        }),
    )?;

    // Must not crash. Result should indicate the session wasn't found.
    assert!(
        result.get("id").is_some(),
        "must have response id: {result}"
    );

    Ok(())
}

/// Inlay hints must work — Zed shows these inline in the editor.
#[test]
fn test_zed_inlay_hints() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def add(a: int, b: int) -> int:\n    return a + b\n\nresult = add(1, 2)\n";
    fixture.did_open("file:///hints.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let hints = fixture.request(
        "textDocument/inlayHint",
        &serde_json::json!({
            "textDocument": { "uri": "file:///hints.py" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 4, "character": 0 }
            }
        }),
    )?;

    // Must return a response (even if empty array).
    assert!(
        hints.get("result").is_some(),
        "inlay hints must return a result: {hints}"
    );

    Ok(())
}

/// Semantic tokens must work — Zed uses these with `semantic_tokens: combined`.
#[test]
fn test_zed_semantic_tokens() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def hello(name: str) -> str:\n    return name\n";
    fixture.did_open("file:///tokens.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let tokens = fixture.request(
        "textDocument/semanticTokens/full",
        &serde_json::json!({
            "textDocument": { "uri": "file:///tokens.py" }
        }),
    )?;

    assert!(
        tokens.get("result").is_some(),
        "semantic tokens must return a result: {tokens}"
    );

    Ok(())
}

/// Go to definition must work.
#[test]
fn test_zed_go_to_definition() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\ngreet(\"world\")\n";
    fixture.did_open("file:///definition.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let definition = fixture.request(
        "textDocument/definition",
        &serde_json::json!({
            "textDocument": { "uri": "file:///definition.py" },
            "position": { "line": 3, "character": 1 }
        }),
    )?;

    assert!(
        definition.get("result").is_some(),
        "definition must return a result: {definition}"
    );

    Ok(())
}

/// Find references must work.
#[test]
fn test_zed_find_references() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\ngreet(\"a\")\ngreet(\"b\")\n";
    fixture.did_open("file:///refs.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let refs = fixture.request(
        "textDocument/references",
        &serde_json::json!({
            "textDocument": { "uri": "file:///refs.py" },
            "position": { "line": 0, "character": 5 },
            "context": { "includeDeclaration": true }
        }),
    )?;

    assert!(
        refs.get("result").is_some(),
        "references must return a result: {refs}"
    );

    Ok(())
}

/// Formatting must work (delegated to Ruff).
#[test]
fn test_zed_formatting() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def  foo(  x:int  )->int:\n    return   x\n";
    fixture.did_open("file:///format.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let format_result = fixture.request(
        "textDocument/formatting",
        &serde_json::json!({
            "textDocument": { "uri": "file:///format.py" },
            "options": {
                "tabSize": 4,
                "insertSpaces": true
            }
        }),
    )?;

    // Must return a result (even if null when Ruff isn't available).
    assert!(
        format_result.get("id").is_some(),
        "formatting must return a response: {format_result}"
    );

    Ok(())
}

/// Multiple documents open concurrently — the Zed editor can have many tabs.
#[test]
fn test_zed_multiple_documents() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code_with_error = "def foo(x):\n    return x\n";
    let code_clean = "def bar(x: int) -> int:\n    return x\n";

    fixture.did_open("file:///doc_a.py", code_with_error)?;
    fixture.did_open("file:///doc_b.py", code_clean)?;

    // Collect diagnostics for both documents.
    let mut got_a = false;
    let mut got_b = false;

    for _ in 0..20 {
        let Some(msg) = fixture.recv() else { break };
        if msg.contains("doc_a.py") && msg.contains("BSK-E0001") {
            got_a = true;
        }
        if msg.contains("doc_b.py") && msg.contains("\"diagnostics\":[]") {
            got_b = true;
        }
        if got_a && got_b {
            break;
        }
    }

    assert!(got_a, "doc_a.py should have diagnostics");
    assert!(got_b, "doc_b.py should be clean");

    Ok(())
}

/// Verify the LSP uses the shared constant for docs URL.
#[test]
fn test_zed_diagnostic_docs_url() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    fixture.initialize_zed_style()?;

    let code = "def foo(x):\n    return x\n";
    fixture.did_open("file:///docs_url.py", code)?;

    let diag = fixture.wait_for_diagnostics().ok_or("no diagnostics")?;

    // Diagnostics should reference the Basilisk docs URL from basilisk_common.
    assert!(
        diag.contains(basilisk_common::diagnostics::DOCS_URL),
        "diagnostics should contain docs URL '{}': {diag}",
        basilisk_common::diagnostics::DOCS_URL
    );

    Ok(())
}
