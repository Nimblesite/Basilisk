//! PEP 484/526: annotated assignments are checked against their declared type.
//! Specifications: https://peps.python.org/pep-0484/ and https://peps.python.org/pep-0526/

mod common;

use common::{assert_rule_count, run};

const RULE: &str = "assignment_compatibility";

#[test]
fn incompatible_annotated_assignment_is_reported_once() -> Result<(), Box<dyn std::error::Error>> {
    for source in [
        "ore: int = \"granite\"\n",
        "ore : int = (\n    \"granite\"\n)\n",
    ] {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            RULE,
            1,
            "PEP 484 requires the assigned value to be compatible with the declared type",
        );
    }
    Ok(())
}

#[test]
fn compatible_assignment_and_bare_declaration_are_not_reported(
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(r#"
declared: int
assigned: int = 42
"#)?;
    assert_rule_count(
        &diagnostics,
        RULE,
        0,
        "a declaration without a value has no assignment to reject, and 42 is compatible with int",
    );
    Ok(())
}

#[test]
fn parameter_types_flow_into_annotated_local_assignments() -> Result<(), Box<dyn std::error::Error>>
{
    let diagnostics = run(r#"
def assay(count: int, label: str) -> None:
    wrong_label: str = count
    wrong_count: int = label
"#)?;
    assert_rule_count(
        &diagnostics,
        RULE,
        2,
        "PEP 484 parameter annotations determine the types used by assignments in the body",
    );
    Ok(())
}

#[test]
fn type_alias_declaration_is_not_a_value_assignment_check() -> Result<(), Box<dyn std::error::Error>>
{
    for source in [
        r#"
from typing import TypeAlias as AliasMarker
Broken: AliasMarker = [int, str]
"#,
        r#"
import typing
Broken: typing.TypeAlias = [int, str]
"#,
    ] {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            RULE,
            0,
            "PEP 613 makes this a type-alias declaration, not an annotated value assignment",
        );
    }
    Ok(())
}
