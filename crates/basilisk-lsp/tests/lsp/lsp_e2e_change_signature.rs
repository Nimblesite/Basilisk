//! Tests for [LSPARCH-FEATURES-SIGHELP]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-SIGHELP
// E2E tests for change signature and edit correctness verification.
//
// Tests change signature (remove/add/reorder parameters) and verifies
// that abstract method implementation produces correct workspace edits.

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

// ── Change Signature: Remove Parameter ───────────────────────────────────────

#[test]
fn test_refactor_change_signature_remove_param_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str, greeting: str) -> str:\n    return f\"{greeting}, {name}\"\n\nresult: str = greet(\"world\", \"Hello\")\n";
    fixture.did_open("file:///change_sig_remove.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///change_sig_remove.py",
        0,
        21,
        0,
        29, // cursor on `greeting` parameter
        319,
    )?;

    assert!(
        resp.contains("Remove parameter"),
        "should offer remove parameter: {resp}"
    );
    Ok(())
}

// ── Change Signature: Add Parameter ─────────────────────────────────────────

#[test]
fn test_refactor_change_signature_add_param_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}\"\n";
    fixture.did_open("file:///change_sig_add.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///change_sig_add.py",
        0,
        4,
        0,
        4, // cursor on function name
        320,
    )?;

    assert!(
        resp.contains("Add parameter"),
        "should offer add parameter: {resp}"
    );
    Ok(())
}

// ── Change Signature: Reorder Parameters ────────────────────────────────────

#[test]
fn test_refactor_change_signature_reorder_offered() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def process(zebra: int, apple: int, mango: int) -> int:\n    return zebra + apple + mango\n";
    fixture.did_open("file:///change_sig_reorder.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///change_sig_reorder.py",
        0,
        4,
        0,
        4, // cursor on function name
        321,
    )?;

    assert!(
        resp.contains("Sort parameters"),
        "should offer sort parameters: {resp}"
    );
    Ok(())
}

// ── Change Signature: Remove Parameter Edit Correctness ─────────────────────

#[test]
fn test_refactor_change_signature_remove_param_edit_correctness() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str, greeting: str) -> str:\n    return f\"{greeting}, {name}\"\n\nresult: str = greet(\"world\", \"Hello\")\n";
    fixture.did_open("file:///change_sig_rm_edit.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///change_sig_rm_edit.py",
        0,
        21,
        0,
        29, // cursor on `greeting` parameter
        322,
    )?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let actions = parsed["result"].as_array().ok_or("expected result array")?;

    let action = actions
        .iter()
        .find(|a| {
            a["title"]
                .as_str()
                .is_some_and(|t| t.contains("Remove parameter"))
        })
        .ok_or("no remove parameter action found")?;

    assert!(
        action["edit"]["changes"].is_object(),
        "remove parameter should produce workspace edit with changes"
    );
    Ok(())
}

// ── Implement Abstract Methods Edit Correctness ─────────────────────────────

// Exercises [REFACTOR-ABSTRACT-ALGO] — verifies the generated stub edit.
#[test]
fn test_refactor_implement_abstract_methods_edit_correctness() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "from abc import ABC, abstractmethod\n\nclass Base(ABC):\n    @abstractmethod\n    def do_thing(self) -> None:\n        ...\n\nclass Child(Base):\n    pass\n";
    fixture.did_open("file:///abstract_edit.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = request_code_actions(
        &mut fixture,
        "file:///abstract_edit.py",
        7,
        6,
        7,
        6, // cursor inside Child class
        323,
    )?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let actions = parsed["result"].as_array().ok_or("expected result array")?;

    let action = actions
        .iter()
        .find(|a| {
            a["title"]
                .as_str()
                .is_some_and(|t| t.contains("abstract") || t.contains("Implement"))
        })
        .ok_or("no implement abstract methods action found")?;

    assert!(
        action["edit"]["changes"].is_object(),
        "implement abstract methods should produce workspace edit with changes"
    );
    Ok(())
}
