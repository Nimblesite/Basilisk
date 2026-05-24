//! Tests for [BSK-E0012] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0012: Argument type mismatch at call site.

use super::common::*;

#[test]
fn e0012_str_literal_for_int_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def add(x: int, y: int) -> int:
    return x + y

result: int = add("hello", "world")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0012"),
        "str literal for int param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0012_correct_arg_types_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def add(x: int, y: int) -> int:
    return x + y

result: int = add(1, 2)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0012"),
        "correct arg types should not fire E0012"
    );
    Ok(())
}

#[test]
fn e0012_int_literal_for_str_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str) -> str:
    return name

result: str = greet(42)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0012"),
        "int literal for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0012_float_literal_for_int_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def count(n: int) -> int:
    return n

result: int = count(3.14)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0012"),
        "float literal for int param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0012_bytes_literal_for_str_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def process(data: str) -> str:
    return data

result: str = process(b"hello")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0012"),
        "bytes literal for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0012_str_for_bytes_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def process(data: bytes) -> bytes:
    return data

result: bytes = process("hello")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0012"),
        "str literal for bytes param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0012_none_for_type_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def register(cls: type) -> None:
    pass

register(None)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0012"),
        "None for type param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0012_overloaded_function_correct_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import overload

@overload
def parse(data: str) -> str: ...

@overload
def parse(data: int) -> int: ...

def parse(data):
    return data

result: str = parse("hello")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0012"),
        "correct args for overloaded function should not fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0012_multiple_params_mixed_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def multi(a: int, b: str, c: float) -> None:
    pass

multi(1, 2, 3)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0012"),
        "int for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
