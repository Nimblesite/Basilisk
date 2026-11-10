//! Rule liveness census for [CHKARCH-DIAG-CATEGORIES].
//!
//! Answers ONE empirical question per registered rule: over a corpus of
//! Python this checker was never fitted to, does the rule ever emit a
//! diagnostic at all?
//!
//! Liveness is NOT correctness. A rule that fires proves only that its code
//! path is reachable; whether the verdict implements its PEP obligation is
//! decided by the permutation oracle in `tests/golden/` ([PERMTEST-PLAN]).
//! A rule that never fires anywhere, however, cannot be implementing
//! anything — that is a definite negative, and it is what this census is
//! for.
//!
//! This census is part of the normal test graph. A test hidden behind
//! `#[ignore]` is not evidence and is prohibited by the reachability guard.
//!
//! ```text
//! cargo test -p basilisk-checker --test rule_liveness_census -- --nocapture
//! ```
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs,
    dead_code
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod common;

/// Corpora walked by the census, relative to the workspace root.
///
/// The real-world entries are third-party projects vendored for the editor
/// extension's manual testing — code with no relationship to any fixture in
/// this repository, which is exactly what makes them evidence.
const CORPORA: &[&str] = &[
    "vscode-extension/.real-world",
    "conformance/tests",
    "crates/basilisk-cli/tests/fixtures/errors",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the crate")
        .to_path_buf()
}

/// Every `.py` file under `dir`, recursively.
fn python_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            python_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "py") {
            out.push(path);
        }
    }
}

#[test]
fn census_which_rules_ever_fire() {
    let root = workspace_root();
    let mut files = Vec::new();
    for corpus in CORPORA {
        python_files(&root.join(corpus), &mut files);
    }
    assert!(
        !files.is_empty(),
        "no corpus found under {}",
        root.display()
    );

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut checked = 0usize;
    for file in &files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(diagnostics) = common::run(&source) else {
            continue;
        };
        checked += 1;
        for diagnostic in diagnostics {
            *counts.entry(diagnostic.code.code.to_owned()).or_insert(0) += 1;
        }
    }

    let report = counts
        .iter()
        .map(|(code, count)| format!("{code}\t{count}"))
        .collect::<Vec<_>>()
        .join("\n");
    let out = std::env::var("BASILISK_CENSUS_OUT")
        .unwrap_or_else(|_| "/tmp/basilisk_rule_liveness.tsv".to_owned());
    std::fs::write(&out, format!("{report}\n")).expect("census report must write");

    println!("files checked: {checked} of {}", files.len());
    println!("distinct rule codes that fired: {}", counts.len());
    println!("report written to {out}");
}
