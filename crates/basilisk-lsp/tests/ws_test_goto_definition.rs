//! Tests for LSP: ws_test_goto_definition.

#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

#[tokio::test]
async fn test_ws_goto_definition_function() -> TestResult<()> {
    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n";
    let (_fixture, resp) = open_and_request(
        "file:///ws_gotodef.py",
        code,
        310,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_gotodef.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )
    .await?;

    assert!(
        resp.contains("ws_gotodef.py"),
        "definition should point to same file: {resp}"
    );

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

    // Hardened: verify URI matches the opened file exactly
    let uri = parsed["result"]["uri"].as_str().unwrap_or("");
    assert_eq!(
        uri, "file:///ws_gotodef.py",
        "definition URI must match the opened document: {resp}"
    );

    // Hardened: verify the range is non-empty (end differs from start)
    let end = &parsed["result"]["range"]["end"];
    assert!(
        start["line"] != end["line"] || start["character"] != end["character"],
        "definition range must be non-empty (start != end): {resp}"
    );

    // Hardened: verify end character is beyond start (for a single-line range)
    if start["line"] == end["line"] {
        assert!(
            end["character"].as_u64() > start["character"].as_u64(),
            "definition end character must be > start character on same line: {resp}"
        );
    }

    // Hardened: verify JSON-RPC envelope
    assert_eq!(
        parsed["jsonrpc"], "2.0",
        "must be valid JSON-RPC 2.0: {resp}"
    );
    assert_eq!(
        parsed["id"], 310,
        "response id must match request id: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_class() -> TestResult<()> {
    let code = "class Dog:\n    name: str\n    def bark(self) -> str:\n        return \"woof\"\n";
    let (_fixture, resp) = open_and_request(
        "file:///ws_gotoclass.py",
        code,
        311,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_gotoclass.py" },
            "position": { "line": 0, "character": 6 }
        }),
    )
    .await?;

    assert!(
        resp.contains("ws_gotoclass.py"),
        "definition should point to same file: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_from_call_site() -> TestResult<()> {
    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    // Line 3: "result: str = greet(\"world\")" — 'g' of call "greet" at character 14.
    let (_fixture, resp) = open_and_request(
        "file:///ws_goto_call.py",
        code,
        312,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_goto_call.py" },
            "position": { "line": 3, "character": 14 }
        }),
    )
    .await?;

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

#[tokio::test]
async fn test_ws_goto_def_no_symbol_returns_null() -> TestResult<()> {
    let code = "x: int = 42\n";
    // Goto definition on whitespace / non-symbol position.
    let (_fixture, resp) = open_and_request(
        "file:///ws_edge_gotodef.py",
        code,
        401,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_edge_gotodef.py" },
            "position": { "line": 5, "character": 0 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "goto-def on non-symbol position should return null: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_class_usage() -> TestResult<()> {
    // "Animal" is defined on line 0, used as a type annotation on line 3.
    let code = "\
class Animal:
    name: str

def greet(pet: Animal) -> str:
    return pet.name
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_goto_class_usage.py",
        code,
        903,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_goto_class_usage.py" },
            // Goto definition on "Animal" in the type annotation on line 3.
            // "def greet(pet: Animal)" — 'A' of "Animal" is at character 15.
            "position": { "line": 3, "character": 15 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on class usage must resolve: {resp}"
    );

    // Should jump to line 0 where "class Animal:" is defined.
    // "class " is 6 chars, so 'Animal' starts at character 6.
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "goto-def from class usage should jump to line 0: {resp}"
    );
    assert_eq!(
        start["character"], 6,
        "goto-def from class usage should land at char 6 where 'Animal' is defined: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_definition_variable() -> TestResult<()> {
    let code = "x: int = 42\n";
    // Goto definition on "x" — line 0, character 0.
    let (_fixture, resp) = open_and_request(
        "file:///ws_goto_var.py",
        code,
        952,
        "textDocument/definition",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_goto_var.py" },
            "position": { "line": 0, "character": 0 }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"] != serde_json::Value::Null,
        "goto-def on variable must resolve: {resp}"
    );
    let start = &parsed["result"]["range"]["start"];
    assert_eq!(
        start["line"], 0,
        "variable definition should be on line 0: {resp}"
    );
    assert_eq!(
        start["character"], 0,
        "variable definition should start at char 0: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_declaration() -> TestResult<()> {
    let code = "\
def compute(x: int) -> int:
    return x * 2

result: int = compute(10)
";
    let (_fixture, resp) = open_and_request(
        "file:///decl.py",
        code,
        300,
        "textDocument/declaration",
        serde_json::json!({
            "textDocument": { "uri": "file:///decl.py" },
            "position": { "line": 3, "character": 16 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"line\":0"),
        "declaration should point to line 0 (function def): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_type_definition_variable() -> TestResult<()> {
    let code = "\
class MyData:
    value: int

instance: MyData = MyData()
";
    let (_fixture, resp) = open_and_request(
        "file:///typedef.py",
        code,
        301,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef.py" },
            "position": { "line": 3, "character": 2 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should point to line 0 (class MyData): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_type_definition_parameter() -> TestResult<()> {
    let code = "\
class Config:
    debug: bool

def process(cfg: Config) -> None:
    pass
";
    let (_fixture, resp) = open_and_request(
        "file:///typedef2.py",
        code,
        302,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef2.py" },
            "position": { "line": 3, "character": 13 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should point to line 0 (class Config): {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_type_definition_optional() -> TestResult<()> {
    let code = "\
class Widget:
    name: str

item: Optional[Widget] = None
";
    let (_fixture, resp) = open_and_request(
        "file:///typedef3.py",
        code,
        303,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef3.py" },
            "position": { "line": 3, "character": 2 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"line\":0"),
        "type definition should unwrap Optional and point to class Widget: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_goto_type_definition_no_annotation_returns_null() -> TestResult<()> {
    let code = "\
x = 42
";
    let (_fixture, resp) = open_and_request(
        "file:///typedef4.py",
        code,
        304,
        "textDocument/typeDefinition",
        serde_json::json!({
            "textDocument": { "uri": "file:///typedef4.py" },
            "position": { "line": 0, "character": 0 }
        }),
    )
    .await?;

    assert!(
        resp.contains("\"result\":null"),
        "type definition for unannotated variable should be null: {resp}"
    );
    Ok(())
}
