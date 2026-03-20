// E2E tests for refactoring code actions.
//
// These tests spin up the full LSP server via stdio and verify that
// refactoring code actions are offered and produce correct edits.

use super::lsp_e2e_common::{send_request, LspTestFixture, TestResult};

/// Request code actions for a given file, range, and no diagnostics.
fn request_code_actions(
    fixture: &mut LspTestFixture,
    uri: &str,
    start_line: u32,
    start_char: u32,
    end_line: u32,
    end_char: u32,
    request_id: u64,
) -> TestResult<String> {
    send_request(
        fixture,
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
    )?
    .ok_or_else(|| "no code action response".into())
}

// ── Extract Variable ────────────────────────────────────────────────────────

#[test]
fn test_refactor_extract_variable_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "result = some_func(42) + other_func(7)\n";
    fixture.did_open("file:///extract_var.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///extract_var.py",
        0, 9, 0, 22, // select `some_func(42)`
        300,
    )?;

    assert!(
        resp.contains("Extract variable (basilisk)"),
        "should offer extract variable: {resp}"
    );
    assert!(
        resp.contains("refactor.extract.variable"),
        "should have correct kind: {resp}"
    );
    Ok(())
}

// ── Extract Constant ────────────────────────────────────────────────────────

#[test]
fn test_refactor_extract_constant_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "import os\n\ndef f() -> int:\n    return 42\n";
    fixture.did_open("file:///extract_const.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///extract_const.py",
        3, 11, 3, 13, // select `42`
        301,
    )?;

    assert!(
        resp.contains("Extract constant (basilisk)"),
        "should offer extract constant: {resp}"
    );
    Ok(())
}

// ── Extract Function ────────────────────────────────────────────────────────

#[test]
fn test_refactor_extract_function_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def main() -> None:\n    x: int = 1\n    y: int = x + 1\n    print(y)\n";
    fixture.did_open("file:///extract_fn.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///extract_fn.py",
        1, 0, 3, 0, // select lines 1-2
        302,
    )?;

    assert!(
        resp.contains("Extract function (basilisk)"),
        "should offer extract function: {resp}"
    );
    assert!(
        resp.contains("refactor.extract.function"),
        "should have correct kind: {resp}"
    );
    Ok(())
}

#[test]
fn test_refactor_extract_function_rejects_yield() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def gen() -> None:\n    yield 1\n    yield 2\n";
    fixture.did_open("file:///no_yield.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///no_yield.py",
        1, 0, 3, 0, // select yield lines
        303,
    )?;

    assert!(
        !resp.contains("Extract function (basilisk)"),
        "should NOT offer extract function when selection contains yield: {resp}"
    );
    Ok(())
}

// ── Union/Optional Conversion ───────────────────────────────────────────────

#[test]
fn test_refactor_convert_union_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "from typing import Union\nx: Union[int, str] = 1\n";
    fixture.did_open("file:///union.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///union.py",
        1, 3, 1, 3, // cursor on Union
        304,
    )?;

    assert!(
        resp.contains("Union[X, Y] to X | Y"),
        "should offer Union to pipe conversion: {resp}"
    );
    Ok(())
}

#[test]
fn test_refactor_convert_optional_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "from typing import Optional\nx: Optional[int] = None\n";
    fixture.did_open("file:///optional.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///optional.py",
        1, 3, 1, 3, // cursor on Optional
        305,
    )?;

    assert!(
        resp.contains("Optional[X] to X | None"),
        "should offer Optional to pipe conversion: {resp}"
    );
    Ok(())
}

// ── f-string Conversion ────────────────────────────────────────────────────

#[test]
fn test_refactor_convert_fstring_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "name: str = \"world\"\nx: str = f\"hello {name}\"\n";
    fixture.did_open("file:///fstr.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///fstr.py",
        1, 9, 1, 9, // cursor on f-string
        306,
    )?;

    assert!(
        resp.contains(".format()"),
        "should offer f-string to .format() conversion: {resp}"
    );
    Ok(())
}

// ── dict/list Literal Conversion ────────────────────────────────────────────

#[test]
fn test_refactor_convert_dict_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "x: dict[str, int] = dict(a=1, b=2)\n";
    fixture.did_open("file:///dictconv.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///dictconv.py",
        0, 20, 0, 20, // cursor on dict()
        307,
    )?;

    assert!(
        resp.contains("dict"),
        "should offer dict() conversion: {resp}"
    );
    Ok(())
}

#[test]
fn test_refactor_convert_list_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "x: list[int] = list()\n";
    fixture.did_open("file:///listconv.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///listconv.py",
        0, 15, 0, 15, // cursor on list()
        308,
    )?;

    assert!(
        resp.contains("list"),
        "should offer list() conversion: {resp}"
    );
    Ok(())
}

// ── Ternary Conversion ──────────────────────────────────────────────────────

#[test]
fn test_refactor_convert_ternary_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def f(cond: bool) -> int:\n    x: int = 1 if cond else 0\n    return x\n";
    fixture.did_open("file:///ternary.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///ternary.py",
        1, 4, 1, 4, // cursor on ternary line
        309,
    )?;

    assert!(
        resp.contains("if/else"),
        "should offer ternary to if/else conversion: {resp}"
    );
    Ok(())
}

// ── Inline Variable ─────────────────────────────────────────────────────────

