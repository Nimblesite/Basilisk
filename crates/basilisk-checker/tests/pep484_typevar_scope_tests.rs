//! PEP 484: binding and scope rules for type variables.
//! Specifications: https://peps.python.org/pep-0484/#scoping-rules-for-type-variables and
//! https://typing.python.org/en/latest/spec/generics.html#scoping-rules-for-type-variables

mod common;

use common::{assert_rule_count, run};

const RULE: &str = "generics_scoping";

#[test]
fn unbound_typevar_in_function_body_is_reported(
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(
        r#"
from typing import TypeVar as Variable

Bound = Variable("Bound")
Unbound = Variable("Unbound")

def assay(value: Bound) -> Bound:
    leaked: list[Unbound] = []
    return value
"#,
    )?;
    assert_rule_count(
        &diagnostics,
        RULE,
        1,
        "PEP 484 forbids an unbound type variable in a generic function body",
    );
    Ok(())
}

#[test]
fn unbound_typevar_in_generic_class_body_is_reported(
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(
        r#"
import typing

Bound = typing.TypeVar("Bound")
Unbound = typing.TypeVar("Unbound")

class Ledger(typing.Generic[Bound]):
    invalid: list[Unbound]
"#,
    )?;
    assert_rule_count(
        &diagnostics,
        RULE,
        1,
        "PEP 484 forbids unrelated type variables in a generic class body",
    );
    Ok(())
}

#[test]
fn class_typevar_is_bound_in_the_class_body() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(
        r#"
from typing import Generic as Template, TypeVar as Variable

Entry = Variable("Entry")

class Ledger(Template[Entry]):
    rows: list[Entry]
"#,
    )?;
    assert_rule_count(
        &diagnostics,
        RULE,
        0,
        "PEP 484 binds a Generic base's type variables throughout that class body",
    );
    Ok(())
}

#[test]
fn nested_generic_class_cannot_reuse_outer_typevar(
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(
        r#"
from typing import Generic as Template, TypeVar as Variable

Entry = Variable("Entry")

class Outer(Template[Entry]):
    class Inner(Template[Entry]):
        pass
"#,
    )?;
    assert_rule_count(
        &diagnostics,
        RULE,
        1,
        "PEP 484 says an outer generic class's type-variable scope does not cover an inner generic class",
    );
    Ok(())
}
