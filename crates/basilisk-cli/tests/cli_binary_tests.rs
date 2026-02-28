//! Subprocess tests for the `basilisk` binary.
//!
//! These are the only tests that exercise `main.rs` and `output.rs` — code
//! that is unreachable from library-level integration tests because it lives
//! inside a binary crate.  Every test spawns the compiled binary, captures
//! stdout/stderr, and asserts on exit code and output content.
//!
//! Exit code contract:
//!   0 — clean, no errors
//!   1 — type errors found
//!   3 — internal error (bad path, I/O failure)

use std::path::Path;
use std::process::{Command, Output};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
}

fn fixture(rel: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

fn run_check(paths: &[&str]) -> Output {
    let mut cmd = binary();
    cmd.arg("check");
    for p in paths {
        cmd.arg(p);
    }
    cmd.output().expect("failed to spawn basilisk binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

// ── Exit codes ───────────────────────────────────────────────────────────────

#[test]
fn exit_0_for_clean_file() {
    let out = run_check(&[&fixture("clean/fully_typed_module.py")]);
    assert_eq!(out.status.code(), Some(0), "clean file must exit 0");
}

#[test]
fn exit_1_for_file_with_errors() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert_eq!(out.status.code(), Some(1), "file with errors must exit 1");
}

#[test]
fn exit_3_for_nonexistent_path() {
    let out = run_check(&["/nonexistent/path/does_not_exist.py"]);
    assert_eq!(out.status.code(), Some(3), "bad path must exit 3");
}

// ── Clean file output ────────────────────────────────────────────────────────

#[test]
fn clean_file_prints_no_issues_found() {
    let out = run_check(&[&fixture("clean/fully_typed_module.py")]);
    assert!(
        stdout(&out).contains("No issues found"),
        "clean output must say 'No issues found', got:\n{}",
        stdout(&out)
    );
}

// ── Error output format ──────────────────────────────────────────────────────

#[test]
fn output_contains_error_code_e0001() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert!(
        stdout(&out).contains("BSK-E0001"),
        "output must contain BSK-E0001, got:\n{}",
        stdout(&out)
    );
}

#[test]
fn output_contains_error_code_e0002() {
    let out = run_check(&[&fixture("errors/e0002_single_func.py")]);
    assert!(
        stdout(&out).contains("BSK-E0002"),
        "output must contain BSK-E0002, got:\n{}",
        stdout(&out)
    );
}

#[test]
fn output_contains_rustc_style_arrow() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert!(
        stdout(&out).contains("-->"),
        "output must contain --> location marker, got:\n{}",
        stdout(&out)
    );
}

#[test]
fn output_contains_source_snippet() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    let text = stdout(&out);
    assert!(
        text.contains("def process(data)"),
        "output must contain the source line, got:\n{text}"
    );
}

#[test]
fn output_contains_caret_underline() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert!(
        stdout(&out).contains('^'),
        "output must contain caret underline, got:\n{}",
        stdout(&out)
    );
}

#[test]
fn output_contains_help_annotation() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert!(
        stdout(&out).contains("= help:"),
        "output must contain help annotation, got:\n{}",
        stdout(&out)
    );
}

#[test]
fn output_contains_note_annotation() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert!(
        stdout(&out).contains("= note:"),
        "output must contain note annotation, got:\n{}",
        stdout(&out)
    );
}

#[test]
fn output_contains_see_url() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert!(
        stdout(&out).contains("= see: https://"),
        "output must contain see URL, got:\n{}",
        stdout(&out)
    );
}

#[test]
fn output_contains_line_col_location() {
    // def process(data) -> None:  — `data` is at line 1, col 13
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert!(
        stdout(&out).contains("1:13"),
        "output must contain line:col 1:13, got:\n{}",
        stdout(&out)
    );
}

#[test]
fn output_contains_diagnostic_summary() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert!(
        stdout(&out).contains("diagnostic"),
        "output must contain summary line, got:\n{}",
        stdout(&out)
    );
}

#[test]
fn output_shows_correct_error_count() {
    // missing_both.py has 3 x E0001 + 2 x E0002 = 5 errors
    let out = run_check(&[&fixture("missing_both.py")]);
    assert!(
        stdout(&out).contains("5 error"),
        "output must show 5 errors, got:\n{}",
        stdout(&out)
    );
}

// ── Multiple files ────────────────────────────────────────────────────────────

#[test]
fn checks_multiple_files_in_one_invocation() {
    let out = run_check(&[
        &fixture("errors/e0001_single_param.py"),
        &fixture("errors/e0002_single_func.py"),
    ]);
    let text = stdout(&out);
    assert!(text.contains("BSK-E0001"), "must flag E0001");
    assert!(text.contains("BSK-E0002"), "must flag E0002");
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn clean_and_error_file_together_exits_1() {
    let out = run_check(&[
        &fixture("clean/fully_typed_module.py"),
        &fixture("errors/e0001_single_param.py"),
    ]);
    assert_eq!(out.status.code(), Some(1));
}

// ── Directory traversal ───────────────────────────────────────────────────────

#[test]
fn traverses_directory_and_finds_errors() {
    let out = run_check(&[&fixture("errors")]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "errors/ directory contains broken files, must exit 1"
    );
}

#[test]
fn traverses_clean_directory_exits_0() {
    let out = run_check(&[&fixture("clean")]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "clean/ directory has no errors, must exit 0"
    );
}

// ── Output severity label ─────────────────────────────────────────────────────

#[test]
fn output_severity_label_is_error() {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")]);
    assert!(
        stdout(&out).contains("error[BSK-"),
        "severity label must be 'error', got:\n{}",
        stdout(&out)
    );
}
