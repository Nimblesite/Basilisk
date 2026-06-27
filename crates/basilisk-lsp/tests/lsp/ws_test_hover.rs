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
    let code = "def calculate(operand: int) -> int:\n    total: int = operand + 1\n    return total\n";
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
