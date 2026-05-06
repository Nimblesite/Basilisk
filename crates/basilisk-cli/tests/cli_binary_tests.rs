#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
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

use serde_json::Value;

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

fn run_check(paths: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    run_check_with_args(paths, &[])
}

fn run_check_with_args(
    paths: &[&str],
    extra_args: &[&str],
) -> Result<Output, Box<dyn std::error::Error>> {
    let mut cmd = binary();
    let _ = cmd.arg("check");
    for p in paths {
        let _ = cmd.arg(p);
    }
    for a in extra_args {
        let _ = cmd.arg(a);
    }
    Ok(cmd.output()?)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

// ── Shipwright version contract ─────────────────────────────────────────────

#[test]
fn version_plain_matches_shipwright_contract() -> Result<(), Box<dyn std::error::Error>> {
    let out = binary().arg("--version").output()?;
    assert_eq!(out.status.code(), Some(0), "--version must exit 0");
    assert_eq!(
        stdout(&out).trim(),
        concat!("basilisk ", env!("CARGO_PKG_VERSION")),
        "--version must emit '<component-id> <semver>'"
    );
    assert!(
        out.stderr.is_empty(),
        "--version must not emit diagnostics to stderr"
    );
    Ok(())
}

#[test]
fn version_json_matches_shipwright_contract() -> Result<(), Box<dyn std::error::Error>> {
    let out = binary().args(["--version", "--json"]).output()?;
    assert_eq!(out.status.code(), Some(0), "--version --json must exit 0");

    let value: Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(value["manifestVersion"], 1);
    assert_eq!(value["name"], "basilisk");
    assert_eq!(value["binaryName"], "basilisk");
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["kind"], "lsp");
    assert_eq!(value["language"], "rust");
    assert_eq!(value["product"], "basilisk");
    assert!(value.get("buildTime").is_some(), "buildTime must be present");
    assert!(value.get("gitDirty").is_some(), "gitDirty must be present");
    assert!(
        out.stderr.is_empty(),
        "--version --json must not emit diagnostics to stderr"
    );
    Ok(())
}

// ── Exit codes ───────────────────────────────────────────────────────────────

#[test]
fn exit_0_for_clean_file() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("clean/fully_typed_module.py")])?;
    assert_eq!(out.status.code(), Some(0), "clean file must exit 0");
    Ok(())
}

#[test]
fn exit_1_for_file_with_errors() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert_eq!(out.status.code(), Some(1), "file with errors must exit 1");
    Ok(())
}

#[test]
fn exit_3_for_nonexistent_path() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&["/nonexistent/path/does_not_exist.py"])?;
    assert_eq!(out.status.code(), Some(3), "bad path must exit 3");
    Ok(())
}

// ── Clean file output ────────────────────────────────────────────────────────

#[test]
fn clean_file_prints_no_issues_found() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("clean/fully_typed_module.py")])?;
    assert!(
        stdout(&out).contains("No issues found"),
        "clean output must say 'No issues found', got:\n{}",
        stdout(&out)
    );
    Ok(())
}

// ── Error output format ──────────────────────────────────────────────────────

#[test]
fn output_contains_error_code_e0001() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert!(
        stdout(&out).contains("BSK-E0001"),
        "output must contain BSK-E0001, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_error_code_e0002() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0002_single_func.py")])?;
    assert!(
        stdout(&out).contains("BSK-E0002"),
        "output must contain BSK-E0002, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_rustc_style_arrow() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert!(
        stdout(&out).contains("-->"),
        "output must contain --> location marker, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_source_snippet() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    let text = stdout(&out);
    assert!(
        text.contains("def process(data)"),
        "output must contain the source line, got:\n{text}"
    );
    Ok(())
}

