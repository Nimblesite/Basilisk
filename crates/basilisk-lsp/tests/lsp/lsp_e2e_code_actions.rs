//! Tests for [LSPARCH-FEATURES-CODEACTIONS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-CODEACTIONS
// Tests for LSP: `lsp_e2e_code_actions`.

// LSP E2E tests — Signature Help, Find References, Rename, Inlay Hints,
// and Code Actions.

use super::lsp_e2e_common::{send_request, LspTestFixture, TestResult};

// ── Document Symbols ─────────────────────────────────────────────────────────

#[test]
fn test_lsp_document_symbols() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///symbols.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        40,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///symbols.py" }
        }),
    )?
    .ok_or("no document symbols response")?;

    assert!(
        resp.contains("Animal"),
        "symbols should include class 'Animal': {resp}"
    );
    assert!(
        resp.contains("greet"),
        "symbols should include function 'greet': {resp}"
    );
    assert!(
        resp.contains("\"x\""),
        "symbols should include variable 'x': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_document_symbols_nested_methods() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Calculator:
    value: int
    def add(self, x: int) -> int:
        return self.value + x
    def multiply(self, x: int) -> int:
        return self.value * x
";
    fixture.did_open("file:///nested.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        41,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///nested.py" }
        }),
    )?
    .ok_or("no document symbols response")?;

    assert!(resp.contains("Calculator"), "should contain class: {resp}");
    assert!(resp.contains("add"), "should contain method 'add': {resp}");
    assert!(
        resp.contains("multiply"),
        "should contain method 'multiply': {resp}"
    );
    assert!(
        resp.contains("value"),
        "should contain attribute 'value': {resp}"
    );
    Ok(())
}

// ── Signature Help ───────────────────────────────────────────────────────────

#[test]
fn test_lsp_signature_help() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str, greeting: str) -> str:
    return f\"{greeting}, {name}!\"

result: str = greet(\"world\", \"Hi\")
";
    fixture.did_open("file:///sighel.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        50,
        "textDocument/signatureHelp",
        serde_json::json!({
            "textDocument": { "uri": "file:///sighel.py" },
            "position": { "line": 3, "character": 21 }
        }),
    )?
    .ok_or("no signature help response")?;

    assert!(
        resp.contains("greet"),
        "signature should show function name: {resp}"
    );
    assert!(
        resp.contains("name"),
        "signature should show parameter 'name': {resp}"
    );
    assert!(
        resp.contains("greeting"),
        "signature should show parameter 'greeting': {resp}"
    );
    Ok(())
}

// ── Find All References ──────────────────────────────────────────────────────

#[test]
fn test_lsp_find_references() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

result: str = greet(\"world\")
";
    fixture.did_open("file:///refs.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        60,
        "textDocument/references",
        serde_json::json!({
            "textDocument": { "uri": "file:///refs.py" },
            "position": { "line": 0, "character": 4 },
            "context": { "includeDeclaration": true }
        }),
    )?
    .ok_or("no references response")?;

    let count = resp.matches("refs.py").count();
    assert!(
        count >= 2,
        "should find at least 2 references for 'greet' (found {count}): {resp}"
    );
    Ok(())
}

// ── Rename ───────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_prepare_rename() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///rename.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        70,
        "textDocument/prepareRename",
        serde_json::json!({
            "textDocument": { "uri": "file:///rename.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no prepare rename response")?;

    assert!(
        resp.contains("result"),
        "prepare rename should return a result: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_rename_symbol() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

result: str = greet(\"world\")
";
    fixture.did_open("file:///ren.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        71,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": "file:///ren.py" },
            "position": { "line": 0, "character": 4 },
            "newName": "say_hello"
        }),
    )?
    .ok_or("no rename response")?;

    assert!(
        resp.contains("say_hello"),
        "rename should include new name: {resp}"
    );
    assert!(
        resp.contains("changes"),
        "rename should include workspace changes: {resp}"
    );
    Ok(())
}

// ── Inlay Hints ──────────────────────────────────────────────────────────────

