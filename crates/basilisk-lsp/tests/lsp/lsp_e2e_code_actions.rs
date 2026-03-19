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
        resp.contains("str"),
        "inlay hints should show 'str' for y=\"hello\": {resp}"
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
        .find(|d| d["code"].as_str() == Some("BSK-E0001"))
        .ok_or("no BSK-E0001 diagnostic")?;

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

    let code = "def greet(name: str):\n    return f\"Hello, {name}!\"";
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
        .find(|d| d["code"].as_str() == Some("BSK-E0002"))
        .ok_or("no BSK-E0002 diagnostic")?;

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
        resp.contains("-> None"),
        "code action should insert '-> None': {resp}"
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
        .find(|a| a["title"].as_str().is_some_and(|t| t.contains("-> None")))
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
        new_text, " -> None",
        "inserted text must be ' -> None' (space before arrow)"
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
        .find(|d| d["code"].as_str() == Some("BSK-W0050"))
        .ok_or("no BSK-W0050 diagnostic")?;

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
    // combined action with edits for both W0050 diagnostics.
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

// ── Fix All by Rule ─────────────────────────────────────────────────────────

#[test]
fn test_lsp_fix_all_by_rule_in_quickfix_menu() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Three redundant annotations — requesting code actions for the first
    // diagnostic should include a "Fix all `BSK-W0050`" quickfix action.
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
        .find(|d| d["code"].as_str() == Some("BSK-W0050"))
        .ok_or("no BSK-W0050 diagnostic")?;

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
        resp.contains("Fix all `BSK-W0050` in this file"),
        "should contain per-rule fix-all action: {resp}"
    );
    assert!(
        resp.contains("3 fixes"),
        "should fix all 3 W0050 instances: {resp}"
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

    // Only one W0050 — per-rule fix-all should not appear.
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
        .find(|d| d["code"].as_str() == Some("BSK-W0050"))
        .ok_or("no BSK-W0050 diagnostic")?;

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
        !resp.contains("Fix all `BSK-W0050` in this file"),
        "per-rule fix-all should NOT appear for single instance: {resp}"
    );
    Ok(())
}
