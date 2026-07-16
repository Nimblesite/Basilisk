//! Tests for [BSK-0001] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
// Integration tests for BSK-0001: Missing parameter type annotation.

use super::common::*;

#[test]
fn missing_param_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name):
    return name
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0001"),
        "unannotated parameter should fire BSK-0001, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn annotated_param_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str) -> str:
    return name
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0001"),
        "annotated parameter should not fire BSK-0001"
    );
    Ok(())
}

#[test]
fn self_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Foo:
    def method(self) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0001"),
        "self parameter should not fire BSK-0001"
    );
    Ok(())
}

#[test]
fn cls_exempt() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Foo:
    @classmethod
    def method(cls) -> None:
        pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0001"),
        "cls parameter should not fire BSK-0001"
    );
    Ok(())
}

// ── Inference-aware exemption [TYPEINF-FUNC-DEFAULTS]: a missing-annotation
// rule must never fire where the current engine already infers the type. ─────

/// `timeout=30` makes the parameter type trivially inferable (`int`), so
/// demanding an annotation is redundant — BSK-0001 must stay silent.
#[test]
fn param_with_inferable_literal_default_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def connect(timeout=30) -> None:
    pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0001"),
        "parameter type inferable from literal default should not fire BSK-0001, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// String and bool literal defaults are equally inferable — BSK-0001 must stay
/// silent for every parameter here.
#[test]
fn params_with_scalar_literal_defaults_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def configure(name="basilisk", strict=True, ratio=1.5) -> None:
    pass
"#;
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0001"),
        "scalar literal defaults are inferable and should not fire BSK-0001, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A default the current engine cannot infer (call expression) still requires
/// an annotation.
#[test]
fn param_with_uninferable_default_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def connect(timeout=default_timeout()) -> None:
    pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0001"),
        "uninferable default must still fire BSK-0001, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A `None` default alone does not determine the parameter type (`T | None`
/// for unknown `T`) — the annotation is still required.
#[test]
fn param_with_none_default_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def connect(timeout=None) -> None:
    pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0001"),
        "a bare None default cannot determine the type and must still fire BSK-0001, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn multiple_unannotated_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def add(a, b):
    return a + b
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    let e0001_count = codes(&diags).iter().filter(|c| **c == "BSK-0001").count();
    assert!(
        e0001_count >= 2,
        "two unannotated params should fire BSK-0001 at least twice, got {e0001_count}"
    );
    Ok(())
}
