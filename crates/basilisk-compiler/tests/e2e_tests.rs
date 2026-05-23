//! Tests for [COMPARCH]. See docs/specs/COMPILER-ARCHITECTURE-SPEC.md#COMPARCH
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! E2E compiler tests.
//!
//! Iterates every `.py` file in `tests/e2e/`, compiles and runs it,
//! then asserts stdout matches the corresponding `-expectedoutput.txt`.

use std::path::Path;

fn run_e2e_test(py_path: &Path) {
    let stem = py_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let dir = py_path.parent().unwrap_or(Path::new("."));

    let expected_path = dir.join(format!("{stem}-expectedoutput.txt"));
    let error_path = dir.join(format!("{stem}-expectederror.txt"));

    let source = std::fs::read_to_string(py_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", py_path.display()));

    let path_str = py_path.to_string_lossy();

    match basilisk_compiler::compile_and_run(&source, &path_str) {
        Ok(result) => {
            // If we expected an error, check diagnostics contain the code
            if error_path.exists() {
                let expected_error = std::fs::read_to_string(&error_path)
                    .unwrap_or_else(|e| panic!("failed to read {}: {e}", error_path.display()));
                let expected_code = expected_error.trim();

                assert!(
                    !result.diagnostics.is_empty(),
                    "{stem}: expected compilation error {expected_code} but got success"
                );

                let codes: Vec<String> = result
                    .diagnostics
                    .iter()
                    .map(|d| d.code.code.to_owned())
                    .collect();

                assert!(
                    codes.iter().any(|c| c.contains(expected_code)),
                    "{stem}: expected error {expected_code}, got: {codes:?}"
                );
                return;
            }

            // Print the actual program output
            eprintln!(
                "--- {stem} stdout ---\n{}\n--- end ---",
                result.stdout.trim_end()
            );

            // Otherwise we expected success — check stdout
            assert!(
                result.diagnostics.is_empty(),
                "{stem}: type errors:\n{}",
                result
                    .diagnostics
                    .iter()
                    .map(|d| format!("  {}: {}", d.code.code, d.message))
                    .collect::<Vec<_>>()
                    .join("\n")
            );

            assert!(
                expected_path.exists(),
                "{stem}: missing expected output file: {}",
                expected_path.display()
            );

            let expected = std::fs::read_to_string(&expected_path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", expected_path.display()));

            assert_eq!(
                result.stdout.trim_end(),
                expected.trim_end(),
                "{stem}: output mismatch"
            );
        }
        Err(err) => {
            // If we expected an error file, that's fine for parse/resolve errors too
            if error_path.exists() {
                return;
            }
            panic!("{stem}: unexpected compiler error: {err}");
        }
    }
}

#[test]
fn e2e_all_examples() {
    let e2e_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e");

    let mut py_files: Vec<_> = std::fs::read_dir(&e2e_dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", e2e_dir.display()))
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "py"))
        .map(|entry| entry.path())
        .collect();

    py_files.sort();

    assert!(
        !py_files.is_empty(),
        "no .py files found in {}",
        e2e_dir.display()
    );

    // Optional filter: BASILISK_COMPILER_FILTER=hello,arithmetic
    // When set, only run tests whose stem matches one of the comma-separated names.
    let filter: Option<Vec<String>> = std::env::var("BASILISK_COMPILER_FILTER")
        .ok()
        .map(|val| val.split(',').map(|s| s.trim().to_string()).collect());

    let mut failures: Vec<String> = Vec::new();

    for py_path in &py_files {
        let stem = py_path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");

        if let Some(ref allowed) = filter {
            if !allowed.iter().any(|a| a == stem) {
                continue;
            }
        }
        eprintln!("running e2e test: {stem}");
        let result = std::panic::catch_unwind(|| run_e2e_test(py_path));
        match result {
            Ok(()) => eprintln!("  PASS: {stem}"),
            Err(err) => {
                let msg = err
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| err.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                eprintln!("  FAIL: {stem}: {msg}");
                failures.push(format!("{stem}: {msg}"));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "\n{} of {} e2e tests FAILED:\n  {}",
        failures.len(),
        py_files.len(),
        failures.join("\n  ")
    );
}
