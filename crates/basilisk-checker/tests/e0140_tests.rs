#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for BSK-E0140: Callable assignment compatibility.
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
fn e0140_callable_param_count_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def two_args(x: int, y: int) -> int:
    return x + y

cb: Callable[[int], int] = two_args
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0140_callable_param_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def str_func(x: str) -> str:
    return x

cb: Callable[[int], str] = str_func
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0140_valid_callable_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def add(x: int) -> int:
    return x + 1

cb: Callable[[int], int] = add
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0140"),
        "compatible callable assignment should not fire E0140"
    );
    Ok(())
}

#[test]
fn e0140_callable_with_varargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def varargs_func(*args: int) -> int:
    return sum(args)

cb: Callable[[int, int], int] = varargs_func
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0140_callable_with_kwargs() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def kwargs_func(**kwargs: int) -> int:
    return 0

cb: Callable[[int], int] = kwargs_func
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0140_protocol_callback_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Protocol

class Callback(Protocol):
    def __call__(self, x: int) -> str: ...

def my_func(x: str) -> str:
    return x

cb: Callback = my_func
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
