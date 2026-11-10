//! Tests for the return-position engine synthesis — [NARROWPLAN-INTEGRATION]
//! Step 2, [TYPEINF-FUNC-RETURN]. See
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION
//!
//! The return half of [#378](https://github.com/Nimblesite/Basilisk/issues/378):
//! the pre-engine rules skipped EVERY call in a return position because they
//! had no way to type one. The module oracle resolves a call through its
//! callee's declared return, so the mismatches fire and the abstentions the
//! gradual guarantee requires still hold.

use super::common::*;

/// Both return-mismatch rules judge the same statement; either firing is a
/// catch, and neither firing is silence.
fn return_messages(diags: &[basilisk_checker::Diagnostic]) -> Vec<&str> {
    let mut msgs = messages_for(diags, "returns_compatibility");
    msgs.extend(messages_for(diags, "returns_compatibility_2"));
    msgs
}

#[test]
fn returned_call_with_wrong_declared_return_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def helper() -> int:
    return 1


def outer() -> str:
    return helper()
"#;
    let diags = run(source)?;
    assert!(
        !return_messages(&diags).is_empty(),
        "returning `int` from a `-> str` function must fire"
    );
    Ok(())
}

#[test]
fn returned_call_with_matching_return_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def helper() -> int:
    return 1


def outer() -> int:
    return helper()
"#;
    let diags = run(source)?;
    let msgs = return_messages(&diags);
    assert!(
        msgs.is_empty(),
        "a matching declared return must stay silent, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn returned_none_call_in_none_function_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // The `-> None` rule fires on the SHAPE of a valued return; the engine
    // disproves it here, because `helper()` really is `None`.
    let source = r#"
def helper() -> None:
    return


def outer() -> None:
    return helper()
"#;
    let diags = run(source)?;
    let msgs = return_messages(&diags);
    assert!(
        msgs.is_empty(),
        "`return helper()` where `helper() -> None` is legal, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn returned_valued_call_in_none_function_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def helper() -> int:
    return 1


def outer() -> None:
    return helper()
"#;
    let diags = run(source)?;
    assert!(
        !return_messages(&diags).is_empty(),
        "`return helper()` where `helper() -> int` must fire in a `-> None` function"
    );
    Ok(())
}

#[test]
fn returned_unresolvable_call_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // [TYPEINF-TARGET-GRADUAL]: an unannotated callee stays gradual, so
    // widening from "skip every call" to "judge every call" adds no false
    // positives on unannotated code.
    let source = r#"
def helper():
    return 1


def outer() -> None:
    return helper()
"#;
    let diags = run(source)?;
    let msgs = return_messages(&diags);
    assert!(
        msgs.is_empty(),
        "an unannotated callee must not manufacture a return error, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn returned_subclass_instance_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Nominal verdicts route through `SubtypingContext`.
    let source = r#"
class Base:
    pass


class Derived(Base):
    pass


def make() -> Base:
    return Derived()
"#;
    let diags = run(source)?;
    let msgs = return_messages(&diags);
    assert!(
        msgs.is_empty(),
        "returning a subclass instance is legal, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn returned_unrelated_instance_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
class Left:
    pass


class Right:
    pass


def make() -> Left:
    return Right()
"#;
    let diags = run(source)?;
    assert!(
        !return_messages(&diags).is_empty(),
        "returning an unrelated class instance must fire"
    );
    Ok(())
}

#[test]
fn yielded_call_with_wrong_type_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The generator family rides the same oracle.
    let source = r#"
from typing import Iterator


def helper() -> int:
    return 1


def gen() -> Iterator[str]:
    yield helper()
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "annotations_generators");
    assert!(
        !msgs.is_empty(),
        "yielding `int` from an `Iterator[str]` generator must fire"
    );
    Ok(())
}

#[test]
fn yielded_call_with_matching_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
from typing import Iterator


def helper() -> str:
    return "x"


def gen() -> Iterator[str]:
    yield helper()
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "annotations_generators");
    assert!(
        msgs.is_empty(),
        "yielding a matching call result must stay silent, got: {msgs:?}"
    );
    Ok(())
}