#[test]
fn test_lsp_inlay_hints_variable_types() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "x = 42\ny = \"hello\"\nz = True\n";
    fixture.did_open("file:///inlay.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        80,
        "textDocument/inlayHint",
        serde_json::json!({
            "textDocument": { "uri": "file:///inlay.py" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 3, "character": 0 }
            }
        }),
    )?
    .ok_or("no inlay hint response")?;

    assert!(
        resp.contains("int"),
        "inlay hints should show 'int' for x=42: {resp}"
    );
    assert!(
        resp.contains("LiteralString"),
        "inlay hints should retain LiteralString precision for y=\"hello\": {resp}"
    );
    assert!(
        resp.contains("bool"),
        "inlay hints should show 'bool' for z=True: {resp}"
    );
    Ok(())
}

// ── Semantic Tokens ──────────────────────────────────────────────────────────

#[test]
fn test_lsp_semantic_tokens_full() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///semtok.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        90,
        "textDocument/semanticTokens/full",
        serde_json::json!({
            "textDocument": { "uri": "file:///semtok.py" }
        }),
    )?
    .ok_or("no semantic tokens response")?;

    assert!(
        resp.contains("\"data\""),
        "semantic tokens should contain 'data' array: {resp}"
    );
    assert!(
        resp.contains("result"),
        "semantic tokens should have result: {resp}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    assert_eq!(
        data.len() % 5,
        0,
        "token data length should be multiple of 5"
    );
    assert!(data.len() >= 5, "should have at least 1 token: {resp}");
    Ok(())
}

// ── Code Actions ─────────────────────────────────────────────────────────────

#[test]
fn test_lsp_code_action_missing_param_annotation() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///actions.py", code)?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let e0001 = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some("BSK-0001"))
        .ok_or("no BSK-0001 diagnostic")?;

    let resp = send_request(
        &mut fixture,
        100,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///actions.py" },
            "range": e0001["range"],
            "context": {
                "diagnostics": [e0001]
            }
        }),
    )?
    .ok_or("no code action response")?;

    assert!(
        resp.contains(": Any"),
        "code action should insert ': Any': {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "code action should be a quickfix: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_code_action_missing_return_annotation() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // The returned method call is not inferable, so BSK-0002 fires — an
    // f-string return would infer `-> str` and stay silent
    // ([TYPEINF-FUNC-RETURN]).
    let code = "def greet(name: str):\n    return name.upper()";
    fixture.did_open("file:///retact.py", code)?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let e0002 = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some("BSK-0002"))
        .ok_or("no BSK-0002 diagnostic")?;

    let resp = send_request(
        &mut fixture,
        101,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///retact.py" },
            "range": e0002["range"],
            "context": {
                "diagnostics": [e0002]
            }
        }),
    )?
    .ok_or("no code action response")?;

    assert!(
        resp.contains("-> Any"),
        "code action should insert '-> Any': {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "code action should be a quickfix: {resp}"
    );

    // Verify the edit inserts AFTER the closing `)`, not at the function name.
    // Input: `def greet(name: str):`  — `)` is at column 19.
    let resp_json: serde_json::Value = serde_json::from_str(&resp)?;
    let actions = resp_json["result"]
        .as_array()
        .ok_or("expected result array")?;
    let return_fix = actions
        .iter()
        .find(|a| a["title"].as_str().is_some_and(|t| t.contains("-> Any")))
        .ok_or("no return type fix action")?;
    let edit = &return_fix["edit"]["changes"]["file:///retact.py"][0];
    let start_line = edit["range"]["start"]["line"].as_u64().unwrap_or(u64::MAX);
    let start_char = edit["range"]["start"]["character"]
        .as_u64()
        .unwrap_or(u64::MAX);
    let new_text = edit["newText"].as_str().unwrap_or("");
    assert_eq!(start_line, 0, "edit must be on the function def line");
    assert_eq!(
        start_char, 20,
        "edit must insert at column 20 (after closing paren), not at function name"
    );
    assert_eq!(
        new_text, " -> Any",
        "inserted text must be ' -> Any' (space before arrow)"
    );
    Ok(())
}

