//! Enforces reachability for the checker test graph.
//!
//! Cargo discovers Rust files directly under `tests/`, but it does not discover
//! files nested under `tests/checker/` or `tests/golden/`. Every nested test
//! module therefore needs an explicit `#[path = "..."]` registration in a
//! top-level integration-test target. Ignored tests are also excluded from a
//! normal run and are prohibited here.

#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    missing_docs
)]

use std::path::{Path, PathBuf};

fn rust_files(directory: &Path, output: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry.expect("test directory entry must be readable").path();
        if path.is_dir() {
            rust_files(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}

#[test]
fn every_nested_test_module_is_registered_and_no_test_is_ignored() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut top_level_sources = String::new();
    let mut all_rust_files = Vec::new();
    rust_files(&tests, &mut all_rust_files);

    for file in all_rust_files
        .iter()
        .filter(|file| file.parent().is_some_and(|parent| parent == tests))
    {
        let source = std::fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
        top_level_sources.push_str(&source);
        top_level_sources.push('\n');
    }

    let mut unregistered = Vec::new();
    for directory in ["checker", "golden"] {
        for file in all_rust_files.iter().filter(|file| {
            file.parent()
                .is_some_and(|parent| parent == tests.join(directory))
                && file
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with("_tests.rs"))
        }) {
            let relative = file
                .strip_prefix(&tests)
                .expect("nested test must be below tests directory")
                .to_string_lossy()
                .replace('\\', "/");
            let registration = format!("#[path = \"{relative}\"]");
            if !top_level_sources.contains(&registration) {
                unregistered.push(relative);
            }
        }
    }
    unregistered.sort();

    let mut ignored = Vec::new();
    for file in &all_rust_files {
        let source = std::fs::read_to_string(file)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
        if source
            .lines()
            .any(|line| line.trim_start().starts_with("#[ignore"))
        {
            ignored.push(
                file.strip_prefix(&tests)
                    .unwrap_or(file)
                    .display()
                    .to_string(),
            );
        }
    }
    ignored.sort();

    assert!(
        unregistered.is_empty(),
        "nested Rust test modules are unreachable from Cargo integration-test targets: {unregistered:#?}"
    );
    assert!(
        ignored.is_empty(),
        "ignored Rust tests are unreachable during a normal test run: {ignored:#?}"
    );
    assert!(
        all_rust_files.len() > 200,
        "reachability audit unexpectedly saw only {} Rust test files",
        all_rust_files.len()
    );
}
