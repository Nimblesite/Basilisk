#![allow(dead_code)]

mod lsp_e2e_common;
use lsp_e2e_common::*;

// ── Call Hierarchy ──────────────────────────────────────────────────────────

#[test]
fn test_lsp_prepare_call_hierarchy() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def main() -> None:
    greet(\"world\")
";
    fixture.did_open("file:///callh.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare call hierarchy on 'greet' definition (line 0, char 4)
    let resp = send_request(
        &mut fixture,
        304,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///callh.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no prepareCallHierarchy response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("greet"),
        "should contain 'greet' in call hierarchy: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_call_hierarchy_incoming() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def main() -> None:
    greet(\"world\")
";
    fixture.did_open("file:///callhi.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // First prepare to get the item
    let prep = send_request(
        &mut fixture,
        305,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///callhi.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no prepareCallHierarchy response")?;

    // Parse the item from the prepare response
    let prep_val: serde_json::Value = serde_json::from_str(&prep)?;
    let items = prep_val["result"].as_array().ok_or("no items in prepare")?;
    if items.is_empty() {
        return Ok(()); // No items — skip incoming calls test
    }

    let resp = send_request(
        &mut fixture,
        306,
        "callHierarchy/incomingCalls",
        serde_json::json!({
            "item": items[0]
        }),
    )?
    .ok_or("no incomingCalls response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

#[test]
fn test_lsp_call_hierarchy_outgoing() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def main() -> None:
    greet(\"world\")
";
    fixture.did_open("file:///callho.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare on 'main' (line 3, char 4) which calls 'greet'
    let prep = send_request(
        &mut fixture,
        307,
        "textDocument/prepareCallHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///callho.py" },
            "position": { "line": 3, "character": 4 }
        }),
    )?
    .ok_or("no prepareCallHierarchy response")?;

    let prep_val: serde_json::Value = serde_json::from_str(&prep)?;
    let items = prep_val["result"].as_array().ok_or("no items in prepare")?;
    if items.is_empty() {
        return Ok(());
    }

    let resp = send_request(
        &mut fixture,
        308,
        "callHierarchy/outgoingCalls",
        serde_json::json!({
            "item": items[0]
        }),
    )?
    .ok_or("no outgoingCalls response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ── Type Hierarchy ──────────────────────────────────────────────────────────

#[test]
fn test_lsp_prepare_type_hierarchy() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str
";
    fixture.did_open("file:///typeh.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare type hierarchy on 'Dog' (line 3, char 6)
    let resp = send_request(
        &mut fixture,
        309,
        "textDocument/prepareTypeHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///typeh.py" },
            "position": { "line": 3, "character": 6 }
        }),
    )?
    .ok_or("no prepareTypeHierarchy response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("Dog"),
        "should contain 'Dog' in type hierarchy: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_type_hierarchy_supertypes() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str
";
    fixture.did_open("file:///typehs.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let prep = send_request(
        &mut fixture,
        310,
        "textDocument/prepareTypeHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///typehs.py" },
            "position": { "line": 3, "character": 6 }
        }),
    )?
    .ok_or("no prepareTypeHierarchy response")?;

    let prep_val: serde_json::Value = serde_json::from_str(&prep)?;
    let items = prep_val["result"].as_array().ok_or("no items")?;
    if items.is_empty() {
        return Ok(());
    }

    let resp = send_request(
        &mut fixture,
        311,
        "typeHierarchy/supertypes",
        serde_json::json!({
            "item": items[0]
        }),
    )?
    .ok_or("no supertypes response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("Animal"),
        "supertypes of Dog should include Animal: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_type_hierarchy_subtypes() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str

class Dog(Animal):
    breed: str
";
    fixture.did_open("file:///typehsub.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Prepare on 'Animal' (line 0, char 6)
    let prep = send_request(
        &mut fixture,
        312,
        "textDocument/prepareTypeHierarchy",
        serde_json::json!({
            "textDocument": { "uri": "file:///typehsub.py" },
            "position": { "line": 0, "character": 6 }
        }),
    )?
    .ok_or("no prepareTypeHierarchy response")?;

    let prep_val: serde_json::Value = serde_json::from_str(&prep)?;
    let items = prep_val["result"].as_array().ok_or("no items")?;
    if items.is_empty() {
        return Ok(());
    }

    let resp = send_request(
        &mut fixture,
        313,
        "typeHierarchy/subtypes",
        serde_json::json!({
            "item": items[0]
        }),
    )?
    .ok_or("no subtypes response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("Dog"),
        "subtypes of Animal should include Dog: {resp}"
    );
    Ok(())
}

// ── Workspace Symbols ───────────────────────────────────────────────────────

#[test]
fn test_lsp_workspace_symbol() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str

def greet(name: str) -> str:
    return name
";
    fixture.did_open("file:///wssym.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        314,
        "workspace/symbol",
        serde_json::json!({
            "query": "greet"
        }),
    )?
    .ok_or("no workspace/symbol response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    assert!(
        resp.contains("greet"),
        "workspace symbols should contain 'greet': {resp}"
    );
    Ok(())
}

// ── Document Formatting (via Ruff) ──────────────────────────────────────────

#[test]
fn test_lsp_formatting() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Badly formatted code
    let code = "def   greet( name:str )->str:\n    return    name\n";
    fixture.did_open("file:///fmt.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        315,
        "textDocument/formatting",
        serde_json::json!({
            "textDocument": { "uri": "file:///fmt.py" },
            "options": {
                "tabSize": 4,
                "insertSpaces": true
            }
        }),
    )?
    .ok_or("no formatting response")?;

    // The response should contain either edits or null result (if ruff is not installed).
    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ── Execute Command ─────────────────────────────────────────────────────────

#[test]
fn test_lsp_execute_command_unknown() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let resp = send_request(
        &mut fixture,
        316,
        "workspace/executeCommand",
        serde_json::json!({
            "command": "basilisk.nonExistentCommand",
            "arguments": []
        }),
    )?
    .ok_or("no executeCommand response")?;

    assert!(
        resp.contains("\"result\""),
        "should have a result (null) for unknown command: {resp}"
    );
    Ok(())
}
