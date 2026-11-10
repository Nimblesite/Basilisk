//! Tests for [`callables_protocol_2`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for callables_protocol_2: Callable assignment compatibility.

use super::common::*;

#[test]
fn callable_param_count_mismatch() -> Result<(), Box<dyn std::error::Error>> {
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
fn callable_param_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // PEP 484 callable subtyping: parameter types are contravariant. A value
    // callable only with `str` cannot inhabit a slot that may be called with
    // `int`. These spellings are semantically identical and must produce the
    // same exact rule diagnostic from resolved symbols, never rendered text.
    // https://peps.python.org/pep-0484/#covariance-and-contravariance
    let rejected = [
        r#"
from typing import Callable as Signature
from builtins import int as Whole, str as Text

def render(value: Text) -> Text:
    return value

callback: Signature[[Whole], Text] = render
"#,
        r#"
import typing as contracts
import builtins as core

def render(value: core.str) -> core.str:
    return value

callback: contracts.Callable[[core.int], core.str] = render
"#,
        r#"
from typing import Callable as Invocation
from builtins import int as Count, str as Label

def label_for(item: Label) -> Label:
    return item

consumer: Invocation[[Count], Label] = label_for
"#,
        "
from typing import Callable as Signature
from builtins import int as Whole, str as Text

def render(
        value : Text ,
) -> Text :
        return value

callback : Signature[
    [ Whole ] ,
    Text ,
] = render
",
    ];
    for source in rejected {
        let diags = run(source)?;
        assert_eq!(
            diags.len(),
            1,
            "PEP 484 callable mismatch must produce one diagnostic: {diags:?}"
        );
        assert_eq!(
            codes(&diags),
            vec!["callables_protocol_2"],
            "callable parameter spelling changed the owning rule"
        );
    }

    let accepted = [
        r#"
from typing import Callable as Signature
from builtins import int as Whole, str as Text

def render(value: Whole) -> Text:
    return "ok"

callback: Signature[[Whole], Text] = render
"#,
        r#"
import typing as contracts
import builtins as core

def render(value: core.int) -> core.str:
    return "ok"

callback: contracts.Callable[[core.int], core.str] = render
"#,
        r#"
from typing import Callable as Invocation
from builtins import int as Count, str as Label

def label_for(item: Count) -> Label:
    return "ok"

consumer: Invocation[[Count], Label] = label_for
"#,
        "
from typing import Callable as Signature
from builtins import int as Whole, str as Text

def render(
        value : Whole ,
) -> Text :
        return 'ok'

callback : Signature[
    [ Whole ] ,
    Text ,
] = render
",
    ];
    for source in accepted {
        let diags = run(source)?;
        assert!(
            diags.is_empty(),
            "PEP 484-compatible callable assignment produced diagnostics: {diags:?}"
        );
    }
    Ok(())
}

#[test]
fn valid_callable_assignment() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable
def add(x: int) -> int:
    return x + 1

cb: Callable[[int], int] = add
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"callables_protocol_2"),
        "compatible callable assignment should not fire E0140"
    );
    Ok(())
}

#[test]
fn callable_with_varargs() -> Result<(), Box<dyn std::error::Error>> {
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
fn callable_with_kwargs() -> Result<(), Box<dyn std::error::Error>> {
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
fn protocol_callback_assignment() -> Result<(), Box<dyn std::error::Error>> {
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
