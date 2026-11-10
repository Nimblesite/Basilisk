//! Tests for [`returns_compatibility_2`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for returns_compatibility_2: Return type mismatch (inference-based).

use super::common::*;

#[test]
fn return_list_for_str_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def get_name() -> str:
    return [1, 2, 3]
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"returns_compatibility_2")
            || codes(&diags).contains(&"returns_compatibility"),
        "returning list for str should fire E0013 or E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn correct_return_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def get_name() -> str:
    return "hello"
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility_2"),
        "correct return type should not fire E0013"
    );
    Ok(())
}

#[test]
fn pep_484_none_return_must_match_the_declared_return_type(
) -> Result<(), Box<dyn std::error::Error>> {
    // PEP 484 requires checked function bodies to be consistent with their
    // annotations. `None` is not assignable to these non-optional return types.
    // https://peps.python.org/pep-0484/#the-meaning-of-annotations
    let rejected = [
        (
            "canonical builtin",
            r#"
def count() -> int:
    return None
"#,
        ),
        (
            "aliased builtin",
            r#"
from builtins import str as Label
def identify() -> Label:
    return None
"#,
        ),
        (
            "qualified builtin",
            r#"
import builtins as runtime
def ratio() -> runtime.float:
    return None
"#,
        ),
        (
            "renamed class and reformatted return",
            r#"
class Ledger: ...
def open_ledger(
) -> Ledger:
    return (
        None
    )
"#,
        ),
    ];

    for (mutation, source) in rejected {
        let diagnostics = run(source)?;
        assert_eq!(
            diagnostics.len(),
            1,
            "{mutation}: one incompatible return must produce one isolated diagnostic: {diagnostics:#?}"
        );
        assert_eq!(
            codes(&diagnostics),
            vec!["returns_compatibility_2"],
            "{mutation}: the PEP 484 return rule itself must reject `None`"
        );
        assert_rule_count(
            &diagnostics,
            "returns_compatibility_2",
            1,
            "PEP 484 None returned for a non-optional type",
        );
    }

    Ok(())
}

#[test]
fn empty_list_return_is_valid_for_union_of_lists() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def values() -> list[int] | list[str]:
    return []
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"returns_compatibility_2"),
        "empty list literal must use either compatible union return context, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
