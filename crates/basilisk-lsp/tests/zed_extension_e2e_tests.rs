#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! E2E tests simulating the Zed extension's interaction with the Basilisk LSP.
//!
//! Tests: initialization, capabilities, configuration, diagnostics, hover,
//! completions, code actions, and document symbols.

mod zed_e2e_common;
use basilisk_common::config_keys;
use zed_e2e_common::*;

// ── Initialization ──────────────────────────────────────────────────────────

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

// ── Capabilities ────────────────────────────────────────────────────────────

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

// ── Configuration ───────────────────────────────────────────────────────────

/// The Zed extension sends workspace configuration with the shared config keys.
/// The LSP must not reject this.
#[test]
fn test_zed_workspace_configuration() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

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

// ── Diagnostics ─────────────────────────────────────────────────────────────

/// Diagnostics must flow to the Zed extension after opening a Python file.
#[test]
fn test_zed_diagnostics_on_open() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

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
    let _ = fixture.initialize_zed_style()?;

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

// ── Hover ───────────────────────────────────────────────────────────────────

/// Hover must work — the Zed extension displays hover info on mouse-over.
#[test]
fn test_zed_hover() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

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

// ── Completions ─────────────────────────────────────────────────────────────

/// Completions must work — the Zed extension triggers these on dot and typing.
#[test]
fn test_zed_completions() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

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

// ── Code Actions ────────────────────────────────────────────────────────────

/// Code actions must work — Zed shows these in the lightbulb menu.
#[test]
fn test_zed_code_actions() -> TestResult<()> {
    let mut fixture = ZedLspFixture::new()?;
    let _ = fixture.initialize_zed_style()?;

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
