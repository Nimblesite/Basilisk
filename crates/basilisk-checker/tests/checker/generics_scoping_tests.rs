//! PEP 484 type-variable scope tests for [`generics_scoping`].
//!
//! The obligations come from PEP 484's scoping rules, not fixture spelling:
//! <https://peps.python.org/pep-0484/#scoping-rules-for-type-variables>.
//! Every obligation is repeated with aliased and module-qualified typing
//! symbols plus formatting mutations so raw-name recognition cannot satisfy it.

use super::common::*;

const RULE: &str = "generics_scoping";

fn assert_scoping(
    source: &str,
    expected: usize,
    offending_name: Option<&str>,
    obligation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(source)?;
    assert_rule_count(&diagnostics, RULE, expected, obligation);

    let messages = messages_for(&diagnostics, RULE);
    assert_eq!(
        messages.len(),
        expected,
        "{obligation}: every counted violation must have a rule-specific message: {diagnostics:#?}",
    );
    match offending_name {
        Some(name) => assert!(
            messages.iter().all(|message| message.contains(name)),
            "{obligation}: every diagnostic must identify the unbound TypeVar `{name}`: {messages:#?}",
        ),
        None => assert!(
            messages.is_empty(),
            "{obligation}: valid TypeVar scoping must produce no `{RULE}` message: {messages:#?}",
        ),
    }
    Ok(())
}

#[test]
fn function_signature_binds_typevar_throughout_function() -> Result<(), Box<dyn std::error::Error>>
{
    let sources = [
        r#"
from typing import TypeVar as VariableForge

Ore = VariableForge("Ore")

def assay(sample: Ore) -> list[Ore]:
    result: list[Ore] = [sample]
    return result
"#,
        r#"
import typing as type_forms

Ore = type_forms.TypeVar( "Ore" )

def assay(
    sample: Ore,
) -> list[
    Ore
]:
    result: list[Ore] = [ sample ]
    return result
"#,
    ];

    for source in sources {
        assert_scoping(
            source,
            0,
            None,
            "PEP 484 binds a function-signature TypeVar throughout that generic function",
        )?;
    }
    Ok(())
}

#[test]
fn unrelated_typevar_in_function_body_is_unbound() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import TypeVar as VariableForge

Ore = VariableForge("Ore")
Slag = VariableForge("Slag")

def assay(sample: Ore) -> Ore:
    rejected: list[Slag] = []
    return sample
"#,
        r#"
import typing as type_forms

Ore = type_forms.TypeVar( "Ore" )
Slag = type_forms.TypeVar(
    "Slag",
)

def assay(
    sample: Ore,
) -> Ore:
    rejected: list[
        Slag
    ] = []
    return sample
"#,
    ];

    for source in sources {
        assert_scoping(
            source,
            1,
            Some("Slag"),
            "PEP 484 does not bind an unrelated TypeVar merely because it appears in a generic function body",
        )?;
    }
    Ok(())
}

#[test]
fn generic_base_binds_typevar_throughout_class() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Generic as Family, TypeVar as VariableForge

Ore = VariableForge("Ore")

class Crucible(Family[Ore]):
    contents: list[Ore]
"#,
        r#"
import typing as type_forms

Ore = type_forms.TypeVar( "Ore" )

class Crucible(
    type_forms.Generic[
        Ore
    ],
):
    contents: list[ Ore ]
"#,
    ];

    for source in sources {
        assert_scoping(
            source,
            0,
            None,
            "PEP 484 binds a Generic base's TypeVar throughout that class body",
        )?;
    }
    Ok(())
}

#[test]
fn unrelated_typevar_in_generic_class_body_is_unbound() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import Generic as Family, TypeVar as VariableForge

Ore = VariableForge("Ore")
Slag = VariableForge("Slag")

class Crucible(Family[Ore]):
    rejected: list[Slag]
"#,
        r#"
import typing as type_forms

Ore = type_forms.TypeVar("Ore")
Slag = type_forms.TypeVar(
    "Slag",
)

class Crucible(
    type_forms.Generic[Ore],
):
    rejected: list[
        Slag
    ]
"#,
    ];

    for source in sources {
        assert_scoping(
            source,
            1,
            Some("Slag"),
            "PEP 484 does not bind an unrelated TypeVar merely because it appears in a generic class body",
        )?;
    }
    Ok(())
}

#[test]
fn same_spelled_attribute_is_not_the_module_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let sources = [
        r#"
from typing import TypeVar as VariableForge

Ore = VariableForge("Ore")

class Namespace:
    Ore = int

value: Namespace.Ore
"#,
        r#"
import typing as type_forms

Ore = type_forms.TypeVar( "Ore" )

class Namespace:
    Ore = str

value: Namespace . Ore
"#,
    ];

    for source in sources {
        assert_scoping(
            source,
            0,
            None,
            "PEP 484 TypeVar scope follows symbol identity; an attribute with the same token is unrelated",
        )?;
    }
    Ok(())
}
