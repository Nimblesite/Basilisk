//! Tests for [qualifiers_annotated_2] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
// Integration tests for qualifiers_annotated_2: Annotated too few arguments.

use super::common::*;

#[test]
fn annotated_single_arg_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import Annotated
x: Annotated[int]
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"qualifiers_annotated_2"),
        "Annotated with single arg should fire E0058, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn annotated_two_args_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Annotated
x: Annotated[int, "metadata"]
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"qualifiers_annotated_2"),
        "Annotated with two args should not fire E0058"
    );
    Ok(())
}
