//! Tests for [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
// Tests for LSP: `lsp_e2e_hover`.

// LSP E2E tests — Hover (type signatures, docstrings, enhanced hover).

use super::lsp_e2e_common::{send_request, LspTestFixture, TestResult};

// ── Basic hover ──────────────────────────────────────────────────────────────

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

// ── Enhanced hover ───────────────────────────────────────────────────────────

#[test]
fn test_lsp_hover_function_exact_signature() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///hover_exact.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        200,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_exact.py" },
            "position": { "line": 0, "character": 4 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(
        resp.contains("(function)"),
        "hover should show '(function)' prefix: {resp}"
    );
    assert!(
        resp.contains("def greet"),
        "hover should show 'def greet': {resp}"
    );
    assert!(
        resp.contains("name: str"),
        "hover should show typed parameter 'name: str': {resp}"
    );
    assert!(
        resp.contains("-> str"),
        "hover should show return type '-> str': {resp}"
    );
    Ok(())
}

/// Regression for #253: hovering an unannotated function must surface
/// inferred types — the return type inferred from the body's `return`
/// statements, and `Unknown` for parameters whose type cannot be inferred —
/// instead of a bare, type-less signature.
#[test]
fn test_lsp_hover_unannotated_function_shows_inferred_types() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    // Repro from #253 (mutation_testing/mutants_report.py:78): every dict
    // value and the `.get` default are `str`, so the return type is `str`.
    let code = "def extracted_function(missed, caught, unviable, timeout, summary):\n    return {\n        \"MissedMutant\": \"missed\",\n        \"CaughtMutant\": \"caught\",\n        \"Unviable\": \"unviable\",\n        \"Timeout\": \"timeout\",\n        \"Success\": \"success\",\n    }.get(summary, \"unknown\")\n";
    fixture.did_open("file:///hover_unannotated.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on "extracted_function" (line 0, character 8).
    let resp = send_request(
        &mut fixture,
        210,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_unannotated.py" },
            "position": { "line": 0, "character": 8 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(
        resp.contains("(function)"),
        "hover should show '(function)' prefix: {resp}"
    );
    assert!(
        resp.contains("def extracted_function"),
        "hover should show 'def extracted_function': {resp}"
    );
    assert!(
        resp.contains("missed: Unknown"),
        "unannotated parameter should render an inferred/Unknown type, not blank: {resp}"
    );
    assert!(
        resp.contains("-> str"),
        "hover should show the inferred return type '-> str': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_from_call_site() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    fixture.did_open("file:///hover_call.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        201,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_call.py" },
            "position": { "line": 3, "character": 14 }
        }),
    )?
    .ok_or("no hover response at call site")?;

    assert!(
        resp.contains("(function)"),
        "call-site hover should resolve to function: {resp}"
    );
    assert!(
        resp.contains("greet"),
        "call-site hover should show function name: {resp}"
    );
    assert!(
        resp.contains("name: str"),
        "call-site hover should show parameter type: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_parameter_shows_type() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///hover_param.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        202,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_param.py" },
            "position": { "line": 0, "character": 10 }
        }),
    )?
    .ok_or("no hover response for parameter")?;

    assert!(
        resp.contains("(parameter)"),
        "hover on parameter should show '(parameter)': {resp}"
    );
    assert!(
        resp.contains("name"),
        "hover should show parameter name: {resp}"
    );
    assert!(
        resp.contains("str"),
        "hover should show parameter type 'str': {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_class_attribute() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "class Animal:\n    name: str\n    age: int\n";
    fixture.did_open("file:///hover_attr.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        203,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_attr.py" },
            "position": { "line": 1, "character": 4 }
        }),
    )?
    .ok_or("no hover response for class attribute")?;

    assert!(
        resp.contains("(property)"),
        "hover on class attribute should show '(property)': {resp}"
    );
    assert!(
        resp.contains("Animal.name"),
        "hover should show 'Animal.name': {resp}"
    );
    assert!(
        resp.contains("str"),
        "hover should show attribute type 'str': {resp}"
    );
    Ok(())
}

// ── Docstring hover ──────────────────────────────────────────────────────────

#[test]
fn test_lsp_hover_shows_docstring() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def calculate(x: int) -> int:
    \"\"\"Compute the square of x.\"\"\"
    return x * x
";
    fixture.did_open("file:///docstr.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        210,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///docstr.py" },
            "position": { "line": 0, "character": 5 }
        }),
    )?
    .ok_or("no hover response")?;

    assert!(
        resp.contains("Compute the square of x"),
        "hover should include docstring: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_hover_shows_docstring_at_call_site() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def calculate(x: int) -> int:
    \"\"\"Compute the square of x.\"\"\"
    return x * x

result: int = calculate(5)
";
    fixture.did_open("file:///docstr_call.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        211,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///docstr_call.py" },
            "position": { "line": 4, "character": 18 }
        }),
    )?
    .ok_or("no hover response at call site")?;

    assert!(
        resp.contains("Compute the square of x"),
        "hover at call site should include docstring: {resp}"
    );
    Ok(())
}

#[test]
fn test_lsp_completion_includes_docstring() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def helper(x: int) -> int:
    \"\"\"Return x plus one.\"\"\"
    return x + 1

hel
";
    fixture.did_open("file:///compdoc.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        211,
        "textDocument/completion",
        serde_json::json!({
            "textDocument": { "uri": "file:///compdoc.py" },
            "position": { "line": 4, "character": 3 }
        }),
    )?
    .ok_or("no completion response")?;

    assert!(
        resp.contains("helper"),
        "completions should include 'helper': {resp}"
    );
    // Docstrings are now lazy-loaded via completionItem/resolve, so the initial
    // completion list includes `data` for resolve but not inline documentation.
    assert!(
        resp.contains("\"data\""),
        "completion should include resolve data: {resp}"
    );
    Ok(())
}
