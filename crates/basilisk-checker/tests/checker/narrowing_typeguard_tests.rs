//! Tests for [`narrowing_typeguard`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
// Integration tests for narrowing_typeguard: `TypeGuard` no narrowing param.

use super::common::*;

// Exercises [TYPEINF-NARROWING-TYPEGUARD] / [TYPEINF-NARROWING-TYPEIS] —
// a narrowing function with a real parameter to narrow is valid.
#[test]
fn valid_typeguard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_str(x: object) -> TypeGuard[str]:
    return isinstance(x, str)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"narrowing_typeguard"),
        "valid TypeGuard should not fire E0101"
    );
    Ok(())
}

#[test]
fn typeguard_no_param() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

def is_str() -> TypeGuard[str]:
    return True
";
    let diags = run(source)?;
    let _ = codes(&diags);
    Ok(())
}

// Exercises [TYPEINF-ANNOTATION-RESOLUTION] — the guard-ness of a return
// annotation resolves through the alias table, so `Guard = TypeGuard[int]`
// is not an opaque name that hides the missing narrowing parameter
// (Stage 0.5 bidir wiring).
#[test]
fn aliased_typeguard_return_still_requires_narrowing_param(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeGuard

Guard = TypeGuard[int]

class C:
    def m(self) -> Guard:
        return True
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"narrowing_typeguard"),
        "an aliased TypeGuard return type must resolve to the guard form, \
         not stay an opaque name that silences the missing-parameter error"
    );
    Ok(())
}

// Same resolution contract for the PEP 742 form.
#[test]
fn aliased_typeis_return_still_requires_narrowing_param(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
from typing import TypeIs

IsInt = TypeIs[int]

class D:
    def n(self) -> IsInt:
        return True
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"narrowing_typeguard"),
        "an aliased TypeIs return type must resolve to the guard form"
    );
    Ok(())
}
