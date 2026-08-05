//! Tests for [`assignment_compatibility`] call/name synthesis through the
//! module oracle — [NARROWPLAN-INTEGRATION] Step 1, [TYPEINF-TARGET-BIDIRECTIONAL].
//! See docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION
//!
//! GitHub #397 / #378: the pre-engine rule only judged literal right-hand
//! sides, so `a: int = returns_str()` sailed through. The engine's
//! `synth_call` resolves a call through its callee's DECLARED return, and the
//! assignment judgment now sees it. The guards pin the abstentions that keep
//! the wider sight from manufacturing false positives: nominal subclassing
//! routes through `SubtypingContext`, and a bare class name is a class
//! OBJECT, not an instance.

use super::common::*;

#[test]
fn int_annotated_call_returning_str_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The #397 mandate case: a call RHS is typed by its declared return.
    let source = r#"
def returns_str() -> str:
    return "hello"


a: int = returns_str()
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "`a: int = returns_str()` must fire: the callee's declared return is `str`"
    );
    Ok(())
}

#[test]
fn matching_call_return_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def returns_str() -> str:
    return "hello"


a: str = returns_str()
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "a call whose declared return matches the annotation must not fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn local_int_annotated_call_returning_str_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The same judgment inside a function body — #378's assignment half.
    let source = r#"
def returns_str() -> str:
    return "hello"


def use() -> None:
    a: int = returns_str()
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "a local `a: int = returns_str()` must fire like the module-level form"
    );
    Ok(())
}

#[test]
fn constructor_call_to_base_annotation_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Nominal subclassing routes through the module's registered hierarchy:
    // `Derived()` IS a `Base` ([NARROWPLAN-INTEGRATION] SubtypingContext).
    let source = r#"
class Base:
    pass


class Derived(Base):
    pass


x: Base = Derived()
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "`x: Base = Derived()` is nominal subclassing; must not fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn constructor_call_to_unrelated_class_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The subclass walk must not degrade into blanket acceptance.
    let source = r#"
class Left:
    pass


class Right:
    pass


x: Left = Right()
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "`x: Left = Right()` relates two unrelated classes and must fire"
    );
    Ok(())
}

#[test]
fn bare_class_name_to_type_annotation_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // A bare class name denotes the class OBJECT — the oracle abstains so
    // `x: type[C] = C` never reads as "an instance of C vs type[C]".
    let source = r#"
class C:
    pass


x: type[C] = C
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "`x: type[C] = C` assigns the class object; must not fire, got: {msgs:?}"
    );
    Ok(())
}

#[test]
fn annotated_variable_reference_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    // A parameter name resolves through the engine's scope overlay.
    let source = r#"
def copy_it(source: str) -> None:
    target: int = source
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        !msgs.is_empty(),
        "`target: int = source` with `source: str` must fire through the scope overlay"
    );
    Ok(())
}

#[test]
fn undeclared_return_call_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    // Enforcement-grade seeding ([TYPEINF-TARGET-GRADUAL]): a callee with no
    // DECLARED return contributes nothing — removing an annotation must never
    // add errors, so the synthesized `str` is not enforced here.
    let source = r#"
def returns_str():
    return "hello"


a: int = returns_str()
"#;
    let diags = run(source)?;
    let msgs = messages_for(&diags, "assignment_compatibility");
    assert!(
        msgs.is_empty(),
        "an undeclared return is display-grade only; must not fire, got: {msgs:?}"
    );
    Ok(())
}
