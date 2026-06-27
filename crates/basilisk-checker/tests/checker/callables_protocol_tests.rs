//! Tests for [callables_protocol] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
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
