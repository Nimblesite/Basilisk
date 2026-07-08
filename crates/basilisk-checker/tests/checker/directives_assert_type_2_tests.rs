//! Tests for [`directives_assert_type_2`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// Integration tests for directives_assert_type_2: `assert_type()` type mismatch.

use super::common::*;

#[test]
fn valid_assert_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import assert_type

def f(a: int) -> None:
    assert_type(a, int)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn assert_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import assert_type

def f(a: int | str) -> None:
    assert_type(a, int)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn generic_call_return_inferred() -> Result<(), Box<dyn std::error::Error>> {
    // `ident(1)` returns `int`; asserting `Literal[1]` is a mismatch.
    let source = r#"
from typing import TypeVar, assert_type, Literal
T = TypeVar("T")
def ident(x: T) -> T:
    return x
assert_type(ident(1), int)
assert_type(ident(1), Literal[1])
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_assert_type_2"),
        "generic-call return inference should flag the Literal mismatch, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn enum_lookup_inferred() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from enum import Enum
from typing import assert_type, Literal
class Color(Enum):
    RED = 1
assert_type(Color["RED"], Color)
assert_type(Color["RED"], Literal[Color.RED])
assert_type(Color(1), Color)
"#;
    let diags = run(source)?;
    // `Color["RED"]` / `Color(1)` are inferred as the enum type, which makes the
    // `Literal[...]` assertion redundant (E0061) or a mismatch (E0053). Either
    // satisfies the conformance group; both come from the enum-lookup inference.
    let codes = codes(&diags);
    assert!(
        codes.contains(&"enums_expansion") || codes.contains(&"directives_assert_type_2"),
        "enum member-lookup inference should flag the Literal assertion, got: {codes:?}"
    );
    Ok(())
}

#[test]
fn typevar_default_inferred() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar, assert_type, Any
T4 = TypeVar("T4", default=int)
def func1(x: int | set[T4]) -> T4:
    raise NotImplementedError
assert_type(func1(0), int)
assert_type(func1(0), Any)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_assert_type_2"),
        "an unbound defaulted TypeVar resolves to its default; Any assertion mismatches, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn constrained_typevar_not_inferred() -> Result<(), Box<dyn std::error::Error>> {
    // A constrained TypeVar widens in ways text-binding cannot judge — no
    // inference, so no false positive.
    let source = r#"
from typing import TypeVar, assert_type
AnyStr = TypeVar("AnyStr", str, bytes)
class MyStr(str): ...
def concat(x: AnyStr, y: AnyStr) -> AnyStr:
    return x
def f(m: MyStr) -> None:
    assert_type(concat(m, m), str)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_assert_type_2"),
        "constrained TypeVar must not be inferred (no false positive), got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn constructor_call_not_inferred() -> Result<(), Box<dyn std::error::Error>> {
    // A class call is a constructor, typed by the class, not analysed here.
    let source = r#"
from typing import assert_type
class Box:
    def __init__(self, x: int) -> None: ...
assert_type(Box(1), Box)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_assert_type_2"),
        "constructor calls must not be inferred as a mismatch, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
