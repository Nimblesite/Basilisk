//! Tests for [CHKARCH-CONFORMANCE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md
#![allow(clippy::allow_attributes, clippy::expect_used, clippy::panic)]
//! PEP conformance gate — thin wrapper around the OFFICIAL Python scorer.
//!
//! The conformance score is **not** computed in Rust. It is computed by
//! `conformance/score.py`, which **imports the committed, sha256-verified
//! `conformance/upstream_main.py`** (a byte-identical copy of the
//! `python/typing` conformance tool `conformance/src/main.py`, pinned to the
//! same commit the fixtures come from) and **runs its own `get_expected_errors`
//! + `diff_expected_errors` functions unmodified**. That guarantees Basilisk is
//! graded by the exact same algorithm as pyright, mypy, pyrefly, ty, zuban and
//! pycroscope — no Basilisk-specific scoring, no excluded diagnostic codes, and
//! nothing fetched from the network at score time.
//!
//! This test exists only so the gate runs inside `make test`: it builds the
//! real `basilisk` binary (via `CARGO_BIN_EXE_basilisk`), invokes the scorer
//! with `--gate`, and fails if the scorer exits non-zero (score below the
//! ratchet threshold or false positives above the ceiling in
//! `coverage-thresholds.json`).
//!
//! On a fresh checkout the conformance fixtures are not present (they are
//! git-ignored and fetched on demand by `make conformance`); in that case the
//! scorer prints a skip notice and exits 0, so this test passes without them.

use std::{path::PathBuf, process::Command};

/// Walk up from the crate manifest to the workspace root.
fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !(dir.join("Cargo.toml").exists() && dir.join("crates").exists()) {
        assert!(
            dir.pop(),
            "could not locate workspace root from CARGO_MANIFEST_DIR"
        );
    }
    dir
}

/// First Python interpreter that responds to `--version`.
fn python() -> Option<&'static str> {
    ["python3", "python"].into_iter().find(|exe| {
        Command::new(exe)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

#[test]
fn conformance_score() {
    let root = repo_root();
    let conformance_dir = root.join("crates/basilisk-cli/tests/conformance");

    // Fresh checkout without fixtures: the scorer itself skips, but short-circuit
    // here too so we don't require Python just to no-op.
    if !conformance_dir.exists() {
        println!("  ⚠  Conformance suite not downloaded — skipping. Run: make conformance");
        return;
    }

    let score_py = root.join("conformance/score.py");
    assert!(
        score_py.exists(),
        "conformance/score.py is missing — the official scorer must be present"
    );

    let py = python().expect(
        "python3 is required to run the official conformance scorer \
         (conformance/score.py). Install Python 3.12+.",
    );

    // `CARGO_BIN_EXE_basilisk` is injected by cargo for integration tests and
    // points at the freshly built binary — the exact artifact users run.
    let binary = env!("CARGO_BIN_EXE_basilisk");

    let status = Command::new(py)
        .arg(&score_py)
        .arg("--bin")
        .arg(binary)
        .arg("--gate")
        .status()
        .expect("failed to spawn the official conformance scorer");

    assert!(
        status.success(),
        "PEP conformance gate failed — see scorer output above. The score is \
         computed by the verbatim python/typing algorithm in conformance/score.py."
    );
}
