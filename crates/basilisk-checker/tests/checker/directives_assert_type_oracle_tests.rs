//! Tests for the oracle half of `directives_assert_type_2` —
//! [NARROWPLAN-INTEGRATION] Step 5, [CHKARCH-DIAG-STRUCTURAL]. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION
//!
//! [#290](https://github.com/Nimblesite/Basilisk/issues/290): expressions the
//! resolver cannot type — call results above all — are judged by the SAME
//! span-indexed engine hover reads, and fire only on a provably disjoint
//! verdict.

use super::common::*;

#[test]
fn assert_type_on_call_result_disjoint_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import assert_type


def make() -> int:
    return 1


assert_type(make(), str)
"#;
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"directives_assert_type_2"),
        "a call known to return `int` asserted as `str` must fire, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn assert_type_on_call_result_matching_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import assert_type


def make() -> int:
    return 1


assert_type(make(), int)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_assert_type_2"),
        "a matching call-result assertion must stay silent, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn assert_type_literal_widening_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // `Literal[1]` is assignable to `int`, so the pair is not disjoint and
    // the engine abstains — exactly the spec's tolerance for literal
    // expressions in `assert_type(1, int)`.
    let source = r#"
from typing import assert_type

assert_type(1, int)
assert_type("x", str)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_assert_type_2"),
        "literal widening must never manufacture an assert_type error, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn assert_type_unresolvable_call_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // [TYPEINF-TARGET-GRADUAL]: an unannotated callee stays gradual.
    let source = r#"
from typing import assert_type


def make():
    return 1


assert_type(make(), str)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_assert_type_2"),
        "an untyped callee must abstain, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn assert_type_generic_call_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // A `TypeVar` return the module cannot ground is a question, not an
    // answer ([CHKARCH-CONFORMANCE-MODE]).
    let source = r#"
from typing import TypeVar, assert_type

T = TypeVar("T")


def identity(value: T) -> T:
    return value


assert_type(identity(1), int)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"directives_assert_type_2"),
        "an unsolved generic call must abstain, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
