//! Tests for [`calls_argument_type`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for calls_argument_type: Argument type mismatch at call site.

use super::common::*;

#[test]
fn str_literal_for_int_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def add(x: int, y: int) -> int:
    return x + y

result: int = add("hello", "world")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "str literal for int param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn correct_arg_types_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def add(x: int, y: int) -> int:
    return x + y

result: int = add(1, 2)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"calls_argument_type"),
        "correct arg types should not fire E0012"
    );
    Ok(())
}

#[test]
fn bound_method_does_not_collide_with_same_named_function() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
def consume(value: int) -> None:
    pass

class Box:
    def consume(self, value: str) -> None:
        pass

box: Box = Box()
box.consume("valid method argument")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"calls_argument_type"),
        "a bound method must not be checked against a same-named module function"
    );
    Ok(())
}

#[test]
fn int_literal_for_str_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str) -> str:
    return name

result: str = greet(42)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "int literal for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn float_literal_for_int_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def count(n: int) -> int:
    return n

result: int = count(3.14)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "float literal for int param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn bytes_literal_for_str_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def process(data: str) -> str:
    return data

result: str = process(b"hello")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "bytes literal for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn str_for_bytes_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def process(data: bytes) -> bytes:
    return data

result: bytes = process("hello")
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "str literal for bytes param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn none_for_type_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def register(cls: type) -> None:
    pass

register(None)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "None for type param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn overloaded_function_correct_args() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"calls_argument_type"),
        "correct args for overloaded function should not fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn multiple_params_mixed_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def multi(a: int, b: str, c: float) -> None:
    pass

multi(1, 2, 3)
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"calls_argument_type"),
        "int for str param should fire E0012, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
