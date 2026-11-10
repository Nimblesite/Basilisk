//! PEP 702: uses of deprecated classes, functions, methods, and properties.
//! Specifications: https://peps.python.org/pep-0702/#type-checker-behavior and
//! https://typing.python.org/en/latest/spec/directives.html#deprecated

mod common;

use common::{assert_rule_count, run};

const RULE: &str = "directives_deprecated";

fn assert_one_deprecation(
    source: &str,
    obligation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(source)?;
    assert_rule_count(&diagnostics, RULE, 1, obligation);
    Ok(())
}

#[test]
fn deprecated_function_use_is_reported_for_resolved_aliases(
) -> Result<(), Box<dyn std::error::Error>> {
    for source in [
        r#"
from typing_extensions import deprecated as obsolete

@obsolete("use quarry instead")
def prospect() -> int:
    return 1

value = prospect()
"#,
        r#"
import typing_extensions as extensions

@extensions.deprecated("use quarry instead")
def prospect() -> int:
    return 1

value = prospect()
"#,
        r#"
from typing_extensions import deprecated as obsolete

@obsolete(
    "use quarry instead",
)
def prospect() -> int:
    return 1

value = prospect(
)
"#,
    ] {
        assert_one_deprecation(
            source,
            "PEP 702 requires a diagnostic for a use of a deprecated function",
        )?;
    }
    Ok(())
}

#[test]
fn deprecated_class_instantiation_is_reported() -> Result<(), Box<dyn std::error::Error>> {
    assert_one_deprecation(
        r#"
from typing_extensions import deprecated as obsolete

@obsolete("use NewLedger")
class OldLedger:
    pass

ledger = OldLedger()
"#,
        "PEP 702 requires a diagnostic for a use of a deprecated class",
    )
}

#[test]
fn deprecated_method_call_is_reported() -> Result<(), Box<dyn std::error::Error>> {
    assert_one_deprecation(
        r#"
from typing_extensions import deprecated as obsolete

class Ledger:
    @obsolete("use reconcile")
    def audit(self) -> None:
        pass

Ledger().audit()
"#,
        "PEP 702 requires a diagnostic for an instance-attribute use of a deprecated method",
    )
}

#[test]
fn nondeprecated_use_is_not_reported() -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = run(r#"
def quarry() -> int:
    return 1

value = quarry()
"#)?;
    assert_rule_count(
        &diagnostics,
        RULE,
        0,
        "PEP 702 applies only to objects marked deprecated",
    );
    Ok(())
}
