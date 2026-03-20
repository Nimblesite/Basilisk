// Integration tests for BSK-E0011: Explicit Any / return type mismatch.

use super::common::*;

#[test]
fn e0011_explicit_any_param_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import Any\n\ndef greet(name: Any) -> str:\n    return name\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0011"),
        "explicit Any param annotation should fire E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0011_explicit_any_return_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "from typing import Any\n\ndef greet(name: str) -> Any:\n    return name\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0011"),
        "explicit Any return annotation should fire E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0011_return_type_mismatch_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def count() -> str:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0011"),
        "returning int from -> str function should fire E0011, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0011_correct_return_type_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def count() -> int:\n    return 42\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0011"),
        "correct return type should not fire E0011"
    );
    Ok(())
}

#[test]
fn e0011_str_annotation_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def greet(name: str) -> str:\n    return name\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0011"),
        "str annotation should not fire E0011"
    );
    Ok(())
}

#[test]
fn e0011_explicit_any_fires_even_on_stub() -> Result<(), Box<dyn std::error::Error>> {
    // Explicit Any check fires even on stub bodies (only return mismatch is guarded)
    let source = "from typing import Any\n\ndef greet(name: Any) -> str:\n    ...\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0011"),
        "explicit Any should fire E0011 even on stub body, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0011_return_mismatch_stub_exempt() -> Result<(), Box<dyn std::error::Error>> {
    // Return type mismatch check is skipped for stub bodies
    let source = "def count() -> str:\n    ...\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0011"),
        "stub body should not fire return type mismatch E0011"
    );
    Ok(())
}
