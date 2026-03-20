// Integration tests for BSK-E0024: Invalid type form (numeric literal as annotation).

use super::common::*;

#[test]
fn e0024_numeric_literal_param_annotation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: 42) -> None:\n    pass\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0024"),
        "numeric literal param annotation should fire E0024, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0024_numeric_literal_return_annotation_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: int) -> 0:\n    pass\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0024"),
        "numeric literal return annotation should fire E0024, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0024_normal_type_annotation_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def f(x: int) -> str:\n    return str(x)\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0024"),
        "normal type annotation should not fire E0024"
    );
    Ok(())
}