#[test]
fn test_lsp_code_action_redundant_annotation_w0050() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "x: int = 42\n";
    fixture.did_open("file:///redundant.py", code)?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let w0050 = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some("BSK-0050"))
        .ok_or("no BSK-0050 diagnostic")?;

    let resp = send_request(
        &mut fixture,
        102,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///redundant.py" },
            "range": w0050["range"],
            "context": {
                "diagnostics": [w0050]
            }
        }),
    )?
    .ok_or("no code action response")?;

    assert!(
        resp.contains("Remove redundant type annotation"),
        "code action should offer to remove redundant annotation: {resp}"
    );
    assert!(
        resp.contains("quickfix"),
        "code action should be a quickfix: {resp}"
    );
    Ok(())
}

// ── Mass Autofix (Fix All in File) ──────────────────────────────────────────
// Exercises [AUTOFIX-MASS] (File scope) and [AUTOFIX-MASS-VSCODE] (the
// `source.fixAll.basilisk` code action + `basilisk.fixFile` command).

#[test]
fn test_lsp_fix_all_in_file_returns_combined_edit() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Two redundant annotations on separate lines — both fixable.
    let code = "x: int = 42\ny: str = \"hello\"\n";
    fixture.did_open("file:///fixall.py", code)?;

    let _ = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    // Request source.fixAll code actions — the server should return a single
    // combined action with edits for both BSK-0050 diagnostics.
    let resp = send_request(
        &mut fixture,
        200,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///fixall.py" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 2, "character": 0 }
            },
            "context": {
                "diagnostics": [],
                "only": ["source.fixAll"]
            }
        }),
    )?
    .ok_or("no fix-all code action response")?;

    assert!(
        resp.contains("Fix all auto-fixable issues"),
        "should return a fix-all action: {resp}"
    );
    assert!(
        resp.contains("source.fixAll.basilisk"),
        "action kind should be source.fixAll.basilisk: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_fix_all_no_fixable_returns_empty() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // All annotations are necessary — nothing to fix.
    let code = "x: list[int] = [1, 2, 3]\n";
    fixture.did_open("file:///nofixall.py", code)?;

    let _ = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    let resp = send_request(
        &mut fixture,
        201,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///nofixall.py" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 1, "character": 0 }
            },
            "context": {
                "diagnostics": [],
                "only": ["source.fixAll"]
            }
        }),
    )?
    .ok_or("no fix-all code action response")?;

    // Should return null result or empty array — no fixable diagnostics.
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    let is_empty = result.is_null() || result.as_array().is_some_and(Vec::is_empty);
    assert!(
        is_empty,
        "fix-all should return null/empty when nothing is fixable: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_fix_file_command() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "x: int = 42\n";
    fixture.did_open("file:///fixcmd.py", code)?;

    let _ = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    let resp = send_request(
        &mut fixture,
        202,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.fixFile",
            "arguments": ["file:///fixcmd.py"]
        }),
    )?
    .ok_or("no fixFile command response")?;

    assert!(
        resp.contains("fixed"),
        "fixFile should return a result with 'fixed' count: {resp}"
    );
    Ok(())
}

