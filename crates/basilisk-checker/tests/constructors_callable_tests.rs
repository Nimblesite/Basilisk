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
from typing import Any, Callable, Generic, Never, ParamSpec, Self, TypeVar, overload

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
fn instance_returning_passthrough_new_does_not_hide_init_signature(
) -> Result<(), Box<dyn std::error::Error>> {
    let messages = check(
        "\
class Class3:
    def __new__(cls, *args, **kwargs) -> Self:
        raise NotImplementedError

    def __init__(self, x: int) -> None:
        pass


r3 = accepts_callable(Class3)
r3()
r3(y=1)
r3(1, 2)
",
    )?;

    assert_eq!(messages.len(), 3, "{messages:#?}");
    assert!(messages
        .iter()
        .any(|message| message.contains("missing required")));
    assert!(messages
        .iter()
        .any(|message| message.contains("Unexpected keyword")));
    assert!(messages
        .iter()
        .any(|message| message.contains("too many positional")));
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

/// [STUBRES-PYI] #289: a metaclass signature terminates conversion only when
/// its return is special. An ordinary instance-producing `__call__` is ignored
/// and construction continues through `__new__`/`__init__`.
#[test]
fn special_metaclass_call_terminates_but_instance_call_does_not(
) -> Result<(), Box<dyn std::error::Error>> {
    let messages = check(
        "\
class SpecialMeta(type):
    def __call__(cls, token: int) -> Never:
        raise NotImplementedError


class Special(metaclass=SpecialMeta):
    def __init__(self, ignored: str) -> None:
        pass


class OrdinaryMeta(type):
    def __call__(cls, *args: Any, **kwargs: Any) -> Self:
        return super().__call__(*args, **kwargs)


class Ordinary(metaclass=OrdinaryMeta):
    def __init__(self, required: int) -> None:
        pass


special = accepts_callable(Special)
ordinary = accepts_callable(Ordinary)

special(token=1)
special(ignored=\"wrong path\")  # special metaclass signature controls
ordinary(1)
ordinary()  # instance-producing metaclass call must not hide __init__
",
    )?;

    assert_eq!(messages.len(), 2, "{messages:#?}");
    assert!(messages.iter().any(|message| message.contains("`ignored`")));
    assert!(messages
        .iter()
        .any(|message| message.contains("`required`")));
    Ok(())
}

/// [STUBRES-PYI] #289: inherited constructor methods are selected over the
/// Python C3 MRO, not a depth-first base walk. The applicable `__new__` and
/// `__init__` bound signatures form a callable union, so a call must be valid
/// for every member.
#[test]
fn inherited_constructor_signatures_follow_c3_and_form_union(
) -> Result<(), Box<dyn std::error::Error>> {
    let messages = check(
        "\
class Root:
    def __init__(self, from_root: int) -> None:
        pass


class Left(Root):
    def __new__(cls, from_new: int) -> Self:
        return super().__new__(cls)


class Right(Root):
    def __init__(self, from_right: str) -> None:
        pass


class Diamond(Left, Right):
    pass


converted = accepts_callable(Diamond)
converted(from_new=1)        # inherited Left.__new__, bound cls removed
converted(from_right=\"x\")  # C3 selects Right.__init__ before Root
converted(from_root=1)       # Root.__init__ must not win the C3 lookup
",
    )?;

    assert_eq!(messages.len(), 3, "{messages:#?}");
    assert!(messages
        .iter()
        .any(|message| message.contains("`from_new`")));
    assert!(messages
        .iter()
        .any(|message| message.contains("`from_right`")));
    assert!(messages
        .iter()
        .any(|message| message.contains("`from_root`")));
    Ok(())
}

/// [STUBRES-PYI] #289: overload declarations, rather than their permissive
/// implementation body, define the converted callable alternatives.
#[test]
fn overloaded_init_preserves_every_bound_alternative() -> Result<(), Box<dyn std::error::Error>> {
    let messages = check(
        "\
class Overloaded:
    @overload
    def __init__(self, value: int) -> None:
        ...

    @overload
    def __init__(self, value: str, extra: str) -> None:
        ...

    def __init__(self, *args: Any) -> None:
        pass


converted = accepts_callable(Overloaded)
converted(1)
converted(\"x\", \"y\")
converted()
converted(1, 2, 3)
",
    )?;

    assert_eq!(messages.len(), 2, "{messages:#?}");
    assert!(
        messages
            .iter()
            .any(|message| message.contains("missing required argument")),
        "{messages:#?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("too many positional argument")),
        "{messages:#?}"
    );
    Ok(())
}

/// [STUBRES-PYI] #289: a non-instance member of a `__new__` return union
/// terminates conversion, so `__init__` is not an applicable alternative.
#[test]
fn union_with_non_instance_new_return_terminates_before_init(
) -> Result<(), Box<dyn std::error::Error>> {
    let messages = check(
        "\
class MaybeInstance:
    def __new__(cls, choose: bool) -> Self | int:
        return super().__new__(cls)

    def __init__(self, init_only: str) -> None:
        pass


converted = accepts_callable(MaybeInstance)
converted(choose=True)
converted(init_only=\"must be ignored\")
",
    )?;

    assert_eq!(messages.len(), 1, "{messages:#?}");
    assert!(messages[0].contains("`init_only`"), "{messages:#?}");
    Ok(())
}

/// [STUBRES-PYI] #289: absent non-`object` constructor declarations bind to
/// the zero-argument object fallback.
#[test]
fn object_constructor_fallback_is_zero_argument() -> Result<(), Box<dyn std::error::Error>> {
    let messages = check(
        "\
class Plain:
    pass


converted = accepts_callable(Plain)
converted()
converted(1)
",
    )?;

    assert_eq!(messages.len(), 1, "{messages:#?}");
    assert!(messages[0].contains("too many positional argument"));
    Ok(())
}
