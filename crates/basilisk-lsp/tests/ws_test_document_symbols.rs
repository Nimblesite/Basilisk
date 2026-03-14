//! Tests for LSP: ws_test_document_symbols.

#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

#[tokio::test]
async fn test_ws_document_symbols() -> TestResult<()> {
    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_symbols.py",
        code,
        320,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_symbols.py" }
        }),
    )
    .await?;

    assert!(
        resp.contains("Animal"),
        "symbols should include class 'Animal': {resp}"
    );
    assert!(
        resp.contains("greet"),
        "symbols should include function 'greet': {resp}"
    );
    assert!(
        resp.contains("\"x\""),
        "symbols should include variable 'x': {resp}"
    );

    // Hardened: parse and verify symbol count and structure
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let symbols = parsed["result"]
        .as_array()
        .ok_or("document symbols result should be an array")?;

    // Exact count: Animal (class), greet (function), x (variable) = 3 top-level symbols
    assert_eq!(
        symbols.len(),
        3,
        "should have exactly 3 top-level symbols (Animal, greet, x), got {}: {resp}",
        symbols.len()
    );

    // Hardened: verify each symbol has a valid range with start <= end
    for symbol in symbols {
        let range = &symbol["range"];
        assert!(!range.is_null(), "every symbol must have a range: {resp}");
        let start_line = range["start"]["line"].as_u64().unwrap_or(u64::MAX);
        let end_line = range["end"]["line"].as_u64().unwrap_or(0);
        assert!(
            start_line <= end_line,
            "symbol range start line must be <= end line: {resp}"
        );
    }

    // Hardened: verify class symbol has children (methods/attributes)
    let animal_symbol = symbols
        .iter()
        .find(|s| s["name"].as_str() == Some("Animal"))
        .ok_or("Animal symbol not found in results")?;
    let children = animal_symbol["children"]
        .as_array()
        .ok_or("Animal class symbol should have children array")?;
    assert!(
        !children.is_empty(),
        "Animal class should have children (name attr + speak method): {resp}"
    );

    // Hardened: verify symbol kinds are present
    for symbol in symbols {
        assert!(
            symbol.get("kind").is_some() && !symbol["kind"].is_null(),
            "every symbol must have a kind: {resp}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_ws_document_symbols_nested_methods() -> TestResult<()> {
    let code = "\
class Calculator:
    value: int
    def add(self, x: int) -> int:
        return self.value + x
    def multiply(self, x: int) -> int:
        return self.value * x
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_nested.py",
        code,
        321,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_nested.py" }
        }),
    )
    .await?;

    assert!(resp.contains("Calculator"), "should contain class: {resp}");
    assert!(resp.contains("add"), "should contain method 'add': {resp}");
    assert!(
        resp.contains("multiply"),
        "should contain method 'multiply': {resp}"
    );
    assert!(
        resp.contains("value"),
        "should contain attribute 'value': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_document_symbols_empty_file_returns_empty() -> TestResult<()> {
    let (_fixture, resp) = open_and_request(
        "file:///ws_edge_symbols.py",
        "",
        402,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_edge_symbols.py" }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = &parsed["result"];
    assert!(
        result.is_null() || result.as_array().is_some_and(Vec::is_empty),
        "document symbols on empty file should be null or empty array: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_document_symbols_module_variables() -> TestResult<()> {
    // File with ONLY top-level variables (no classes or functions).
    let code = "\
MAX_SIZE: int = 100
name: str = \"basilisk\"
enabled: bool = True
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_symbols_vars.py",
        code,
        1100,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_symbols_vars.py" }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = parsed["result"].as_array().ok_or("expected result array")?;

    // All three module variables should appear.
    let names: Vec<&str> = result.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(
        names.contains(&"MAX_SIZE"),
        "symbols should include 'MAX_SIZE': {resp}"
    );
    assert!(
        names.contains(&"name"),
        "symbols should include 'name': {resp}"
    );
    assert!(
        names.contains(&"enabled"),
        "symbols should include 'enabled': {resp}"
    );

    // Verify they are VARIABLE kind (SymbolKind::VARIABLE = 13).
    for sym in result {
        if sym["name"].as_str() == Some("MAX_SIZE")
            || sym["name"].as_str() == Some("name")
            || sym["name"].as_str() == Some("enabled")
        {
            assert_eq!(
                sym["kind"].as_u64(),
                Some(13),
                "module variable should have kind VARIABLE (13): {sym}"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_ws_document_symbols_multiple_classes() -> TestResult<()> {
    let code = "\
class Cat:
    name: str
    def meow(self) -> str:
        return \"meow\"

class Dog:
    name: str
    def bark(self) -> str:
        return \"woof\"

class Bird:
    name: str
    def chirp(self) -> str:
        return \"tweet\"
";
    let (_fixture, resp) = open_and_request(
        "file:///ws_symbols_multi_class.py",
        code,
        1101,
        "textDocument/documentSymbol",
        serde_json::json!({
            "textDocument": { "uri": "file:///ws_symbols_multi_class.py" }
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let result = parsed["result"].as_array().ok_or("expected result array")?;

    // All three classes should appear at top level.
    let top_names: Vec<&str> = result.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(
        top_names.contains(&"Cat"),
        "should contain class 'Cat': {resp}"
    );
    assert!(
        top_names.contains(&"Dog"),
        "should contain class 'Dog': {resp}"
    );
    assert!(
        top_names.contains(&"Bird"),
        "should contain class 'Bird': {resp}"
    );

    // Each class should have children (nested methods).
    for class_name in &["Cat", "Dog", "Bird"] {
        let class_sym = result
            .iter()
            .find(|s| s["name"].as_str() == Some(class_name))
            .ok_or(format!("class '{class_name}' not found"))?;

        // Classes should be kind CLASS (5).
        assert_eq!(
            class_sym["kind"].as_u64(),
            Some(5),
            "class should have kind CLASS (5): {class_sym}"
        );

        let children = class_sym["children"]
            .as_array()
            .ok_or(format!("class '{class_name}' should have children"))?;
        assert!(
            !children.is_empty(),
            "class '{class_name}' should have nested children (methods/attributes): {resp}"
        );
    }

    Ok(())
}