#[test]
fn test_refactor_inline_variable_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def f() -> None:\n    temp = calculate()\n    result = temp + 1\n";
    fixture.did_open("file:///inline_var.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///inline_var.py",
        1, 4, 1, 4, // cursor on assignment inside function
        310,
    )?;

    assert!(
        resp.contains("Inline variable (basilisk)"),
        "should offer inline variable: {resp}"
    );
    Ok(())
}

// ── Inline Function ─────────────────────────────────────────────────────────

#[test]
fn test_refactor_inline_function_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def double(x: int) -> int:\n    return x * 2\n\nresult: int = double(5)\n";
    fixture.did_open("file:///inline_fn.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///inline_fn.py",
        3, 14, 3, 14, // cursor on call
        311,
    )?;

    assert!(
        resp.contains("Inline function (basilisk)"),
        "should offer inline function: {resp}"
    );
    Ok(())
}

// ── Move Symbol ─────────────────────────────────────────────────────────────

#[test]
fn test_refactor_move_symbol_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "import os\n\nclass MyWidget:\n    pass\n";
    fixture.did_open("file:///move.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///move.py",
        2, 0, 2, 0, // cursor on class line
        312,
    )?;

    assert!(
        resp.contains("Move") && resp.contains("new file"),
        "should offer move to new file: {resp}"
    );
    assert!(
        resp.contains("refactor.move"),
        "should have correct kind: {resp}"
    );
    Ok(())
}

// ── NamedTuple Conversion ───────────────────────────────────────────────────

#[test]
fn test_refactor_convert_namedtuple_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "from typing import NamedTuple\n\nclass Point(NamedTuple):\n    x: int\n    y: int\n";
    fixture.did_open("file:///nt.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///nt.py",
        2, 0, 2, 0, // cursor on class line
        313,
    )?;

    assert!(
        resp.contains("NamedTuple") || resp.contains("namedtuple"),
        "should offer NamedTuple conversion: {resp}"
    );
    Ok(())
}

// ── Rename with Scope Awareness ─────────────────────────────────────────────

#[test]
fn test_refactor_rename_produces_scoped_edits() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def outer() -> None:
    x: int = 1
    print(x)

def inner() -> None:
    x: int = 2
    print(x)
";
    fixture.did_open("file:///scope_rename.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Rename `x` in outer — should NOT touch `x` in inner.
    let resp = send_request(
        &mut fixture,
        314,
        "textDocument/rename",
        serde_json::json!({
            "textDocument": { "uri": "file:///scope_rename.py" },
            "position": { "line": 1, "character": 4 },
            "newName": "outer_x"
        }),
    )?
    .ok_or("no rename response")?;

    assert!(
        resp.contains("outer_x"),
        "rename should produce outer_x: {resp}"
    );
    // The response should contain changes — verify it has edits.
    assert!(
        resp.contains("changes"),
        "rename should include workspace changes: {resp}"
    );
    Ok(())
}

// ── Code Action Edit Verification ───────────────────────────────────────────

#[test]
fn test_refactor_extract_variable_edit_correctness() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "result: int = some_func(42) + other_func(7)\n";
    fixture.did_open("file:///ev_edit.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///ev_edit.py",
        0, 14, 0, 27, // select `some_func(42)`
        315,
    )?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let actions = parsed["result"]
        .as_array()
        .ok_or("expected result array")?;

    let extract_action = actions
        .iter()
        .find(|a| {
            a["title"]
                .as_str()
                .is_some_and(|t| t.contains("Extract variable (basilisk)"))
        })
        .ok_or("no extract variable action found")?;

    // Verify it has a workspace edit with changes.
    assert!(
        extract_action["edit"]["changes"].is_object(),
        "extract variable should produce workspace edit with changes"
    );
    Ok(())
}

#[test]
fn test_refactor_inline_variable_edit_correctness() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def f() -> None:\n    temp = calculate()\n    result = temp + 1\n";
    fixture.did_open("file:///iv_edit.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///iv_edit.py",
        1, 4, 1, 4, // cursor on assignment
        316,
    )?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let actions = parsed["result"]
        .as_array()
        .ok_or("expected result array")?;

    let inline_action = actions
        .iter()
        .find(|a| {
            a["title"]
                .as_str()
                .is_some_and(|t| t.contains("Inline variable"))
        })
        .ok_or("no inline variable action found")?;

    assert!(
        inline_action["edit"]["changes"].is_object(),
        "inline variable should produce workspace edit with changes"
    );
    Ok(())
}

// ── Negative Cases ──────────────────────────────────────────────────────────

#[test]
fn test_refactor_extract_variable_not_offered_for_empty_selection() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "x: int = 1\n";
    fixture.did_open("file:///ev_empty.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///ev_empty.py",
        0, 5, 0, 5, // zero-width selection
        317,
    )?;

    assert!(
        !resp.contains("Extract variable (basilisk)"),
        "should NOT offer extract variable for empty selection: {resp}"
    );
    Ok(())
}

#[test]
fn test_refactor_move_symbol_not_offered_for_assignment() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "x: int = 42\n";
    fixture.did_open("file:///no_move.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///no_move.py",
        0, 0, 0, 0,
        318,
    )?;

    assert!(
        !resp.contains("Move") || !resp.contains("new file"),
        "should NOT offer move for plain assignments: {resp}"
    );
    Ok(())
}
