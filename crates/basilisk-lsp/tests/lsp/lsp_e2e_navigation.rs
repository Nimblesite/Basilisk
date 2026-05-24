//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Tests for LSP: `lsp_e2e_navigation`.

// LSP E2E tests — Go to Definition, Declaration, and Type Definition.

use super::lsp_e2e_common::{send_request, LspTestFixture, TestResult};

// ── Go to Definition (basic) ─────────────────────────────────────────────────

#[test]
fn test_lsp_goto_definition_function() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///gotodef.py", code)?;
    let _ = fixture.wait_for_diagnostics();

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

// ── Enhanced Go to Definition ────────────────────────────────────────────────

#[test]
fn test_lsp_goto_definition_returns_exact_position() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    fixture.did_open("file:///gotoexact.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        300,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///gotoexact.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no definition response")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "definition result must not be null: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(start["line"], 0, "definition must be on line 0: {resp}");
    assert_eq!(
        start["character"], 4,
        "definition must start at char 4, where 'greet' begins: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_goto_definition_from_call_site() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    fixture.did_open("file:///goto_call.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        301,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///goto_call.py" },
            "position": { "line": 3, "character": 14 }
        }),
    )?
    .ok_or("no definition response from call site")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def from call site must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "goto-def from call should jump to line 0: {resp}"
    );
    assert_eq!(
        start["character"], 4,
        "goto-def from call should land at char 4 where 'greet' is defined: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_goto_definition_class_from_type_annotation() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "class Dog:\n    name: str\n\ndef pet(dog: Dog) -> None:\n    pass\n";
    fixture.did_open("file:///goto_type.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        302,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///goto_type.py" },
            "position": { "line": 3, "character": 13 }
        }),
    )?
    .ok_or("no definition for class used in type annotation")?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on type annotation must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "goto-def should jump to class definition at line 0: {resp}"
    );
    assert_eq!(
        start["character"], 6,
        "goto-def should land at char 6 where 'Dog' is defined: {resp}"
    );
    Ok(())
}

// ── Go to Declaration ────────────────────────────────────────────────────────

#[test]
fn test_lsp_goto_declaration_function() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def compute(x: int) -> int:
    return x * 2

result: int = compute(10)
";
    fixture.did_open("file:///decl.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        200,
        "textDocument/declaration",
        serde_json::json!({
            "textDocument": { "uri": "file:///decl.py" },
            "position": { "line": 3, "character": 16 }
        }),
    )?
    .ok_or("no declaration response")?;

    assert!(
        resp.contains("\"line\":0"),
        "declaration should point to line 0 (function def): {resp}"
    );
    Ok(())
}

// ── Go to Type Definition ────────────────────────────────────────────────────

#[test]
fn test_lsp_goto_type_definition_variable() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class MyData:
    value: int

instance: MyData = MyData()
";
    fixture.did_open("file:///typedef.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        201,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef.py" },
            "position": { "line": 3, "character": 2 }
        }),
    )?
    .ok_or("no type definition response")?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should point to line 0 (class MyData): {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_goto_type_definition_parameter() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Config:
    debug: bool

def process(cfg: Config) -> None:
    pass
";
    fixture.did_open("file:///typedef2.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        202,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef2.py" },
            "position": { "line": 3, "character": 13 }
        }),
    )?
    .ok_or("no type definition response")?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should point to line 0 (class Config): {resp}"
    );
    Ok(())
}
