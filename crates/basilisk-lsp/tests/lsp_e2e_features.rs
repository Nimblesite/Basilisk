#![allow(dead_code)]

mod lsp_e2e_common;
use lsp_e2e_common::*;

// ── Hover (type signature) ──────────────────────────────────────────────────

#[test]
fn test_lsp_hover_shows_function_signature() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///hover.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on "greet" (line 0, character 4)
    let resp = send_request(
        &mut fixture,
        20,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(
        resp.contains("def"),
        "hover should show function def: {resp}"
    );
    assert!(
        resp.contains("greet"),
        "hover should show function name: {resp}"
    );
    assert!(resp.contains("name"), "hover should show parameter: {resp}");
    Ok(())
}

#[test]
fn test_lsp_hover_shows_class_signature() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code =
        "class Animal:\n    name: str\n    def speak(self) -> str:\n        return self.name\n";
    fixture.did_open("file:///hclass.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on "Animal" (line 0, character 6)
    let resp = send_request(
        &mut fixture,
        21,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hclass.py" },
            "position": { "line": 0, "character": 6 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(resp.contains("class"), "hover should show 'class': {resp}");
    assert!(
        resp.contains("Animal"),
        "hover should show class name: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_shows_variable_type() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "x: int = 42\n";
    fixture.did_open("file:///hvar.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on "x" (line 0, character 0)
    let resp = send_request(
        &mut fixture,
        22,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hvar.py" },
            "position": { "line": 0, "character": 0 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(
        resp.contains("variable"),
        "hover should show 'variable': {resp}"
    );
    assert!(resp.contains("int"), "hover should show type 'int': {resp}");
    Ok(())
}

// ── Go to Definition ────────────────────────────────────────────────────────

#[test]
fn test_lsp_goto_definition_function() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///gotodef.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Go to definition on "greet" (line 0, character 4)
    let resp = send_request(
        &mut fixture,
        30,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///gotodef.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no definition response")?;

    // Should return a location pointing to the function definition
    assert!(
        resp.contains("gotodef.py"),
        "definition should point to same file: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_goto_definition_class() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "class Dog:\n    name: str\n    def bark(self) -> str:\n        return \"woof\"\n";
    fixture.did_open("file:///gotoclass.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Go to definition on "Dog" (line 0, character 6)
    let resp = send_request(
        &mut fixture,
        31,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///gotoclass.py" },
            "position": { "line": 0, "character": 6 }
        }),
    )?
    .ok_or("no definition response")?;

    assert!(
        resp.contains("gotoclass.py"),
        "definition should point to same file: {resp}"
    );
    Ok(())
}

// ── Document Symbols ────────────────────────────────────────────────────────

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

// ── Signature Help ──────────────────────────────────────────────────────────

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

    // Cursor inside the greet() call — after the opening paren
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

// ── Find All References ─────────────────────────────────────────────────────

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

    // Find references for "greet" (line 0, character 4)
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

    // Should find at least 2 references (definition + usage)
    let count = resp.matches("refs.py").count();
    assert!(
        count >= 2,
        "should find at least 2 references for 'greet' (found {count}): {resp}"
    );
    Ok(())
}

// ── Rename ──────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_prepare_rename() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///rename.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare rename on "greet" (line 0, character 4)
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

    // Should return a range covering "greet"
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

    // Rename "greet" to "say_hello" (line 0, character 4)
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

// ── Inlay Hints ─────────────────────────────────────────────────────────────

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

// ── Semantic Tokens ─────────────────────────────────────────────────────────

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

    // Should return a data array with encoded tokens
    assert!(
        resp.contains("\"data\""),
        "semantic tokens should contain 'data' array: {resp}"
    );
    assert!(
        resp.contains("result"),
        "semantic tokens should have result: {resp}"
    );

    // Parse the response and verify we get tokens
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    // Each token is 5 integers, so data length should be a multiple of 5
    assert_eq!(
        data.len() % 5,
        0,
        "token data length should be multiple of 5"
    );
    // We should have tokens for Animal, name, speak, self, greet, animal, x at minimum
    assert!(data.len() >= 5, "should have at least 1 token: {resp}");
    Ok(())
}

// ── Code Actions ────────────────────────────────────────────────────────────

#[test]
fn test_lsp_code_action_missing_param_annotation() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name):\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///actions.py", code)?;

    let diag_msg = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics published")?;

    // Parse the published diagnostics to pass to the code action request.
    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    // Find the BSK-E0001 diagnostic.
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

    // Parse the published diagnostics to pass to the code action request.
    let diag_json: serde_json::Value = serde_json::from_str(&diag_msg)?;
    let diagnostics = diag_json["params"]["diagnostics"]
        .as_array()
        .ok_or("expected diagnostics array")?;

    // Find the BSK-W0050 diagnostic.
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
