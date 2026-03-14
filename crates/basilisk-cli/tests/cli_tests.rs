#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! End-to-end integration tests for the full pipeline.
//!
//! These tests exercise parse -> resolve -> check using real Python fixture
//! files. They do NOT test CLI argument parsing — they test the pipeline
//! that powers the CLI.

use std::path::Path;

use basilisk_checker::{check, Severity};
use basilisk_parser::parse_file;
use basilisk_resolver::resolve;

fn fixture(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest)
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn check_fixture(
    name: &str,
) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let path = fixture(name);
    let parsed = parse_file(&path)?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

#[test]
fn all_annotated_produces_no_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    let diags = check_fixture("all_annotated.py")?;
    assert!(
        diags.is_empty(),
        "all_annotated.py should produce zero diagnostics, got: {diags:#?}"
    );
    Ok(())
}

#[test]
fn missing_param_annotation_produces_only_e0001() -> Result<(), Box<dyn std::error::Error>> {
    let diags = check_fixture("missing_param_annotation.py")?;
    assert!(!diags.is_empty(), "should have diagnostics");
    assert!(
        diags.iter().all(|d| d.code.code == "BSK-E0001"),
        "all diagnostics should be E0001, got: {diags:#?}"
    );
    // `process` has 1 unannotated param, `transform` has 1 unannotated param
    assert_eq!(
        diags.len(),
        2,
        "expected 2 E0001 diagnostics, got {}",
        diags.len()
    );
    Ok(())
}

#[test]
fn missing_return_annotation_produces_only_e0002() -> Result<(), Box<dyn std::error::Error>> {
    let diags = check_fixture("missing_return_annotation.py")?;
    assert!(!diags.is_empty(), "should have diagnostics");
    assert!(
        diags.iter().all(|d| d.code.code == "BSK-E0002"),
        "all diagnostics should be E0002, got: {diags:#?}"
    );
    assert_eq!(diags.len(), 2, "two functions without return annotations");
    Ok(())
}

#[test]
fn missing_both_produces_e0001_and_e0002() -> Result<(), Box<dyn std::error::Error>> {
    let diags = check_fixture("missing_both.py")?;
    let codes: Vec<&str> = diags.iter().map(|d| d.code.code).collect();
    assert!(codes.contains(&"BSK-E0001"), "should contain E0001");
    assert!(codes.contains(&"BSK-E0002"), "should contain E0002");
    assert!(
        diags.iter().all(|d| d.severity == Severity::Error),
        "all diagnostics should be errors"
    );
    Ok(())
}

#[test]
fn all_diagnostics_have_valid_spans() -> Result<(), Box<dyn std::error::Error>> {
    let diags = check_fixture("missing_both.py")?;
    assert!(!diags.is_empty(), "fixture should produce diagnostics");
    for diag in &diags {
        assert!(
            diag.span.start <= diag.span.end,
            "span start ({}) must not exceed end ({})",
            diag.span.start,
            diag.span.end
        );
    }
    Ok(())
}

#[test]
fn all_diagnostics_reference_correct_file_path() -> Result<(), Box<dyn std::error::Error>> {
    let path = fixture("missing_both.py");
    let diags = check_fixture("missing_both.py")?;
    for diag in &diags {
        assert_eq!(diag.path, path, "diagnostic path should match fixture path");
    }
    Ok(())
}

#[test]
fn missing_both_broken_has_two_params_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // `broken(x, y)` has 2 unannotated params -> 2 x E0001
    // `also_broken(name)` has 1 unannotated param -> 1 x E0001
    // Both functions lack return annotation -> 2 x E0002
    let diags = check_fixture("missing_both.py")?;
    let e0001_count = diags.iter().filter(|d| d.code.code == "BSK-E0001").count();
    let e0002_count = diags.iter().filter(|d| d.code.code == "BSK-E0002").count();
    assert_eq!(e0001_count, 3, "expected 3 E0001s (x, y, name)");
    assert_eq!(e0002_count, 2, "expected 2 E0002s (broken, also_broken)");
    Ok(())
}
