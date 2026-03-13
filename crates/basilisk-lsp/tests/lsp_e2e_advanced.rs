#![allow(dead_code)]

mod lsp_e2e_common;
use lsp_e2e_common::*;

// ── Hover — enhanced (exact format + call-site + parameter + attribute) ──────

#[test]
fn test_lsp_hover_function_exact_signature() -> TestResult<()> {
    // Proves hover shows the COMPLETE formatted signature, not just fragments.
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///hover_exact.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Hover on the 'g' in "greet" — line 0, character 4.
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

#[test]
fn test_lsp_hover_from_call_site() -> TestResult<()> {
    // THE KEY TEST: hovering on a CALL SITE resolves to the function definition.
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    fixture.did_open("file:///hover_call.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // "result: str = greet(\"world\")" is line 3.
    // "result: str = " is 14 chars, so 'g' of "greet" is at character 14.
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
    // Hover on a parameter at its definition site shows "(parameter) name: type".
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///hover_param.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // "def greet(" is 10 chars, so 'n' of "name" is at character 10.
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
    // Hover on a class attribute shows "(property) ClassName.attr: type".
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "class Animal:\n    name: str\n    age: int\n";
    fixture.did_open("file:///hover_attr.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Line 1: "    name: str" — "name" starts at character 4.
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

// ── Go to Definition — exact position + call-site + type annotation ─────────

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
    // THE KEY TEST: goto-def triggered FROM a call site jumps to the function
    // definition — the primary end-to-end user workflow for F12.
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n\nresult: str = greet(\"world\")\n";
    fixture.did_open("file:///goto_call.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Line 3: "result: str = greet(\"world\")" — 'g' of call "greet" at character 14.
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
    // Should jump to line 0, char 4 — where "def greet" begins.
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
    // goto-def on a class name used in a type annotation resolves to the class definition.
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "class Dog:\n    name: str\n\ndef pet(dog: Dog) -> None:\n    pass\n";
    fixture.did_open("file:///goto_type.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Line 3: "def pet(dog: Dog) -> None:"
    // "def pet(dog: " is 13 chars, so 'D' of "Dog" is at character 13.
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
    // "class Dog:" — 'D' of "Dog" is at char 6 on line 0.
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

// ── Capability advertisement ────────────────────────────────────────────────

#[test]
fn test_lsp_initialize_advertises_new_capabilities() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let response = fixture.initialize()?;

    assert!(
        response.contains("\"definitionProvider\""),
        "should advertise definition: {response}"
    );
    assert!(
        response.contains("\"documentSymbolProvider\""),
        "should advertise document symbols: {response}"
    );
    assert!(
        response.contains("\"signatureHelpProvider\""),
        "should advertise signature help: {response}"
    );
    assert!(
        response.contains("\"referencesProvider\""),
        "should advertise references: {response}"
    );
    assert!(
        response.contains("\"renameProvider\""),
        "should advertise rename: {response}"
    );
    assert!(
        response.contains("\"inlayHintProvider\""),
        "should advertise inlay hints: {response}"
    );
    assert!(
        response.contains("\"semanticTokensProvider\""),
        "should advertise semantic tokens: {response}"
    );
    assert!(
        response.contains("\"declarationProvider\""),
        "should advertise declaration: {response}"
    );
    assert!(
        response.contains("\"typeDefinitionProvider\""),
        "should advertise type definition: {response}"
    );
    Ok(())
}

// ── Go to Declaration ───────────────────────────────────────────────────────

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

    // Cursor on "compute" at the call site: line 3, col 14
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

// ── Go to Type Definition ───────────────────────────────────────────────────

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

    // Cursor on "instance" at line 3, col 0
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

    // Cursor on "cfg" parameter at line 3, col 12
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

// ── Docstrings ──────────────────────────────────────────────────────────────

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

    // Hover at the call site "calculate" on line 4, col 18.
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

// ── Folding Ranges ──────────────────────────────────────────────────────────

#[test]
fn test_lsp_folding_range() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(name: str) -> str:
    return f\"Hello, {name}!\"
";
    fixture.did_open("file:///fold.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        300,
        "textDocument/foldingRange",
        serde_json::json!({
            "textDocument": { "uri": "file:///fold.py" }
        }),
    )?
    .ok_or("no foldingRange response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    // The class and two functions should produce folding ranges.
    assert!(
        resp.contains("startLine"),
        "should contain folding ranges with startLine: {resp}"
    );
    Ok(())
}

// ── Selection Ranges (Smart Select) ─────────────────────────────────────────

#[test]
fn test_lsp_selection_range() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"
";
    fixture.did_open("file:///sel.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        301,
        "textDocument/selectionRange",
        serde_json::json!({
            "textDocument": { "uri": "file:///sel.py" },
            "positions": [{ "line": 0, "character": 4 }]
        }),
    )?
    .ok_or("no selectionRange response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ── Code Lens ───────────────────────────────────────────────────────────────

#[test]
fn test_lsp_code_lens() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return f\"Hello, {name}!\"

def caller() -> None:
    greet(\"world\")
    greet(\"test\")
";
    fixture.did_open("file:///lens.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    let resp = send_request(
        &mut fixture,
        302,
        "textDocument/codeLens",
        serde_json::json!({
            "textDocument": { "uri": "file:///lens.py" }
        }),
    )?
    .ok_or("no codeLens response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ── Document Highlight ──────────────────────────────────────────────────────

#[test]
fn test_lsp_document_highlight() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "\
def greet(name: str) -> str:
    return name
";
    fixture.did_open("file:///hl.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Highlight 'name' at the parameter position (line 0, char 10)
    let resp = send_request(
        &mut fixture,
        303,
        "textDocument/documentHighlight",
        serde_json::json!({
            "textDocument": { "uri": "file:///hl.py" },
            "position": { "line": 0, "character": 10 }
        }),
    )?
    .ok_or("no documentHighlight response")?;

    assert!(resp.contains("\"result\""), "should have a result: {resp}");
    Ok(())
}

// ── didSave re-checks diagnostics ───────────────────────────────────────────

#[test]
fn test_lsp_did_save_rechecks() -> TestResult<()> {
    let mut fixture = LspTestFixture::new()?;
    let _ = fixture.initialize()?;

    let code = "def greet(name: str) -> str:\n    return f\"Hello, {name}!\"";
    fixture.did_open("file:///save.py", code)?;
    let _ = fixture.wait_for_diagnostics();

    // Send didSave — should re-publish diagnostics.
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didSave",
        "params": {
            "textDocument": { "uri": "file:///save.py" }
        }
    }))?;

    let diag = fixture
        .wait_for_diagnostics()
        .ok_or("no diagnostics after save")?;

    assert!(
        diag.contains("\"diagnostics\":[]"),
        "clean code should have empty diagnostics after save: {diag}"
    );
    Ok(())
}
