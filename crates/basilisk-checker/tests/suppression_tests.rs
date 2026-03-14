#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for the suppression system.
#![allow(missing_docs)]

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run(source: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

fn codes(diags: &[basilisk_checker::Diagnostic]) -> Vec<&str> {
    diags.iter().map(|d| d.code.code).collect()
}

#[test]
fn type_ignore_suppresses_all() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def process(data) -> None:  # type: ignore\n    pass\n";
    let diags = run(source)?;
    // E0001 should be suppressed by type: ignore
    assert!(
        !codes(&diags).contains(&"BSK-E0001"),
        "type: ignore should suppress E0001"
    );
    Ok(())
}

#[test]
fn type_ignore_with_code_suppresses_specific() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def process(data) -> None:  # type: ignore[BSK-E0001]\n    pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0001"),
        "type: ignore[BSK-E0001] should suppress E0001"
    );
    Ok(())
}

#[test]
fn type_warning_demotes_to_warning() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def process(data) -> None:  # type: warning[BSK-E0001]\n    pass\n";
    let diags = run(source)?;
    let e0001_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            basilisk_checker::Severity::Warning,
            "type: warning should demote to warning"
        );
    }
    Ok(())
}

#[test]
fn basilisk_relaxed_demotes_errors() -> Result<(), Box<dyn std::error::Error>> {
    let source = "# basilisk: relaxed\ndef process(data) -> None:\n    pass\n";
    let diags = run(source)?;
    for diag in &diags {
        assert_ne!(
            diag.severity,
            basilisk_checker::Severity::Error,
            "basilisk: relaxed should demote all errors, but {} is still Error",
            diag.code.code
        );
    }
    Ok(())
}

#[test]
fn file_disabled_suppresses_specific_rule() -> Result<(), Box<dyn std::error::Error>> {
    let source = "# basilisk: file-disabled[BSK-E0001]\ndef process(data) -> None:\n    pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0001"),
        "file-disabled should suppress specific rule"
    );
    Ok(())
}

#[test]
fn type_info_demotes_to_info() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def process(data) -> None:  # type: info[BSK-E0001]\n    pass\n";
    let diags = run(source)?;
    let e0001_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    for diag in &e0001_diags {
        assert_eq!(
            diag.severity,
            basilisk_checker::Severity::Info,
            "type: info should demote to info"
        );
    }
    Ok(())
}

#[test]
fn block_disabled_suppresses_range() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"# type: disabled[BSK-E0010]
import numpy
import pandas
# type: end-disabled[BSK-E0010]
import os
";
    let diags = run(source)?;
    // E0010 should be suppressed for numpy and pandas but not os (os is stdlib anyway)
    let e0010_count = diags.iter().filter(|d| d.code.code == "BSK-E0010").count();
    assert_eq!(
        e0010_count, 0,
        "block disabled should suppress E0010 for imports within the block"
    );
    Ok(())
}
