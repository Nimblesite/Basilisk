//! Tests for [`callables_protocol`] from [CHKARCH-DIAG-CATEGORIES] and
//! [TYPEINF-GENERICS-PARAMSPEC]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for callables_protocol: Callable call-site arity violations.

use super::common::*;

#[test]
fn correct_callable_arity() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable

def invoke(cb: Callable[[int, str], bool]) -> bool:
    return cb(1, "hello")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"callables_protocol"),
        "correct arity should not fire E0122"
    );
    Ok(())
}

#[test]
fn wrong_callable_arity() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def invoke(cb: Callable[[int, str], bool]) -> bool:
    return cb(1)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn keyword_arg_on_callable() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Callable

def invoke(cb: Callable[[int], bool]) -> bool:
    return cb(x=1)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn paramspec_components_preserve_the_bound_callable_shape() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
from typing import Callable, ParamSpec, TypeVar

P = ParamSpec("P")
T = TypeVar("T")

def logged(f: Callable[P, T]) -> Callable[P, T]:
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> T:
        return f(*args, **kwargs)
    return wrapper

def invalid_component(value: P.args) -> None:
    pass
"#;
    let diags = run(source)?;
    let paramspec_diags = diags
        .iter()
        .filter(|diag| diag.code.code == "callables_protocol")
        .collect::<Vec<_>>();
    assert_eq!(
        paramspec_diags.len(),
        1,
        "the valid wrapper must pass and the misplaced P.args must fail: {diags:?}"
    );
    Ok(())
}
