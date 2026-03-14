//! Tests for LSP: ws_test_semantic_tokens.

#![allow(dead_code)]

mod ws_test_common;
use ws_test_common::*;

// ── Semantic Token Tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_ws_semantic_tokens_full() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
class Animal:
    name: str
    def speak(self) -> str:
        return self.name

def greet(animal: Animal) -> str:
    return animal.name

x: int = 42
";
    fixture.did_open("file:///ws_semtok.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            370,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    // Should return a data array with encoded tokens
    assert!(
        resp.contains("\"data\""),
        "semantic tokens should contain 'data' array: {resp}"
    );
    assert!(
        resp.contains("result"),
        "semantic tokens should have result: {resp}"
    );

    // Parse the response and verify we get tokens
    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    // Each token is 5 integers, so data length should be a multiple of 5
    assert_eq!(
        data.len() % 5,
        0,
        "token data length should be multiple of 5"
    );
    // We should have tokens for Animal, name, speak, self, greet, animal, x at minimum
    assert!(data.len() >= 5, "should have at least 1 token: {resp}");

    // Hardened: verify first token has valid tokenType (0-9 range for standard LSP token types)
    let first_token_type = data[3].as_u64().unwrap_or(u64::MAX);
    assert!(
        first_token_type <= 20,
        "first token's tokenType should be in valid range (0-20), got {first_token_type}: {resp}"
    );

    // Hardened: verify no negative deltas in the data (all values should be non-negative)
    for (idx, value) in data.iter().enumerate() {
        let num = value.as_i64().unwrap_or(-1);
        assert!(
            num >= 0,
            "semantic token data[{idx}] must be non-negative, got {num}: {resp}"
        );
    }

    // Hardened: verify we have a reasonable number of tokens for this code
    let token_count = data.len() / 5;
    assert!(
        token_count >= 3,
        "should have at least 3 tokens for code with class, function, and variable, got {token_count}: {resp}"
    );

    // Hardened: verify JSON-RPC structure
    assert_eq!(
        parsed["jsonrpc"], "2.0",
        "must be valid JSON-RPC 2.0: {resp}"
    );
    assert_eq!(
        parsed["id"], 370,
        "response id must match request id: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_semantic_tokens_decorator() -> TestResult<()> {
    let mut fixture = WsTestFixture::new().await?;
    let _ = fixture.initialize().await?;

    let code = "\
from typing import Generic, TypeVar

T = TypeVar('T')

class Box(Generic[T]):
    value: T

    @staticmethod
    def empty() -> None:
        pass

def greet(name: str) -> str:
    return name
";
    fixture.did_open("file:///ws_semtok_dec.py", code).await?;
    let _ = fixture.wait_for_diagnostics().await;

    let resp = fixture
        .request(
            900,
            "textDocument/semanticTokens/full",
            serde_json::json!({
                "textDocument": { "uri": "file:///ws_semtok_dec.py" }
            }),
        )
        .await?
        .ok_or("no semantic tokens response")?;

    assert!(
        resp.contains("\"data\""),
        "semantic tokens should contain 'data' array: {resp}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&resp)?;
    let data = parsed["result"]["data"]
        .as_array()
        .ok_or("data should be an array")?;

    // Each token is 5 integers; we have decorators, type annotations, type params, etc.
    assert_eq!(
        data.len() % 5,
        0,
        "token data length should be multiple of 5"
    );

    // Should have many tokens: imports, T, Box, Generic[T], value, staticmethod,
    // empty, greet, name, str, str return annotations, etc.
    // Minimum: at least 8 tokens (40 integers)
    assert!(
        data.len() >= 40,
        "should have at least 8 tokens for decorated code: {resp}"
    );

    Ok(())
}

#[tokio::test]
async fn test_ws_semantic_tokens_class_token() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_class.py",
        "\
class Animal:
    name: str
",
        1203,
    )
    .await?;

    assert!(!tokens.is_empty(), "should have at least 1 token: {resp}");

    // Token type 2 = class. The first token should be "Animal" at line 0.
    // data layout: [deltaLine, deltaStart, length, tokenType, tokenModifiers]
    // Find a token with tokenType=2 (class).
    let has_class_token = tokens.iter().any(|t| t[3] == 2);
    assert!(
        has_class_token,
        "should have a token with type 2 (class) for 'Animal': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_semantic_tokens_parameter_token() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_param.py",
        "\
def greet(name: str) -> str:
    return name
",
        1204,
    )
    .await?;

    // Token type 3 = parameter. "name" should be classified as a parameter.
    let has_param_token = tokens.iter().any(|t| t[3] == 3);
    assert!(
        has_param_token,
        "should have a token with type 3 (parameter) for 'name': {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_semantic_tokens_variable_token() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_var.py",
        "\
x: int = 42
y: str = \"hello\"
",
        1205,
    )
    .await?;

    // Token type 4 = variable. Module-level x and y should be classified as variables.
    let variable_count = tokens.iter().filter(|t| t[3] == 4).count();
    assert!(
        variable_count >= 2,
        "should have at least 2 tokens with type 4 (variable) for x and y: {resp}"
    );
    Ok(())
}

#[tokio::test]
async fn test_ws_semantic_tokens_method_vs_function() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_meth_fn.py",
        "\
class Dog:
    def bark(self) -> str:
        return \"woof\"

def greet(name: str) -> str:
    return name
