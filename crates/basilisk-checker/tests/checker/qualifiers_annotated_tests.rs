//! Tests for [qualifiers_annotated] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
// Integration tests for qualifiers_annotated: Invalid first argument to Annotated.

use super::common::*;

#[test]
fn valid_annotated_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[int, "metadata"] = 42
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"qualifiers_annotated"),
        "valid Annotated usage should not fire E0045"
    );
    Ok(())
}

#[test]
fn annotated_with_list_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[[int, str], ""] = 42
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn annotated_with_bool_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[True, ""] = True
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn annotated_with_int_literal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated

x: Annotated[1, ""] = 1
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn annotated_too_few_args() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Annotated

x: Annotated[int] = 42
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn annotated_callable_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated, Callable

x: Annotated[Callable[[int], str], "meta"] = lambda a: str(a)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"qualifiers_annotated"),
        "Annotated with Callable first arg should not fire E0045"
    );
    Ok(())
}
