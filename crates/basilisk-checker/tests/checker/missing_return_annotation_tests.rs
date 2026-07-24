//! Tests for [BSK-0002] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
// Integration tests for BSK-0002: Missing return type annotation.

use super::common::*;

#[test]
fn missing_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str):
    return name
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0002"),
        "function without return annotation should fire BSK-0002, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn with_return_annotation_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def greet(name: str) -> str:
    return name
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0002"),
        "function with return annotation should not fire BSK-0002"
    );
    Ok(())
}

#[test]
fn none_return_annotation_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def do_nothing() -> None:
    pass
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0002"),
        "function with -> None should not fire BSK-0002"
    );
    Ok(())
}

// ── Inference-aware exemption [TYPEINF-FUNC-RETURN]: a missing-annotation rule
// must never fire where the current engine already infers the type. ──────────

/// The return type of `return 42` is trivially inferable (`int`), so demanding
/// an annotation is redundant — BSK-0002 must stay silent.
#[test]
fn return_type_inferable_from_literal_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def answer():
    return 42
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0002"),
        "return type inferable from literal return should not fire BSK-0002, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A function whose body never returns a value has the inferable return type
/// `None` — BSK-0002 must stay silent.
#[test]
fn no_return_statements_infers_none_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def log_it(msg: str):
    print(msg)
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0002"),
        "no-return function infers -> None and should not fire BSK-0002, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Mixed inferable literals (`int` / `str`) still form an inferable union —
/// BSK-0002 must stay silent.
#[test]
fn return_type_inferable_from_literal_union_no_fire() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
def pick(flag: bool):
    if flag:
        return 1
    return "no"
"#;
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        !codes(&diags).contains(&"BSK-0002"),
        "union of inferable literal returns should not fire BSK-0002, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A returned call expression is NOT inferable by the current engine, so the
/// annotation is still required.
#[test]
fn uninferable_call_return_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def fetch():
    return make_value()
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0002"),
        "uninferable returned call must still fire BSK-0002, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// Generators produce `Generator[...]`, which the current engine cannot infer
/// from return statements — the annotation is still required.
#[test]
fn generator_still_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
def numbers():
    yield 1
";
    let diags = run_with_config(source, &annotation_rules_config())?;
    assert!(
        codes(&diags).contains(&"BSK-0002"),
        "generator return type is not inferable and must still fire BSK-0002, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
