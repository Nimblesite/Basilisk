//! Tests for [`constructors_callable`] from [CHKARCH-DIAG-CTOR-CALLABLE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CTOR-CALLABLE
//!
//! `constructors_callable` validates calls to a variable bound to a class's
//! constructor-to-callable conversion (the typing-spec rule "Converting a
//! constructor to callable").
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    dead_code,
    missing_docs
)]

mod common;

use common::{messages_for, run};

const PRELUDE: &str = "\
from typing import Any, Callable, Generic, ParamSpec, Self, TypeVar

P = ParamSpec(\"P\")
R = TypeVar(\"R\")
T = TypeVar(\"T\")


def accepts_callable(cb: Callable[P, R]) -> Callable[P, R]:
    return cb
";

fn check(body: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let source = format!("{PRELUDE}\n{body}");
    let diagnostics = run(&source)?;
    Ok(messages_for(&diagnostics, "constructors_callable")
        .into_iter()
        .map(str::to_owned)
        .collect())
}

#[test]
fn invalid_calls_emit_e0153() -> Result<(), Box<dyn std::error::Error>> {
    let messages = check(
        "\
class Class1:
    def __init__(self, x: int) -> None:
        pass


class Class2:
    pass


class Class9:
    def __init__(self, x: list[T], y: list[T]) -> None:
        pass


r1 = accepts_callable(Class1)
r2 = accepts_callable(Class2)
r9 = accepts_callable(Class9)

r1()           # missing required argument `x`
r1(y=1)        # unexpected keyword argument `y`
r2(1)          # too many positional arguments
r9([1], [\"\"])  # inconsistent TypeVar binding
",
    )?;

    assert_eq!(messages.len(), 4, "{messages:#?}");
    assert!(
        messages
            .iter()
            .any(|m| m.contains("missing required argument `x`")),
        "{messages:#?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("Unexpected keyword argument `y`")),
        "{messages:#?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("too many positional argument")),
        "{messages:#?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("Inconsistent binding for type variable `T`")),
        "{messages:#?}"
    );
    Ok(())
}

#[test]
fn valid_calls_do_not_emit_e0153() -> Result<(), Box<dyn std::error::Error>> {
    let messages = check(
        "\
class Class1:
    def __init__(self, x: int) -> None:
        pass


class Class2:
    pass


class Class9:
    def __init__(self, x: list[T], y: list[T]) -> None:
        pass


r1 = accepts_callable(Class1)
r2 = accepts_callable(Class2)
r9 = accepts_callable(Class9)

r1(1)
r2()
r9([\"\"], [\"\"])
",
    )?;

    assert!(messages.is_empty(), "{messages:#?}");
    Ok(())
}

#[test]
fn metaclass_call_accepts_any_arguments() -> Result<(), Box<dyn std::error::Error>> {
    // A metaclass `__call__` taking `*args, **kwargs` accepts any call, so no
    // E0153 should fire regardless of the class's own `__new__`/`__init__`.
    let messages = check(
        "\
class Meta1(type):
    def __call__(cls, *args: Any, **kwargs: Any) -> Any:
        raise NotImplementedError


class Class5(metaclass=Meta1):
    def __new__(cls, *args: Any, **kwargs: Any) -> Self:
        return super().__new__(cls)


r5 = accepts_callable(Class5)
r5()
r5(1, x=1)
",
    )?;

    assert!(messages.is_empty(), "{messages:#?}");
    Ok(())
}

#[test]
fn new_returning_non_self_controls_signature() -> Result<(), Box<dyn std::error::Error>> {
    // `__new__` returning a type other than `Self`/the class takes over: its
    // signature is used and `__init__` is ignored. `Class6.__new__` takes no
    // args, so `r6(1)` is too many; `Class4.__new__` requires `x`.
    let messages = check(
        "\
class Proxy:
    pass


class Class6:
    def __new__(cls) -> Proxy:
        return Proxy()

    def __init__(self, x: int) -> None:
        pass


class Class4:
    def __new__(cls, x: int) -> int:
        raise NotImplementedError


r6 = accepts_callable(Class6)
r4 = accepts_callable(Class4)

r6(1)   # too many: __new__ takes no args, __init__ ignored
r4()    # missing required argument `x`
r6()    # ok
r4(1)   # ok
",
    )?;

    assert_eq!(messages.len(), 2, "{messages:#?}");
    assert!(
        messages
            .iter()
            .any(|m| m.contains("too many positional argument")),
        "{messages:#?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("missing required argument `x`")),
        "{messages:#?}"
    );
    Ok(())
}

#[test]
fn non_identity_wrapper_is_not_tracked() -> Result<(), Box<dyn std::error::Error>> {
    // A wrapper whose return type differs from its parameter is NOT an
    // identity-over-callable, so the bound variable is not validated.
    let messages = check(
        "\
def changes(cb: Callable[P, int]) -> Callable[P, str]:
    return cb  # type: ignore


class Class1:
    def __init__(self, x: int) -> None:
        pass


w = changes(Class1)
w()
",
    )?;

    assert!(messages.is_empty(), "{messages:#?}");
    Ok(())
}

#[test]
fn typevar_conflict_detects_float_and_bytes_elements() -> Result<(), Box<dyn std::error::Error>> {
    // Exercises the float/bytes literal element classification in the
    // TypeVar-binding consistency check.
    let messages = check(
        "\
class Pair:
    def __init__(self, x: list[T], y: list[T]) -> None:
        pass


rp = accepts_callable(Pair)

rp([1.0], [b\"\"])   # float vs bytes -> inconsistent
rp([1.0], [2.0])    # ok: both float
",
    )?;

    assert_eq!(messages.len(), 1, "{messages:#?}");
    assert!(
        messages
            .iter()
            .any(|m| m.contains("list[float]") && m.contains("list[bytes]")),
        "{messages:#?}"
    );
    Ok(())
}

#[test]
fn heterogeneous_list_argument_yields_no_conflict() -> Result<(), Box<dyn std::error::Error>> {
    // A heterogeneous list literal has no single element type, so it cannot
    // bind the TypeVar and must not produce a (false) conflict diagnostic.
    let messages = check(
        "\
class Pair:
    def __init__(self, x: list[T], y: list[T]) -> None:
        pass


rp = accepts_callable(Pair)

rp([1, \"x\"], [2])
",
    )?;

    assert!(messages.is_empty(), "{messages:#?}");
    Ok(())
}

#[test]
fn direct_class_alias_is_not_handled_by_e0153() -> Result<(), Box<dyn std::error::Error>> {
    // A plain alias `Alias = Class1` is a constructor call site handled by
    // other rules; E0153 only covers identity-callable-wrapped class objects.
    let messages = check(
        "\
class Class1:
    def __init__(self, x: int) -> None:
        pass


Alias = Class1
Alias()
",
    )?;

    assert!(messages.is_empty(), "{messages:#?}");
    Ok(())
}
