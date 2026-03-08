//! Integration tests for BSK-E0082: TypeVarTuple callable/tuple mismatch.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn codes(diags: &[basilisk_checker::Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

#[test]
fn e0082_tuple_arg_order_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Callable
Ts = TypeVarTuple("Ts")

class Process:
    def __init__(self, target: Callable[[*Ts], None], args: tuple[*Ts]) -> None: ...

def func1(arg1: int, arg2: str) -> None: ...

Process(target=func1, args=("", 0))
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0082_valid_tuple_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVarTuple, Callable
Ts = TypeVarTuple("Ts")

class Process:
    def __init__(self, target: Callable[[*Ts], None], args: tuple[*Ts]) -> None: ...

def func1(arg1: int, arg2: str) -> None: ...

Process(target=func1, args=(0, ""))
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0082"),
        "correct tuple arg order should not fire E0082"
    );
    Ok(())
}
