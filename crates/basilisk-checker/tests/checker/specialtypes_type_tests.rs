//! Tests for [`specialtypes_type`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for specialtypes_type: Invalid type[X] usage violations.

use super::common::*;

/// PEP 484 requires `type[U]` arguments to be class objects whose represented
/// classes are compatible with `U`. Import spelling and formatting do not
/// change that relation.
///
/// <https://peps.python.org/pep-0484/#the-type-of-class-objects>
#[test]
fn pep_484_class_object_union_is_resolved_by_symbol_identity(
) -> Result<(), Box<dyn std::error::Error>> {
    let rejected = [
        (
            "canonical builtin spelling",
            r#"
class Cedar: ...
class Flint: ...
class Kestrel: ...
def admit(candidate: type[Cedar | Flint]) -> None: ...
admit(Kestrel)
"#,
        ),
        (
            "aliased builtin import",
            r#"
from builtins import type as ClassObject
class Cedar: ...
class Flint: ...
class Kestrel: ...
def admit(candidate: ClassObject[Cedar | Flint]) -> None: ...
admit(Kestrel)
"#,
        ),
        (
            "qualified builtin import",
            r#"
import builtins as runtime
class Cedar: ...
class Flint: ...
class Kestrel: ...
def admit(candidate: runtime.type[Cedar | Flint]) -> None: ...
admit(Kestrel)
"#,
        ),
        (
            "reformatted union",
            r#"
class Cedar: ...
class Flint: ...
class Kestrel: ...
def admit(
    candidate: type[
        Cedar
        | Flint
    ],
) -> None: ...
admit(Kestrel)
"#,
        ),
    ];

    for (mutation, source) in rejected {
        let diagnostics = run(source)?;
        assert_eq!(
            diagnostics.len(),
            1,
            "{mutation}: the incompatible class object must produce one isolated diagnostic: {diagnostics:#?}"
        );
        assert_eq!(
            codes(&diagnostics),
            vec!["specialtypes_type"],
            "{mutation}: the PEP 484 class-object rule itself must reject the argument"
        );
        assert_eq!(
            messages_for(&diagnostics, "specialtypes_type").len(),
            1,
            "{mutation}: an unrelated diagnostic is not proof of this obligation"
        );
    }

    let accepted = [
        (
            "canonical accepted member",
            r#"
class Cedar: ...
class Flint: ...
def admit(candidate: type[Cedar | Flint]) -> None: ...
admit(Cedar)
"#,
        ),
        (
            "aliased accepted member",
            r#"
from builtins import type as ClassObject
class Cedar: ...
class Flint: ...
def admit(candidate: ClassObject[Cedar | Flint]) -> None: ...
admit(Flint)
"#,
        ),
        (
            "qualified accepted member",
            r#"
import builtins as runtime
class Cedar: ...
class Flint: ...
def admit(candidate: runtime.type[Cedar | Flint]) -> None: ...
admit(Cedar)
"#,
        ),
        (
            "reformatted accepted member",
            r#"
class Cedar: ...
class Flint: ...
def admit(
    candidate: type[
        Cedar | Flint
    ],
) -> None: ...
admit(Flint)
"#,
        ),
    ];

    for (mutation, source) in accepted {
        let diagnostics = run(source)?;
        assert!(
            diagnostics.is_empty(),
            "{mutation}: a union member class object must be accepted: {diagnostics:#?}"
        );
        assert_eq!(
            codes(&diagnostics),
            Vec::<&str>::new(),
            "{mutation}: equivalent spellings must not invent a diagnostic"
        );
    }

    Ok(())
}

#[test]
fn valid_type_usage() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import TypeVar

T = TypeVar("T")

class A: ...

def func(x: type[A]) -> None:
    pass

func(A)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"specialtypes_type"),
        "valid type usage should not fire E0145"
    );
    Ok(())
}

#[test]
fn callable_as_type() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Callable, TypeVar

T = TypeVar("T")

def func5(x: type[T]) -> None:
    pass

func5(Callable)
"#;
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

#[test]
fn unknown_attr_on_type_object() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def func8(a: type[object]) -> None:
    a.unknown
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}
