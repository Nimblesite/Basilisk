//! Tests for [LSPARCH-TESTING]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-TESTING
// Tests for LSP refactoring code actions.
//
// Exercises all refactoring code actions through the real LSP server:
// extract variable/constant/function, inline variable/function,
// convert Union/Optional/f-string/literals/ternary/NamedTuple,
// implement abstract methods, remove parameter.

use super::ws_test_common::*;

/// Helper: request code actions at a specific range and return the response.
async fn code_actions_at(
    fixture: &mut WsTestFixture,
    uri: &str,
    start_line: u32,
    start_char: u32,
    end_line: u32,
    end_char: u32,
    request_id: u64,
) -> TestResult<String> {
    fixture
        .request(
            request_id,
            "textDocument/codeAction",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": start_line, "character": start_char },
                    "end": { "line": end_line, "character": end_char }
                },
                "context": { "diagnostics": [] }
            }),
        )
        .await?
        .ok_or("no code action response".into())
}

/// Helper: check that a code action with a given title substring exists.
fn has_action(resp: &str, title_fragment: &str) -> bool {
    resp.contains(title_fragment)
}

// ── Extract Variable ─────────────────────────────────────────────────────────

// Exercises [REFACTOR-EXTRACT-VAR] / [REFACTOR-EXTRACT-VAR-ALGO].
#[tokio::test]
async fn test_ws_extract_variable_offered() -> TestResult<()> {
    let uri = "file:///refactor_extract_var.py";
    let code = "result: int = some_func(42) + other_func(7)\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Select `some_func(42)` (chars 14-27).
    let resp = code_actions_at(&mut fixture, uri, 0, 14, 0, 27, 600).await?;

    assert!(
        has_action(&resp, "Extract variable"),
        "should offer extract variable: {resp}"
    );

    Ok(())
}

// ── Extract Constant ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_extract_constant_offered() -> TestResult<()> {
    let uri = "file:///refactor_extract_const.py";
    let code = "import os\n\ndef f() -> int:\n    return 42\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Select `42` on line 3.
    let resp = code_actions_at(&mut fixture, uri, 3, 11, 3, 13, 601).await?;

    assert!(
        has_action(&resp, "Extract constant"),
        "should offer extract constant: {resp}"
    );

    Ok(())
}

// ── Extract Function ─────────────────────────────────────────────────────────

// Exercises [REFACTOR-EXTRACT-FUNC] / [REFACTOR-EXTRACT-FUNC-ALGO].
#[tokio::test]
async fn test_ws_extract_function_offered() -> TestResult<()> {
    let uri = "file:///refactor_extract_func.py";
    let code = "def main() -> None:\n    x: int = 1\n    y: int = x + 1\n    print(y)\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Select lines 1-2 (the two assignments).
    let resp = code_actions_at(&mut fixture, uri, 1, 0, 3, 0, 602).await?;

    assert!(
        has_action(&resp, "Extract function"),
        "should offer extract function: {resp}"
    );

    Ok(())
}

// ── Inline Variable ──────────────────────────────────────────────────────────

// Exercises [REFACTOR-INLINE-VAR] / [REFACTOR-INLINE-VAR-ALGO].
#[tokio::test]
async fn test_ws_inline_variable_offered() -> TestResult<()> {
    let uri = "file:///refactor_inline_var.py";
    let code = "temp = calculate()\nresult = temp + 1\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor on line 0 (the assignment).
    let resp = code_actions_at(&mut fixture, uri, 0, 0, 0, 0, 603).await?;

    assert!(
        has_action(&resp, "Inline variable"),
        "should offer inline variable: {resp}"
    );

    Ok(())
}

// ── Inline Function ──────────────────────────────────────────────────────────

// Exercises [REFACTOR-INLINE-FUNC] / [REFACTOR-INLINE-FUNC-ALGO].
#[tokio::test]
async fn test_ws_inline_function_offered() -> TestResult<()> {
    let uri = "file:///refactor_inline_func.py";
    let code = "def double(x: int) -> int:\n    return x * 2\n\nresult: int = double(5)\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor on `double` call (line 3, char 14).
    let resp = code_actions_at(&mut fixture, uri, 3, 14, 3, 14, 604).await?;

    assert!(
        has_action(&resp, "Inline function"),
        "should offer inline function: {resp}"
    );

    Ok(())
}