",
        1206,
    )
    .await?;

    // Token type 0 = function, 1 = method.
    let has_method = tokens.iter().any(|t| t[3] == 1);
    let has_function = tokens.iter().any(|t| t[3] == 0);

    assert!(
        has_method,
        "should have a token with type 1 (method) for 'bark': {resp}"
    );
    assert!(
        has_function,
        "should have a token with type 0 (function) for 'greet': {resp}"
    );
    Ok(())
}

/// Verify tokenType=7 (decorator) appears for @decorator usage.
#[tokio::test]
async fn test_ws_semantic_tokens_decorator_token() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_decorator.py",
        "\
def my_decorator(func):
    return func

@my_decorator
def hello() -> None:
    pass
",
        1207,
    )
    .await?;

    // Token type 7 = decorator. The @my_decorator usage should emit a decorator token.
    let has_decorator_token = tokens.iter().any(|t| t[3] == 7);
    assert!(
        has_decorator_token,
        "should have a token with type 7 (decorator) for '@my_decorator': {resp}"
    );
    Ok(())
}

/// Verify tokenType=8 (type) appears for type annotations.
#[tokio::test]
async fn test_ws_semantic_tokens_type_annotation() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_type_ann.py",
        "\
def process(data: str) -> int:
    return 42
",
        1208,
    )
    .await?;

    // Token type 8 = type. Both "str" (param annotation) and "int" (return annotation)
    // should produce type tokens.
    let type_token_count = tokens.iter().filter(|t| t[3] == 8).count();
    assert!(
        type_token_count >= 2,
        "should have at least 2 tokens with type 8 (type) for 'str' and 'int' annotations: {resp}"
    );
    Ok(())
}

/// Verify tokenType=9 (typeParameter) appears for generic type parameters.
#[tokio::test]
async fn test_ws_semantic_tokens_type_parameter() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_typeparam.py",
        "\
from typing import Generic, TypeVar

T = TypeVar('T')

class Box(Generic[T]):
    value: T
",
        1209,
    )
    .await?;

    // Token type 9 = typeParameter. Generic params in class Box should emit this.
    let has_type_param = tokens.iter().any(|t| t[3] == 9);
    assert!(
        has_type_param,
        "should have a token with type 9 (typeParameter) for generic param T: {resp}"
    );
    Ok(())
}

/// Verify `MOD_STATIC` (bit 3, value 8) is set for @staticmethod function tokens.
#[tokio::test]
async fn test_ws_semantic_tokens_static_modifier() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_static.py",
        "\
class MathUtils:
    @staticmethod
    def add(a: int, b: int) -> int:
        return a + b
",
        1210,
    )
    .await?;

    // Token type 1 = method. MOD_STATIC = bit 3 = value 8.
    // The "add" method token should have the static modifier set (bit 3).
    // Find method tokens (type 1) and check at least one has static modifier (bit 3 = 8).
    let has_static_method = tokens.iter().any(|t| t[3] == 1 && (t[4] & 8) != 0);
    assert!(
        has_static_method,
        "should have a method token with MOD_STATIC (bit 3) for @staticmethod 'add': {resp}"
    );
    Ok(())
}

/// Verify `MOD_DECLARATION` (bit 2, value 4) is set on function/class definition tokens.
#[tokio::test]
async fn test_ws_semantic_tokens_declaration_modifier() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_decl.py",
        "\
class Animal:
    pass

def greet(name: str) -> str:
    return name
",
        1211,
    )
    .await?;

    // MOD_DECLARATION = bit 2 = value 4.
    // Both class (type 2) and function (type 0) definition tokens should have this.

    // Class token (type 2) should have declaration modifier.
    let class_has_decl = tokens.iter().any(|t| t[3] == 2 && (t[4] & 4) != 0);
    assert!(
        class_has_decl,
        "class 'Animal' token should have MOD_DECLARATION (bit 2): {resp}"
    );

    // Function token (type 0) should have declaration modifier.
    let func_has_decl = tokens.iter().any(|t| t[3] == 0 && (t[4] & 4) != 0);
    assert!(
        func_has_decl,
        "function 'greet' token should have MOD_DECLARATION (bit 2): {resp}"
    );
    Ok(())
}

/// Verify tokenType=5 (property) appears for class attributes.
#[tokio::test]
async fn test_ws_semantic_tokens_property_token() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_property.py",
        "\
class Person:
    name: str
    age: int
",
        1212,
    )
    .await?;

    // Token type 5 = property. Class attributes "name" and "age" should be properties.
    let property_count = tokens.iter().filter(|t| t[3] == 5).count();
    assert!(
        property_count >= 2,
        "should have at least 2 tokens with type 5 (property) for 'name' and 'age': {resp}"
    );
    Ok(())
}

/// Verify tokenType=6 (namespace) appears for import statements.
#[tokio::test]
async fn test_ws_semantic_tokens_namespace_token() -> TestResult<()> {
    let (resp, tokens) = semantic_tokens_for(
        "file:///ws_semtok_namespace.py",
        "\
import os
import sys
",
        1213,
    )
    .await?;

    // Token type 6 = namespace. Import statements should produce namespace tokens.
    let namespace_count = tokens.iter().filter(|t| t[3] == 6).count();
    assert!(
        namespace_count >= 2,
        "should have at least 2 tokens with type 6 (namespace) for 'os' and 'sys' imports: {resp}"
    );
    Ok(())
}