// Regression for issue #245 [AUTOFIX-CLASSIFY] / [AUTOFIX-MASS-VSCODE]: every
// LSP fix-all surface must apply Safe fixes only by default. The Unsafe
// BSK-0003 fix (insert `: Any` on an unannotated variable) may only be
// applied by the explicit all-tier command variants (`basilisk.fixFileAll` /
// `basilisk.fixWorkspaceAll`), mirroring the CLI's safe-only default.
#[test]
fn test_lsp_fix_all_defaults_to_safe_fixes_only() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    std::fs::write(
        fixture.workspace_root.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"BSK-0003\" = \"error\"\n\"BSK-0050\" = \"warning\"\n",
    )?;
    let _ = fixture.initialize()?;

    // Line 0: redundant annotation → BSK-0050 (Safe fix: remove `: int`).
    // Line 1: unannotated `None` variable → BSK-0003 (Unsafe fix: insert `: Any`).
    let uri =
        tower_lsp::lsp_types::Url::from_file_path(fixture.workspace_root.join("safe_default.py"))
            .map_err(|()| "fixture path cannot be represented as a URI")?
            .to_string();
    let code = "x: int = 42\ny = None\n";
    fixture.did_open(&uri, code)?;
    let _ = fixture
        .wait_for_diagnostics_matching(|message| {
            message.contains(&uri) && message.contains("BSK-0003") && message.contains("BSK-0050")
        })
        .ok_or("no settled Safe and Unsafe diagnostics published")?;

    // Surface 1: the `source.fixAll` code action must include only Safe fixes.
    let resp = send_request(
        &mut fixture,
        300,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": &uri },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 2, "character": 0 }
            },
            "context": {
                "diagnostics": [],
                "only": ["source.fixAll"]
            }
        }),
    )?
    .ok_or("no fix-all code action response")?;
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let edits = parsed["result"][0]["edit"]["changes"][&uri]
        .as_array()
        .ok_or("fix-all action should carry edits")?;
    assert!(
        edits.iter().all(|e| e["newText"].as_str() != Some(": Any")),
        "source.fixAll must not apply the Unsafe BSK-0003 `: Any` insertion: {resp}"
    );
    assert_eq!(
        edits.len(),
        1,
        "source.fixAll should include exactly the Safe BSK-0050 fix: {resp}"
    );

    // Surfaces 2–3: the plain commands (keybinding / context menu / toolbar)
    // are Safe-only by default; the spec-promised all-tier variants
    // ([AUTOFIX-MASS-VSCODE]) exist and widen to the Unsafe BSK-0003 fix.
    let second_uri =
        tower_lsp::lsp_types::Url::from_file_path(fixture.workspace_root.join("safe_workspace.py"))
            .map_err(|()| "fixture path cannot be represented as a URI")?
            .to_string();
    let third_uri =
        tower_lsp::lsp_types::Url::from_file_path(fixture.workspace_root.join("all_file.py"))
            .map_err(|()| "fixture path cannot be represented as a URI")?
            .to_string();
    let expectations: [(&str, &str, bool, u64); 4] = [
        ("basilisk.fixFile", &uri, false, 1),
        ("basilisk.fixWorkspace", &second_uri, true, 1),
        ("basilisk.fixFileAll", &third_uri, true, 2),
        ("basilisk.fixWorkspaceAll", &uri, false, 2),
    ];
    for ((command, command_uri, open_first, expected_fixed), request_id) in
        expectations.into_iter().zip(301_u64..)
    {
        if open_first {
            fixture.did_open(command_uri, code)?;
            let _ = fixture
                .wait_for_diagnostics()
                .ok_or("no diagnostics published for command fixture")?;
        }
        let resp = send_request(
            &mut fixture,
            request_id,
            "workspace/executeCommand",
            serde_json::json!({ "command": command, "arguments": [command_uri] }),
        )?
        .ok_or("no executeCommand response")?;
        let parsed: serde_json::Value = serde_json::from_str(&resp)?;
        assert_eq!(
            parsed["result"]["fixed"].as_u64(),
            Some(expected_fixed),
            "{command} must fix exactly {expected_fixed} issue(s) — Safe fixes \
             only by default, every fixable rule for the `All` variants: {resp}"
        );
    }
    Ok(())
}