// ── Convert Union ────────────────────────────────────────────────────────────

// Exercises [REFACTOR-CONVERT] — Union[X, Y] ↔ X | Y row.
#[tokio::test]
async fn test_ws_convert_union_offered() -> TestResult<()> {
    let uri = "file:///refactor_union.py";
    let code = "from typing import Union\n\nx: Union[int, str] = 1\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor on the Union annotation (line 2).
    let resp = code_actions_at(&mut fixture, uri, 2, 3, 2, 3, 605).await?;

    assert!(
        has_action(&resp, "Union"),
        "should offer Union conversion: {resp}"
    );

    Ok(())
}

// ── Convert Optional ─────────────────────────────────────────────────────────

// Exercises [REFACTOR-CONVERT] — Optional[X] ↔ X | None row.
#[tokio::test]
async fn test_ws_convert_optional_offered() -> TestResult<()> {
    let uri = "file:///refactor_optional.py";
    let code = "from typing import Optional\n\nx: Optional[int] = None\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    let resp = code_actions_at(&mut fixture, uri, 2, 3, 2, 3, 606).await?;

    assert!(
        has_action(&resp, "Optional"),
        "should offer Optional conversion: {resp}"
    );

    Ok(())
}

// ── Convert f-string ─────────────────────────────────────────────────────────

// Exercises [REFACTOR-CONVERT] — f-string ↔ .format() row.
#[tokio::test]
async fn test_ws_convert_fstring_offered() -> TestResult<()> {
    let uri = "file:///refactor_fstring.py";
    let code = "name: str = \"world\"\nx: str = f\"hello {name}\"\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor on the f-string line.
    let resp = code_actions_at(&mut fixture, uri, 1, 9, 1, 9, 607).await?;

    assert!(
        has_action(&resp, ".format()"),
        "should offer f-string to .format() conversion: {resp}"
    );

    Ok(())
}

// ── Convert dict literal ─────────────────────────────────────────────────────

// Exercises [REFACTOR-CONVERT] — dict() ↔ {} row.
#[tokio::test]
async fn test_ws_convert_dict_literal_offered() -> TestResult<()> {
    let uri = "file:///refactor_dict.py";
    let code = "x = dict(a=1, b=2)\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    let resp = code_actions_at(&mut fixture, uri, 0, 4, 0, 4, 608).await?;

    assert!(
        has_action(&resp, "dict"),
        "should offer dict() to literal conversion: {resp}"
    );

    Ok(())
}

// ── Convert list literal ─────────────────────────────────────────────────────

// Exercises [REFACTOR-CONVERT] — list() ↔ [] row.
#[tokio::test]
async fn test_ws_convert_list_literal_offered() -> TestResult<()> {
    let uri = "file:///refactor_list.py";
    let code = "x = list()\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    let resp = code_actions_at(&mut fixture, uri, 0, 4, 0, 4, 609).await?;

    assert!(
        has_action(&resp, "list"),
        "should offer list() to literal conversion: {resp}"
    );

    Ok(())
}

// ── Convert ternary ──────────────────────────────────────────────────────────

// Exercises [REFACTOR-CONVERT] — Ternary ↔ if/else block row.
#[tokio::test]
async fn test_ws_convert_ternary_offered() -> TestResult<()> {
    let uri = "file:///refactor_ternary.py";
    let code = "condition: bool = True\nx: int = 1 if condition else 0\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor on the ternary line.
    let resp = code_actions_at(&mut fixture, uri, 1, 4, 1, 4, 610).await?;

    assert!(
        has_action(&resp, "if/else"),
        "should offer ternary to if/else conversion: {resp}"
    );

    Ok(())
}

// ── Convert NamedTuple ───────────────────────────────────────────────────────