#[test]
fn output_contains_caret_underline() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert!(
        stdout(&out).contains('^'),
        "output must contain caret underline, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_help_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert!(
        stdout(&out).contains("= help:"),
        "output must contain help annotation, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_note_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert!(
        stdout(&out).contains("= note:"),
        "output must contain note annotation, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_see_url() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert!(
        stdout(&out).contains("= see: https://"),
        "output must contain see URL, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_line_col_location() -> Result<(), Box<dyn std::error::Error>> {
    // def process(data) -> None:  — `data` is at line 1, col 13
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert!(
        stdout(&out).contains("1:13"),
        "output must contain line:col 1:13, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_diagnostic_summary() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert!(
        stdout(&out).contains("diagnostic"),
        "output must contain summary line, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_shows_correct_error_count() -> Result<(), Box<dyn std::error::Error>> {
    // missing_both.py has 3 x E0001 + 2 x E0002 = 5 errors
    let out = run_check(&[&fixture("missing_both.py")])?;
    assert!(
        stdout(&out).contains("5 error"),
        "output must show 5 errors, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

// ── Multiple files ────────────────────────────────────────────────────────────

#[test]
fn checks_multiple_files_in_one_invocation() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[
        &fixture("errors/e0001_single_param.py"),
        &fixture("errors/e0002_single_func.py"),
    ])?;
    let text = stdout(&out);
    assert!(text.contains("BSK-E0001"), "must flag E0001");
    assert!(text.contains("BSK-E0002"), "must flag E0002");
    assert_eq!(out.status.code(), Some(1));
    Ok(())
}

#[test]
fn clean_and_error_file_together_exits_1() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[
        &fixture("clean/fully_typed_module.py"),
        &fixture("errors/e0001_single_param.py"),
    ])?;
    assert_eq!(out.status.code(), Some(1));
    Ok(())
}

// ── Directory traversal ───────────────────────────────────────────────────────

#[test]
fn traverses_directory_and_finds_errors() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors")])?;
    assert_eq!(
        out.status.code(),
        Some(1),
        "errors/ directory contains broken files, must exit 1"
    );
    Ok(())
}

#[test]
fn traverses_clean_directory_exits_0() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("clean")])?;
    assert_eq!(
        out.status.code(),
        Some(0),
        "clean/ directory has no errors, must exit 0"
    );
    Ok(())
}

// ── Output severity label ─────────────────────────────────────────────────────

#[test]
fn output_severity_label_is_error() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("errors/e0001_single_param.py")])?;
    assert!(
        stdout(&out).contains("error[BSK-"),
        "severity label must be 'error', got:\n{}",
        stdout(&out)
    );
    Ok(())
}

// ── Terminal colours ─────────────────────────────────────────────────────────

/// ANSI escape sequences produced by the `colored` crate.
const BOLD_RED: &str = "\x1b[1;31m";
const BOLD_BLUE: &str = "\x1b[1;34m";
const BOLD_CYAN: &str = "\x1b[1;36m";
const BOLD: &str = "\x1b[1m";

#[test]
fn color_always_emits_ansi_for_error_label() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("errors/e0001_single_param.py")],
        &["--color", "always"],
    )?;
    let text = stdout(&out);
    assert!(
        text.contains(BOLD_RED),
        "--color always must emit bold-red ANSI for error label, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_error_code() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("errors/e0001_single_param.py")],
        &["--color", "always"],
    )?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD}[BSK-E0001]")),
        "--color always must emit bold error code, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_arrow() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("errors/e0001_single_param.py")],
        &["--color", "always"],
    )?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_BLUE}-->")),
        "--color always must emit bold-blue arrow, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_pipe() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("errors/e0001_single_param.py")],
        &["--color", "always"],
    )?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_BLUE}|")),
        "--color always must emit bold-blue pipe, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_caret_underline() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("errors/e0001_single_param.py")],
        &["--color", "always"],
    )?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_RED}^")),
        "--color always must emit bold-red caret underline, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_help_label() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("errors/e0001_single_param.py")],
        &["--color", "always"],
    )?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_CYAN}help")),
        "--color always must emit bold-cyan help label, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_note_label() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("errors/e0001_single_param.py")],
        &["--color", "always"],
    )?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_CYAN}note")),
        "--color always must emit bold-cyan note label, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_see_label() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("errors/e0001_single_param.py")],
        &["--color", "always"],
    )?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_CYAN}see")),
        "--color always must emit bold-cyan see label, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_never_strips_all_ansi() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("errors/e0001_single_param.py")],
        &["--color", "never"],
    )?;
    let text = stdout(&out);
    assert!(
        !text.contains("\x1b["),
        "--color never must not emit any ANSI codes, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_never_clean_file_strips_all_ansi() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("clean/fully_typed_module.py")],
        &["--color", "never"],
    )?;
    let text = stdout(&out);
    assert!(
        !text.contains("\x1b["),
        "--color never must not emit any ANSI codes for clean output, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_clean_file_emits_ansi() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check_with_args(
        &[&fixture("clean/fully_typed_module.py")],
        &["--color", "always"],
    )?;
    let text = stdout(&out);
    assert!(
        text.contains("\x1b["),
        "--color always must emit ANSI codes even for clean output, got:\n{text}"
    );
    Ok(())
}
