//! Enforces [CHKARCH-TESTING]: a test that observes diagnostics and asserts
//! nothing is not a test.
//!
//! `let _ = run(source)?;` and `let _ = codes(&diags);` prove that a function
//! returned. They pass whether the checker is correct, silently wrong, or
//! emitting nothing at all — so they report "ok" while proving no PEP
//! obligation whatsoever. A suite full of them reads as coverage and is not:
//! it is the same dishonesty as a text-matched verdict, one level up.
//!
//! This test FAILS while that debt exists, and names it. The only way to
//! satisfy it is to give each test a specific positive or negative diagnostic
//! assertion derived from the relevant typing specification — never to delete
//! the test, and never to relax this check.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use std::path::{Path, PathBuf};

/// Patterns that consume a checker result without asserting anything about it.
const NO_ASSERTION_PATTERNS: &[&str] = &[
    "let _ = run(",
    "let _ = codes(",
    "let _ = codes_owned(",
    "let _ = diags",
    "let _ = diagnostics",
    "let _ = messages_for(",
];

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_test_observes_diagnostics_without_asserting() {
    let mut files = Vec::new();
    rust_files(&tests_dir(), &mut files);
    assert!(!files.is_empty(), "no test sources found");

    let mut offenders: Vec<(String, usize)> = Vec::new();
    let mut total = 0usize;
    for file in &files {
        // This file necessarily contains the patterns as string literals.
        if file.file_name().is_some_and(|name| name == "no_assertion_debt_tests.rs") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let count: usize = NO_ASSERTION_PATTERNS
            .iter()
            .map(|pattern| source.matches(pattern).count())
            .sum();
        if count > 0 {
            total += count;
            let name = file
                .strip_prefix(tests_dir())
                .unwrap_or(file)
                .display()
                .to_string();
            offenders.push((name, count));
        }
    }

    offenders.sort_by(|left, right| right.1.cmp(&left.1));
    let worst: Vec<String> = offenders
        .iter()
        .take(15)
        .map(|(name, count)| format!("  {count:>5}  {name}"))
        .collect();

    assert_eq!(
        total,
        0,
        "\n{total} no-assertion test bodies across {} files observe checker output and \
         assert nothing. Each reports `ok` regardless of whether the rule is correct, \
         silently wrong, or entirely inert — so none of them can catch a wrong result.\n\n\
         Worst offenders (count, file):\n{}\n\n\
         Fix by giving each a specific diagnostic assertion derived from the typing \
         specification (`assert_rule_count`, or a golden obligation attributed with \
         `assert_rejected_by`). Never by deleting the test or weakening this check.\n",
        offenders.len(),
        worst.join("\n")
    );
}
