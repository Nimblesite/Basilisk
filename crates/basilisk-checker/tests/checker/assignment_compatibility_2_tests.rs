//! Tests for [`assignment_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Edge cases for the enum literal-expansion equivalence of
// [TYPEINF-SUBTYPING-UNION] (`assignment_compatibility/enum_expand.rs`,
// GitHub #374): membership subtleties the base complete/partial-union tests
// in `assignment_compatibility_tests.rs` do not cover — single-member enums,
// `nonmember(...)` attributes, and unions naming a different enum's members.

use super::common::*;

#[test]
fn single_member_enum_assigned_to_its_literal_no_diagnostic(
) -> Result<(), Box<dyn std::error::Error>> {
    // A one-member enum's single literal IS the complete union.
    let source = r#"
from enum import Enum
from typing import Literal

class Single(Enum):
    ONLY = 1

def test(s: Single) -> None:
    x: Literal[Single.ONLY] = s
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "Literal[Single.ONLY] is the complete union of a one-member enum; \
         should not fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn nonmember_attribute_does_not_count_toward_completeness() -> Result<(), Box<dyn std::error::Error>>
{
    // `nonmember(...)` attributes and sunder/dunder names are not enum
    // members, so the union of the two real members is still complete.
    let source = r#"
from enum import Enum, nonmember
from typing import Literal

class Answer(Enum):
    Yes = 1
    No = 2
    helper = nonmember(3)

def test(a: Answer) -> None:
    x: Literal[Answer.Yes, Answer.No] = a
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "nonmember attributes are not members; Literal[Yes, No] is complete, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn literal_union_of_wrong_enum_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The union names another enum's members; assigning Answer must fire.
    let source = r#"
from enum import Enum
from typing import Literal

class Answer(Enum):
    Yes = 1
    No = 2

class Other(Enum):
    A = 1
    B = 2

def test(a: Answer) -> None:
    x: Literal[Other.A, Other.B] = a
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "a literal union of a DIFFERENT enum's members must reject the value, got no diagnostic"
    );
    Ok(())
}
