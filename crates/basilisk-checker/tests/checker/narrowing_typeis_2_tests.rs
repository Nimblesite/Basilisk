//! Tests for [`narrowing_typeis_2`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for narrowing_typeis_2: `TypeIs` inconsistent narrowing.

use super::common::*;

// Exercises [TYPEINF-NARROWING-TYPEIS] — PEP 742 consistency precondition:
// the narrowed type must be a subtype of the input parameter type.
#[test]
fn valid_typeis() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def is_str(x: object) -> TypeIs[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"narrowing_typeis_2"),
        "valid TypeIs should not fire E0113"
    );
    Ok(())
}

#[test]
fn inconsistent_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

def bad_check(x: int) -> TypeIs[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// Exercises [TYPEINF-ANNOTATION-RESOLUTION] — the narrowed target resolves
// through the alias table before the consistency judgment, so an alias of the
// parameter type is consistent, not an opaque mismatched name
// (Stage 0.5 bidir wiring).
#[test]
fn aliased_narrowed_type_is_consistent() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

MyAlias = str

def is_my(x: str) -> TypeIs[MyAlias]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"narrowing_typeis_2"),
        "`TypeIs[MyAlias]` where `MyAlias = str` narrows `str` to `str`; \
         comparing the unresolved alias name is a false positive"
    );
    Ok(())
}

// Exercises [TYPEINF-SUBTYPING-NOMINAL] through the resolved cascade — a
// same-module subclass is consistent with its base as a narrowing target.
#[test]
fn subclass_narrowed_type_is_consistent() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

class Base:
    pass

class MyClass(Base):
    pass

def is_mine(x: Base) -> TypeIs[MyClass]:
    return isinstance(x, MyClass)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"narrowing_typeis_2"),
        "narrowing `Base` to its subclass `MyClass` is the canonical TypeIs \
         use; the nominal walk must see the resolved class, not opaque text"
    );
    Ok(())
}

// The resolution work must not blunt the rule: a resolved alias that IS
// inconsistent still fires.
#[test]
fn aliased_narrowed_type_still_fires_when_inconsistent() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

MyAlias = str

def bad(x: int) -> TypeIs[MyAlias]:
    return False
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"narrowing_typeis_2"),
        "`MyAlias` resolves to `str`, which cannot narrow an `int` input"
    );
    Ok(())
}
