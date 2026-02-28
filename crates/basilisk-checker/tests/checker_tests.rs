//! Integration tests for basilisk-checker.

use basilisk_checker::{check, Severity};
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(src: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

#[test]
fn no_diagnostics_for_fully_annotated_function() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def greet(name: str) -> str:\n    return name\n")?;
    assert!(
        diags.is_empty(),
        "fully annotated function should produce no diagnostics"
    );
    Ok(())
}

#[test]
fn emits_e0001_for_missing_parameter_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def process(data) -> None:\n    pass\n")?;
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.code, "BSK-E0001");
    assert_eq!(diags[0].severity, Severity::Error);
    Ok(())
}

#[test]
fn emits_e0002_for_missing_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def process(data: str):\n    pass\n")?;
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.code, "BSK-E0002");
    assert_eq!(diags[0].severity, Severity::Error);
    Ok(())
}

#[test]
fn emits_both_for_unannotated_function() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def process(data):\n    pass\n")?;
    assert_eq!(diags.len(), 2, "should emit E0001 and E0002");

    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(codes.contains(&"BSK-E0001"));
    assert!(codes.contains(&"BSK-E0002"));
    Ok(())
}

#[test]
fn emits_one_e0001_per_unannotated_parameter() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def multi(a, b, c) -> None:\n    pass\n")?;
    let count = diags.iter().filter(|d| d.code.code == "BSK-E0001").count();
    assert_eq!(
        count, 3,
        "three unannotated params should produce three E0001s"
    );
    Ok(())
}

#[test]
fn handles_empty_file() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("")?;
    assert!(diags.is_empty());
    Ok(())
}

#[test]
fn all_diagnostics_have_nonempty_message() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def bad(x):\n    pass\n")?;
    for d in &diags {
        assert!(!d.message.is_empty());
    }
    Ok(())
}

#[test]
fn all_diagnostics_have_docs_url() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run("def bad(x):\n    pass\n")?;
    for d in &diags {
        assert!(d.code.docs_url.starts_with("https://"));
    }
    Ok(())
}

#[test]
fn severity_error_displays_as_error() {
    assert_eq!(format!("{}", Severity::Error), "error");
}

#[test]
fn severity_warning_displays_as_warning() {
    assert_eq!(format!("{}", Severity::Warning), "warning");
}

#[test]
fn severity_error_greater_than_warning() {
    assert!(Severity::Error > Severity::Warning);
}
