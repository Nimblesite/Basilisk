//! Tests for [`assignment_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Enum-expansion equivalence (typing spec, enums chapter): "a type checker
// should treat a complete union of all literal members as equivalent to the
// enum type". Added for python/typing@a490662's broadened enums_expansion
// conformance test (test4): an enum-typed value assigned to a Literal union
// naming EVERY member must not fire assignment_compatibility.

use super::common::*;

#[test]
fn enum_assigned_to_complete_literal_union_no_diagnostic() -> Result<(), Box<dyn std::error::Error>>
{
    // Mirrors conformance enums_expansion.py test4: Literal[Answer.Yes,
    // Answer.No] covers every member of Answer, so it is equivalent to Answer.
    let source = r#"
from enum import Enum
from typing import Literal

class Answer(Enum):
    Yes = 1
    No = 2

def test4(a: Answer) -> None:
    x: Literal[Answer.Yes, Answer.No] = a
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "a complete union of all literal members is equivalent to the enum \
         type (typing spec, enums chapter); should not fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn enum_assigned_to_incomplete_literal_union_fires() -> Result<(), Box<dyn std::error::Error>> {
    // Maybe is missing from the union, so Answer is NOT assignable to it.
    let source = r#"
from enum import Enum
from typing import Literal

class Answer(Enum):
    Yes = 1
    No = 2
    Maybe = 3

def test(a: Answer) -> None:
    x: Literal[Answer.Yes, Answer.No] = a
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "an INCOMPLETE literal union must still reject the enum type, got no diagnostic"
    );
    Ok(())
}

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
