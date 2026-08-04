//! Golden gate for the type-torture corpus ([NARROWPLAN-SUPERIORITY] slice).
//! See docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md and
//! `benchmarks/torture/run_torture.py` (the cross-checker scoreboard over the
//! same cases).
//!
//! Each case in `benchmarks/torture/cases/*.py` is scored conformance-style:
//! every line ending in `# E` must draw at least one error-severity
//! diagnostic, and no other line may draw any. The checker runs in-process
//! with the default configuration — exactly `common::run` — so `cargo test`
//! (and therefore CI) breaks the moment a torture case regresses.
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs,
    dead_code
)]

#[path = "common/mod.rs"]
mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use basilisk_checker::Severity;
use common::run;

/// Absolute path of a torture case file.
fn case_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/torture/cases")
        .join(name)
}

/// 1-based lines carrying a `# E` marker — the required-error lines.
fn expected_error_lines(source: &str) -> BTreeSet<usize> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim_end().ends_with("# E"))
        .map(|(index, _)| index + 1)
        .collect()
}

/// 1-based line number of a byte offset.
fn line_of_offset(source: &str, offset: usize) -> usize {
    source.get(..offset).map_or(1, |prefix| {
        prefix.bytes().filter(|b| *b == b'\n').count() + 1
    })
}

/// Run one case and assert the reported error lines equal the `# E` lines.
fn assert_case_golden(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(case_path(name))?;
    let expected = expected_error_lines(&source);

    let diags = run(&source)?;
    let reported: BTreeSet<usize> = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| line_of_offset(&source, usize::try_from(d.span.start).unwrap_or(0)))
        .collect();

    let missed: Vec<usize> = expected.difference(&reported).copied().collect();
    let extra: Vec<usize> = reported.difference(&expected).copied().collect();
    assert!(
        missed.is_empty() && extra.is_empty(),
        "{name}: golden mismatch — missed required-error lines {missed:?}, \
         false-positive lines {extra:?}; diagnostics: {:?}",
        diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| {
                format!(
                    "L{} {}: {}",
                    line_of_offset(&source, usize::try_from(d.span.start).unwrap_or(0)),
                    d.code.code,
                    d.message
                )
            })
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn enum_literal_expansion() -> Result<(), Box<dyn std::error::Error>> {
    assert_case_golden("enum_literal_expansion.py")
}

#[test]
fn generic_constructor() -> Result<(), Box<dyn std::error::Error>> {
    assert_case_golden("generic_constructor.py")
}

#[test]
fn param_inference() -> Result<(), Box<dyn std::error::Error>> {
    assert_case_golden("param_inference.py")
}

#[test]
fn paramspec_decorator() -> Result<(), Box<dyn std::error::Error>> {
    assert_case_golden("paramspec_decorator.py")
}

#[test]
fn recursive_aliases() -> Result<(), Box<dyn std::error::Error>> {
    assert_case_golden("recursive_aliases.py")
}

#[test]
fn recursive_bases() -> Result<(), Box<dyn std::error::Error>> {
    assert_case_golden("recursive_bases.py")
}

#[test]
fn tuple_index() -> Result<(), Box<dyn std::error::Error>> {
    assert_case_golden("tuple_index.py")
}

#[test]
fn typeis_narrowing() -> Result<(), Box<dyn std::error::Error>> {
    assert_case_golden("typeis_narrowing.py")
}
