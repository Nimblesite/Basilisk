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
fn pep_484_class_object_attributes_follow_the_resolved_class(
) -> Result<(), Box<dyn std::error::Error>> {
    // PEP 484 defines `type[C]` as the class-object type for C and its
    // subclasses. Attribute lookup must therefore use that resolved class,
    // regardless of how builtin `type` or C is spelled.
    // https://peps.python.org/pep-0484/#the-type-of-class-objects
    let rejected = [
        (
            "canonical builtins",
            r#"
def inspect(candidate: type[object]) -> None:
    candidate.not_a_class_attribute
"#,
        ),
        (
            "aliased builtins",
            r#"
from builtins import object as Root, type as ClassObject
def inspect(candidate: ClassObject[Root]) -> None:
    candidate.not_a_class_attribute
"#,
        ),
        (
            "qualified builtins",
            r#"
import builtins as runtime
def inspect(candidate: runtime.type[runtime.object]) -> None:
    candidate.not_a_class_attribute
"#,
        ),
        (
            "renamed user class",
            r#"
class Archive:
    seal: int
def inspect(candidate: type[Archive]) -> None:
    candidate.missing_seal
"#,
        ),
        (
            "reformatted annotation",
            r#"
class Observatory:
    aperture: int
def inspect(
    candidate:
        type[
            Observatory
        ],
) -> None:
    candidate.missing_aperture
"#,
        ),
    ];

    for (mutation, source) in rejected {
        let diagnostics = run(source)?;
        assert_eq!(
            diagnostics.len(),
            1,
            "{mutation}: one missing class-object attribute must produce one diagnostic: {diagnostics:#?}"
        );
        assert_eq!(
            codes(&diagnostics),
            vec!["specialtypes_type"],
            "{mutation}: the class-object attribute rule itself must reject the access"
        );
        assert_eq!(
            messages_for(&diagnostics, "specialtypes_type").len(),
            1,
            "{mutation}: an unrelated diagnostic cannot satisfy the PEP 484 obligation"
        );
    }

    let accepted = [
        (
            "canonical metaclass attribute",
            r#"
def inspect(candidate: type[object]) -> str:
    return candidate.__name__
"#,
        ),
        (
            "aliased metaclass attribute",
            r#"
from builtins import object as Root, type as ClassObject
def inspect(candidate: ClassObject[Root]) -> tuple[type, ...]:
    return candidate.__mro__
"#,
        ),
        (
            "qualified metaclass attribute",
            r#"
import builtins as runtime
def inspect(candidate: runtime.type[runtime.object]) -> str:
    return candidate.__qualname__
"#,
        ),
        (
            "declared user class attribute",
            r#"
class Archive:
    seal: int
def inspect(candidate: type[Archive]) -> int:
    return candidate.seal
"#,
        ),
        (
            "reformatted declared attribute",
            r#"
class Observatory:
    aperture: int
def inspect(
    candidate:
        type[
            Observatory
        ],
) -> int:
    return candidate.aperture
"#,
        ),
    ];

    for (mutation, source) in accepted {
        let diagnostics = run(source)?;
        assert!(
            diagnostics.is_empty(),
            "{mutation}: a real class-object attribute must be accepted: {diagnostics:#?}"
        );
        assert_eq!(
            codes(&diagnostics),
            Vec::<&str>::new(),
            "{mutation}: equivalent spellings must preserve acceptance"
        );
    }

    Ok(())
}
