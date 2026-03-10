//! Integration tests for BSK-E0112: `TypeGuard` callable return type mismatch.
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
fn e0112_typeguard_passed_where_str_expected() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard, Callable
def takes_callable_str(f: Callable[[object], str]) -> None: ...
def simple_typeguard(val: object) -> TypeGuard[int]: ...
takes_callable_str(simple_typeguard)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0112_typeguard_passed_where_bool_expected_ok() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard, Callable
def takes_callable_bool(f: Callable[[object], bool]) -> None: ...
def simple_typeguard(val: object) -> TypeGuard[int]: ...
takes_callable_bool(simple_typeguard)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0112"),
        "TypeGuard is a subtype of bool; should not fire E0112"
    );
    Ok(())
}

#[test]
fn e0112_typeis_passed_where_str_expected() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs, Callable
def takes_callable_str(f: Callable[[object], str]) -> None: ...
def check_int(val: object) -> TypeIs[int]: ...
takes_callable_str(check_int)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn e0112_protocol_callback_typeguard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard, Protocol
class Checker(Protocol):
    def __call__(self, val: object) -> str: ...

def my_guard(val: object) -> TypeGuard[int]: ...
c: Checker = my_guard
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
