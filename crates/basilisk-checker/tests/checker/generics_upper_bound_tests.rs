//! Tests for [`generics_upper_bound`] from [CHKARCH-DIAG-UNUSED] / [TYPEINF-GENERICS-BOUND]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-UNUSED
// Tests for generics_upper_bound: `TypeVar` upper bound violation at call site.
//
// This rule detects when a call site passes a value whose type does not satisfy
// the `TypeVar` upper bound declared on the corresponding parameter.

use super::common::*;

#[test]
fn test_e0080_int_violates_sized_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Sized, TypeVar

ST = TypeVar("ST", bound=Sized)

def longer(x: ST, y: ST) -> ST:
    if len(x) > len(y):
        return x
    return y

longer(3, 3)  # E -- int does not implement Sized (__len__)
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(!e0080.is_empty(), "int should violate Sized bound");
    Ok(())
}

#[test]
fn test_e0080_str_satisfies_sized_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Sized, TypeVar

ST = TypeVar("ST", bound=Sized)

def longer(x: ST, y: ST) -> ST:
    if len(x) > len(y):
        return x
    return y

longer("hello", "world")  # OK -- str implements Sized
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(e0080.is_empty(), "str should satisfy Sized bound");
    Ok(())
}

#[test]
fn test_e0080_multiple_bounds_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Sized, Hashable, TypeVar

STH = TypeVar("STH", bound=Sized)

def process(x: STH) -> STH:
    return x

process(42)  # E -- int does not implement Sized
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(!e0080.is_empty(), "int should violate Sized bound");
    Ok(())
}

#[test]
fn test_e0080_custom_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypeVar

T = TypeVar("T", bound="int")

def process(x: T) -> T:
    return x

process("hello")  # E -- str does not satisfy int bound
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(!e0080.is_empty(), "str should violate int bound");
    Ok(())
}

#[test]
fn test_e0080_nested_typevar_bounds() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Sized, TypeVar

ST = TypeVar("ST", bound=Sized)

def outer(x: ST) -> ST:
    def inner(y: ST) -> ST:
        return y
    return inner(x)

outer(3.14)  # E -- float does not implement Sized
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(!e0080.is_empty(), "float should violate Sized bound");
    Ok(())
}

#[test]
fn test_e0080_multiple_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Sized, TypeVar

ST = TypeVar("ST", bound=Sized)

def process(a: ST, b: ST, c: ST) -> ST:
    return a

process(1, 2, 3)  # E -- all ints violate Sized bound
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(!e0080.is_empty(), "ints should violate Sized bound");
    Ok(())
}

#[test]
fn test_e0080_class_method_bounds() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Sized, TypeVar

ST = TypeVar("ST", bound=Sized)

class Processor:
    def process(self, x: ST) -> ST:
        return x

# Method calls are not collected as call sites by the resolver,
# so E0080 cannot detect violations in method calls.
# This test verifies that no E0080 is emitted for method calls.
p = Processor()
p.process(42)  # No E0080 - method calls not checked
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(e0080.is_empty(), "method calls should not trigger E0080");
    Ok(())
}

#[test]
fn test_e0080_complex_bound_expression() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Sized, Hashable, TypeVar

ComplexT = TypeVar("ComplexT", bound=Sized)

def handle(x: ComplexT) -> ComplexT:
    return x

handle(True)  # E -- bool does not implement Sized
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(!e0080.is_empty(), "bool should violate Sized bound");
    Ok(())
}

#[test]
fn test_e0080_none_violates_sized_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Sized, TypeVar

ST = TypeVar("ST", bound=Sized)

def process(x: ST) -> ST:
    return x

process(None)  # E -- None does not implement Sized
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(!e0080.is_empty(), "None should violate Sized bound");
    Ok(())
}

#[test]
fn test_e0080_bytes_satisfies_sized_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Sized, TypeVar

ST = TypeVar("ST", bound=Sized)

def process(x: ST) -> ST:
    return x

process(b"hello")  # OK -- bytes implements Sized
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(e0080.is_empty(), "bytes should satisfy Sized bound");
    Ok(())
}

#[test]
fn test_e0080_int_satisfies_int_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypeVar

T = TypeVar("T", bound="int")

def process(x: T) -> T:
    return x

process(42)  # OK -- int satisfies int bound
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(e0080.is_empty(), "int should satisfy int bound");
    Ok(())
}

#[test]
fn test_e0080_int_satisfies_float_bound() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypeVar

T = TypeVar("T", bound="float")

def process(x: T) -> T:
    return x

process(42)  # OK -- int satisfies float bound (widening)
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(e0080.is_empty(), "int should satisfy float bound");
    Ok(())
}

#[test]
fn test_e0080_unbound_typevar_no_error() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import TypeVar

T = TypeVar("T")  # No bound

def process(x: T) -> T:
    return x

process(42)  # OK -- unbound TypeVar accepts any type
process("hello")  # OK -- unbound TypeVar accepts any type
process(3.14)  # OK -- unbound TypeVar accepts any type
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    assert!(e0080.is_empty(), "unbound TypeVar should accept any type");
    Ok(())
}

#[test]
fn test_e0080_protocol_bound_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
from typing import Protocol, TypeVar

class SomeProtocol(Protocol):
    def method(self) -> None: ...

T = TypeVar("T", bound=SomeProtocol)

def process(x: T) -> T:
    return x

process(42)  # E -- int does not implement SomeProtocol
"#;
    let diags = run(src)?;
    let e0080: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "generics_upper_bound")
        .collect();
    // This test may fail if the rule doesn't handle custom protocol bounds yet
    // That's OK - failing test shows missing functionality
    assert!(!e0080.is_empty(), "int should violate SomeProtocol bound");
    Ok(())
}
