//! Tests for [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
// Tests for LSP: `ws_test_hover`.

use super::ws_test_common::*;

#[tokio::test]
async fn test_ws_hover_function_exact_signature() -> TestResult<()> {
    let uri = "file:///hover_fn_sig.py";
    let code = "def greet(name: str) -> str:\n    return f'Hello, {name}!'\n";
    let resp = hover_at(uri, code, 0, 4, 300).await?;

    assert!(
        resp.contains("(function)"),
        "should contain (function): {resp}"
    );
    assert!(
        resp.contains("def greet"),
        "should contain def greet: {resp}"
    );
    assert!(
        resp.contains("name: str"),
        "should contain name: str: {resp}"
    );
    assert!(resp.contains("-> str"), "should contain -> str: {resp}");

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert_eq!(parsed["jsonrpc"], "2.0", "must be jsonrpc 2.0: {resp}");
    assert_eq!(parsed["id"], 300, "id must be 300: {resp}");
    assert!(
        !parsed["result"]["contents"].is_null(),
        "contents must not be null: {resp}"
    );
    let contents_str = parsed["result"]["contents"].to_string();
    assert!(
        contents_str.contains("greet"),
        "contents should contain greet: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_from_call_site() -> TestResult<()> {
    let uri = "file:///hover_call.py";
    let code = "def greet(name: str) -> str:\n    return f'Hello, {name}!'\n\nresult: str = greet('world')\n";
    let resp = hover_at(uri, code, 3, 14, 301).await?;

    assert!(
        resp.contains("(function)"),
        "should contain (function): {resp}"
    );
    assert!(resp.contains("greet"), "should contain greet: {resp}");
    assert!(
        resp.contains("name: str"),
        "should contain name: str: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_parameter_shows_type() -> TestResult<()> {
    let uri = "file:///hover_param.py";
    let code = "def greet(name: str) -> str:\n    return f'Hello, {name}!'\n";
    let resp = hover_at(uri, code, 0, 10, 302).await?;

    assert!(
        resp.contains("(parameter)"),
        "should contain (parameter): {resp}"
    );
    assert!(resp.contains("name"), "should contain name: {resp}");
    assert!(resp.contains("str"), "should contain str: {resp}");

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_class_attribute() -> TestResult<()> {
    let uri = "file:///hover_attr.py";
    let code =
        "class Animal:\n    name: str\n    def speak(self) -> str:\n        return self.name\n";
    let resp = hover_at(uri, code, 1, 4, 303).await?;

    assert!(
        resp.contains("(property)"),
        "should contain (property): {resp}"
    );
    assert!(
        resp.contains("Animal.name"),
        "should contain Animal.name: {resp}"
    );
    assert!(resp.contains("str"), "should contain str: {resp}");

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_unknown_position_returns_null() -> TestResult<()> {
    let uri = "file:///hover_null.py";
    let code = "x: int = 1\n";
    let resp = hover_at(uri, code, 100, 0, 304).await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    assert!(
        parsed["result"].is_null(),
        "hover beyond content should return null result: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_method_shows_class_prefix() -> TestResult<()> {
    let uri = "file:///hover_method.py";
    let code =
        "class Animal:\n    name: str\n    def speak(self) -> str:\n        return self.name\n";
    let resp = hover_at(uri, code, 2, 8, 305).await?;

    assert!(resp.contains("(method)"), "should contain (method): {resp}");
    assert!(
        resp.contains("Animal.speak"),
        "should contain Animal.speak: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_class_name_shows_class_info() -> TestResult<()> {
    let uri = "file:///hover_class.py";
    let code = "class Animal:\n    name: str\n";
    let resp = hover_at(uri, code, 0, 6, 306).await?;

    assert!(resp.contains("(class)"), "should contain (class): {resp}");
    assert!(resp.contains("Animal"), "should contain Animal: {resp}");

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_variable_shows_type() -> TestResult<()> {
    let uri = "file:///hover_var.py";
    let code = "x: int = 42\n";
    let resp = hover_at(uri, code, 0, 0, 307).await?;

    assert!(
        resp.contains("(variable)"),
        "should contain (variable): {resp}"
    );
    assert!(resp.contains("int"), "should contain int: {resp}");

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_class_with_bases() -> TestResult<()> {
    let uri = "file:///hover_bases.py";
    let code = "class Animal:\n    name: str\n\nclass Dog(Animal):\n    breed: str\n";
    let resp = hover_at(uri, code, 3, 6, 308).await?;

    assert!(resp.contains("(class)"), "should contain (class): {resp}");
    assert!(resp.contains("Dog"), "should contain Dog: {resp}");
    assert!(resp.contains("Animal"), "should contain Animal: {resp}");

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_import_shows_module() -> TestResult<()> {
    let uri = "file:///hover_import.py";
    let code = "import os\n";
    let resp = hover_at(uri, code, 0, 7, 309).await?;

    assert!(resp.contains("os"), "should contain os: {resp}");
    assert!(
        resp.contains("import") || resp.contains("module"),
        "should contain import or module: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_shows_docstring() -> TestResult<()> {
    let uri = "file:///hover_docstring.py";
    let code =
        "def square(x: int) -> int:\n    \"\"\"Compute the square of x.\"\"\"\n    return x * x\n";
    let resp = hover_at(uri, code, 0, 4, 310).await?;

    assert!(
        resp.contains("Compute the square of x"),
        "should contain docstring: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_class_docstring() -> TestResult<()> {
    let uri = "file:///hover_class_doc.py";
    let code = "class Animal:\n    \"\"\"Represents an animal with a name.\"\"\"\n    name: str\n";
    let resp = hover_at(uri, code, 0, 6, 311).await?;

    assert!(
        resp.contains("Represents an animal with a name"),
        "should contain class docstring: {resp}"
    );

    Ok(())
}

// Regression for #199: hovering a module-level constant (`PI: Final = 3.14`)
// showed no hover popup at all. Mirrors the issue's repro
// (upstream conformance `_qualifiers_final_annotation_2.py`) under the
// conditions the editor is in when it reproduces: a real on-disk workspace
// whose startup scan has completed, so the salsa (search-paths-known)
// analysis path serves the hover — not the pre-scan fallback.
#[tokio::test]
async fn test_ws_hover_module_constant_with_final_annotation() -> TestResult<()> {
    let dir = unique_temp_dir("bsk_ws_hover_final");
    std::fs::create_dir_all(&dir)?;
    let code = "\"\"\"\nUsed as part of the test for the typing.Final special form.\n\"\"\"\n\nfrom typing import Final\n\nPI: Final = 3.14\n";
    std::fs::write(dir.join("qualifiers_final.py"), code)?;
    // A second file with a guaranteed default-config diagnostic marks the
    // startup scan's completion (a clean file may publish nothing).
    std::fs::write(
        dir.join("scan_marker.py"),
        "def bad() -> int:\n    return \"s\"\n",
    )?;

    let root_uri = format!("file://{}", dir.display());
    let mut fixture = WsTestFixture::new().await?;
    let _ = initialize_with_root(&mut fixture, &root_uri, "wholeModule").await?;

    // Wait for the startup scan to publish the marker file's diagnostic —
    // after this the workspace search paths are known and per-file analysis
    // runs through the salsa engine.
    let mut scan_done = false;
    for _ in 0..20 {
        let Some(msg) = fixture.recv().await else {
            break;
        };
        if msg.contains("\"method\":\"textDocument/publishDiagnostics\"")
            && msg.contains("scan_marker.py")
        {
            scan_done = true;
            break;
        }
    }
    assert!(scan_done, "startup scan should publish for scan_marker.py");

    // Open the constant's file like the editor does, then hover `PI` at its
    // definition site (line 6, col 0 — "line 7" in the issue's 1-based repro).
    let uri = format!("{root_uri}/qualifiers_final.py");
    fixture.did_open(&uri, code).await?;
    let _ = fixture.wait_for_diagnostics().await?;
    let resp = fixture
        .request(
            320,
            "textDocument/hover",
            serde_json::json!({
                "textDocument": { "uri": uri },
                "position": { "line": 6, "character": 0 }
            }),
        )
        .await?
        .ok_or("no response to textDocument/hover")?;

    assert!(
        resp.contains("(variable)"),
        "module constant hover should contain (variable): {resp}"
    );
    assert!(
        resp.contains("PI"),
        "module constant hover should contain PI: {resp}"
    );
    assert!(
        resp.contains("Final"),
        "module constant hover should show its Final annotation: {resp}"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// Regression for #200: hovering a function-local binding returned nothing
// because the symbol lookup never searched `local_vars`/`local_unannotated_vars`.
// Mirrors the VSIX "hover: local variable inside a function (squared)" check.
#[tokio::test]
async fn test_ws_hover_local_variable_at_definition() -> TestResult<()> {
    let uri = "file:///hover_local_def.py";
    let code = "def calculate(operand: int) -> int:\n    squared = operand * operand\n    return squared\n";
    // Cursor on `squared` at its definition site (line 1, col 4).
    let resp = hover_at(uri, code, 1, 4, 312).await?;

    assert!(
        resp.contains("(variable)"),
        "local var hover should contain (variable): {resp}"
    );
    assert!(
        resp.contains("squared"),
        "local var hover should contain squared: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_local_variable_at_usage() -> TestResult<()> {
    let uri = "file:///hover_local_use.py";
    let code = "def calculate(operand: int) -> int:\n    squared = operand * operand\n    return squared\n";
    // Cursor on the `squared` reference in `return squared` (line 2, col 11).
    let resp = hover_at(uri, code, 2, 11, 313).await?;

    assert!(
        resp.contains("(variable)"),
        "local var usage hover should contain (variable): {resp}"
    );
    assert!(
        resp.contains("squared"),
        "local var usage hover should contain squared: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_hover_local_annotated_variable() -> TestResult<()> {
    let uri = "file:///hover_local_ann.py";
    let code =
        "def calculate(operand: int) -> int:\n    total: int = operand + 1\n    return total\n";
    // Cursor on the annotated local `total` at its definition (line 1, col 4).
    let resp = hover_at(uri, code, 1, 4, 314).await?;

    assert!(
        resp.contains("(variable)"),
        "annotated local hover should contain (variable): {resp}"
    );
    assert!(
        resp.contains("total"),
        "annotated local hover should contain total: {resp}"
    );
    assert!(
        resp.contains("int"),
        "annotated local hover should show its type int: {resp}"
    );

    Ok(())
}

/// [NARROWPLAN-CHECKLIST] Stage 2 — member-access hover resolves the
/// receiver's type through the SAME bidirectional engine as checker
/// diagnostics (`receiver_type_name` → `rhs_or_expr_type_display` in
/// `crates/basilisk-lsp/src/hover/access.rs`): a variable bound to a method
/// call the `RhsKind` table cannot type still resolves for `str` methods.
/// Dot completions share this exact receiver path.
#[tokio::test]
async fn test_ws_hover_member_access_via_engine_inferred_receiver() -> TestResult<()> {
    let uri = "file:///hover_engine_receiver.py";
    let code = "name = \"a\".upper()\nshort = name.strip()\n";
    // Hover over `strip` on line 1 (`name.strip()`, col 13 hits "strip").
    let resp = hover_at(uri, code, 1, 13, 320).await?;

    assert!(
        resp.contains("strip"),
        "hover must resolve str.strip through the engine-inferred receiver: {resp}"
    );
    assert!(
        resp.contains("str"),
        "should present the str method: {resp}"
    );

    Ok(())
}