// Implements [AUTOFIX-MASS-OVERVIEW] / [CONFIGEDITOR-OPERATIONS]: command
// arguments are workspace authority boundaries, and accepted edits converge
// the index before the execute-command response is returned.
#[test]
fn test_fix_commands_enforce_workspace_authority_and_converge() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    std::fs::write(
        fixture.workspace_root.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"BSK-0050\" = \"warning\"\n",
    )?;
    let _ = fixture.initialize()?;

    let root_uri =
        tower_lsp::lsp_types::Url::from_file_path(fixture.workspace_root.join("inside.py"))
            .map_err(|()| "fixture path cannot be represented as a URI")?
            .to_string();
    let external_uri = "file:///external_fix_scope.py";
    let code = "x: int = 42\n";
    fixture.did_open(&root_uri, code)?;
    let _ = fixture
        .wait_for_diagnostics_matching(|message| {
            message.contains(&root_uri) && message.contains("BSK-0050")
        })
        .ok_or("no settled diagnostics for in-root document")?;
    fixture.did_open(external_uri, code)?;
    let _ = fixture
        .wait_for_diagnostics_matching(|message| {
            message.contains(external_uri) && message.contains("BSK-0050")
        })
        .ok_or("no settled diagnostics for external document")?;

    let workspace = send_request(
        &mut fixture,
        320,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.fixWorkspace",
            "arguments": []
        }),
    )?
    .ok_or("no fixWorkspace response")?;
    let parsed: serde_json::Value = serde_json::from_str(&workspace)?;
    assert_eq!(parsed["result"]["fixed"].as_u64(), Some(1));
    assert_eq!(parsed["result"]["files"].as_u64(), Some(1));

    let external = send_request(
        &mut fixture,
        321,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.fixFile",
            "arguments": [external_uri]
        }),
    )?
    .ok_or("no external fixFile response")?;
    let parsed: serde_json::Value = serde_json::from_str(&external)?;
    assert_eq!(
        parsed["result"]["fixed"].as_u64(),
        Some(1),
        "workspace fix must leave the external open document unchanged: {external}"
    );

    let malformed = send_request(
        &mut fixture,
        322,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.fixWorkspace",
            "arguments": [{}]
        }),
    )?
    .ok_or("no malformed-root response")?;
    let parsed: serde_json::Value = serde_json::from_str(&malformed)?;
    assert_eq!(parsed["error"]["code"].as_i64(), Some(-32602));

    let outside_disable = send_request(
        &mut fixture,
        323,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.disableRule",
            "arguments": [{
                "rule": "BSK-0050",
                "severity": "off",
                "uri": external_uri
            }]
        }),
    )?
    .ok_or("no outside disableRule response")?;
    let parsed: serde_json::Value = serde_json::from_str(&outside_disable)?;
    assert_eq!(parsed["error"]["code"].as_i64(), Some(-32602));
    Ok(())
}

// ── Fix All by Rule ─────────────────────────────────────────────────────────

#[test]
fn test_lsp_fix_all_by_rule_in_quickfix_menu() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Three redundant annotations — requesting code actions for the first
    // diagnostic should include a "Fix all `BSK-0050`" quickfix action.
    let code = "x: int = 42\ny: str = \"hello\"\nz: bool = True\n";
    fixture.did_open("file:///fixrule.py", code)?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let w0050 = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some("BSK-0050"))
        .ok_or("no BSK-0050 diagnostic")?;

    let resp = send_request(
        &mut fixture,
        210,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///fixrule.py" },
            "range": w0050["range"],
            "context": {
                "diagnostics": [w0050]
            }
        }),
    )?
    .ok_or("no code action response")?;

    assert!(
        resp.contains("Fix all `BSK-0050` in this file"),
        "should contain per-rule fix-all action: {resp}"
    );
    assert!(
        resp.contains("3 fixes"),
        "should fix all 3 BSK-0050 instances: {resp}"
    );
    // Also verify the global fix-all is present.
    assert!(
        resp.contains("Fix all auto-fixable issues"),
        "should also contain global fix-all action: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_fix_all_by_rule_not_shown_for_single_instance() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Only one BSK-0050 — per-rule fix-all should not appear.
    let code = "x: int = 42\n";
    fixture.did_open("file:///fixrule1.py", code)?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    let w0050 = diagnostics
        .iter()
        .find(|d| d["code"].as_str() == Some("BSK-0050"))
        .ok_or("no BSK-0050 diagnostic")?;

    let resp = send_request(
        &mut fixture,
        211,
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": { "uri": "file:///fixrule1.py" },
            "range": w0050["range"],
            "context": {
                "diagnostics": [w0050]
            }
        }),
    )?
    .ok_or("no code action response")?;

    assert!(
        !resp.contains("Fix all `BSK-0050` in this file"),
        "per-rule fix-all should NOT appear for single instance: {resp}"
    );
    Ok(())
}
