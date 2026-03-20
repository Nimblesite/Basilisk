// Tests for LSP: `lsp_e2e_hierarchies`.

// LSP E2E tests — Call Hierarchy and Type Hierarchy.

use super::lsp_e2e_common::{send_request, LspTestFixture, TestResult};

// ── Call Hierarchy ───────────────────────────────────────────────────────────

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

    let prep_val: serde_json::Value = serde_json::from_str(&prep)?;
    let items = prep_val["result"].as_array().ok_or("no items in prepare")?;
    if items.is_empty() {
        return Ok(());
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

// ── Type Hierarchy ───────────────────────────────────────────────────────────

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