// Exercises [REFACTOR-CONVERT] — Named tuple class ↔ functional row.
#[tokio::test]
async fn test_ws_convert_namedtuple_offered() -> TestResult<()> {
    let uri = "file:///refactor_namedtuple.py";
    let code =
        "from typing import NamedTuple\n\nclass Point(NamedTuple):\n    x: int\n    y: int\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor on the class definition.
    let resp = code_actions_at(&mut fixture, uri, 2, 6, 2, 6, 611).await?;

    assert!(
        has_action(&resp, "NamedTuple") || has_action(&resp, "namedtuple"),
        "should offer NamedTuple conversion: {resp}"
    );

    Ok(())
}

// ── Implement Abstract Methods ───────────────────────────────────────────────

// Exercises [REFACTOR-ABSTRACT] / [REFACTOR-ABSTRACT-ALGO].
#[tokio::test]
async fn test_ws_implement_abstract_methods_offered() -> TestResult<()> {
    let uri = "file:///refactor_abstract.py";
    let code = "from abc import ABC, abstractmethod\n\nclass Base(ABC):\n    @abstractmethod\n    def do_thing(self) -> None:\n        ...\n\nclass Child(Base):\n    pass\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor inside the Child class.
    let resp = code_actions_at(&mut fixture, uri, 7, 6, 7, 6, 612).await?;

    assert!(
        has_action(&resp, "abstract") || has_action(&resp, "Implement"),
        "should offer implement abstract methods: {resp}"
    );

    Ok(())
}

// ── Change Signature: Remove Parameter ───────────────────────────────────────

// Exercises [REFACTOR-SIGNATURE] / [REFACTOR-SIGNATURE-OPS] — Remove parameter.
#[tokio::test]
async fn test_ws_change_signature_remove_param_offered() -> TestResult<()> {
    let uri = "file:///refactor_remove_param.py";
    let code = "def greet(name: str, greeting: str) -> str:\n    return f\"{greeting}, {name}\"\n\nresult: str = greet(\"world\", \"Hello\")\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor on `greeting` parameter (line 0, char 21).
    let resp = code_actions_at(&mut fixture, uri, 0, 21, 0, 29, 613).await?;

    assert!(
        has_action(&resp, "Remove parameter"),
        "should offer remove parameter: {resp}"
    );

    Ok(())
}

// ── Change Signature: Add Parameter ─────────────────────────────────────────

// Exercises [REFACTOR-SIGNATURE] / [REFACTOR-SIGNATURE-OPS] — Add parameter.
#[tokio::test]
async fn test_ws_change_signature_add_param_offered() -> TestResult<()> {
    let uri = "file:///refactor_add_param.py";
    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}\"\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor on the function name (line 0, char 4).
    let resp = code_actions_at(&mut fixture, uri, 0, 4, 0, 4, 614).await?;

    assert!(
        has_action(&resp, "Add parameter"),
        "should offer add parameter: {resp}"
    );

    Ok(())
}

// ── Change Signature: Reorder Parameters ────────────────────────────────────

// Exercises [REFACTOR-SIGNATURE-OPS] — Reorder parameters (def-line only;
// see DEVIATION: callers are not updated).
#[tokio::test]
async fn test_ws_change_signature_reorder_offered() -> TestResult<()> {
    let uri = "file:///refactor_reorder_params.py";
    let code = "def process(zebra: int, apple: int, mango: int) -> int:\n    return zebra + apple + mango\n";

    let (mut fixture, _) = open_and_diagnose(uri, code).await?;

    // Cursor on the function name (line 0, char 4).
    let resp = code_actions_at(&mut fixture, uri, 0, 4, 0, 4, 615).await?;

    assert!(
        has_action(&resp, "Sort parameters"),
        "should offer sort parameters: {resp}"
    );

    Ok(())
}

// ── All refactoring kinds are registered ─────────────────────────────────────

#[tokio::test]
async fn test_ws_server_advertises_refactor_capability() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let resp = fixture.initialize().await?;

    // The server should advertise refactor code action kind.
    assert!(
        resp.contains("refactor"),
        "server should advertise refactor capability: {resp}"
    );

    Ok(())
}
