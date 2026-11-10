//! PEP 613: explicit `TypeAlias` declarations and their permitted scopes/RHS.
//! Specifications: https://peps.python.org/pep-0613/#specification and
//! https://typing.python.org/en/latest/spec/aliases.html

mod common;

use common::{assert_rule_count, run};

const RULE: &str = "aliases_implicit";

#[test]
fn invalid_alias_rhs_is_reported_through_every_import_form(
) -> Result<(), Box<dyn std::error::Error>> {
    for source in [
        r#"
from typing import TypeAlias as AliasMarker
Broken: AliasMarker = [int, str]
"#,
        r#"
import typing
Broken: typing.TypeAlias = [int, str]
"#,
        r#"
from typing import TypeAlias as AliasMarker

Broken : AliasMarker = [
    int,
    str,
]
"#,
    ] {
        let diagnostics = run(source)?;
        assert_rule_count(
            &diagnostics,
            RULE,
            1,
            "PEP 613 requires an explicit alias RHS to be a valid type expression",
        );
    }
    Ok(())
}

#[test]
fn call_expression_is_not_a_valid_alias_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(
        r#"
from typing import TypeAlias as AliasMarker
Broken: AliasMarker = eval("int")
"#,
    )?;
    assert_rule_count(
        &diagnostics,
        RULE,
        1,
        "PEP 613 requires the checker to report an invalid type at the alias declaration",
    );
    Ok(())
}

#[test]
fn valid_alias_rhs_and_class_scope_are_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(
        r#"
from typing import TypeAlias as AliasMarker

Number: AliasMarker = int | float

class Ledger:
    Row: AliasMarker = tuple[str, Number]
"#,
    )?;
    assert_rule_count(
        &diagnostics,
        RULE,
        0,
        "PEP 613 permits valid explicit aliases at module and class scope",
    );
    Ok(())
}

#[test]
fn explicit_alias_inside_function_is_reported() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(
        r#"
from typing import TypeAlias as AliasMarker

def build() -> None:
    LocalAlias: AliasMarker = list[int]
"#,
    )?;
    assert_rule_count(
        &diagnostics,
        RULE,
        1,
        "PEP 613 forbids explicit type-alias declarations inside functions",
    );
    Ok(())
}
