//! Tests for [LSPFMT-CLIENTS] / [CHKARCH-CLI-COMMANDS]. See
//! docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-CLIENTS
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! Real-binary tests for `basilisk format`: write and `--check` behaviour,
//! multiple paths, parse failures, `[tool.ruff]` style configuration,
//! formatter disablement, and byte-parity with the LSP formatting path.
//!
//! Every test spawns the compiled binary with `PATH` pointing at an empty
//! directory, so no external `ruff` (or anything else) is findable — the
//! embedded engine must do all the work ([LSPFMT-DECISION]).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

static DIR_COUNTER: AtomicU32 = AtomicU32::new(0);

/// A unique, empty project directory for one test.
fn project_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "basilisk_fmt_{tag}_{}_{}",
        std::process::id(),
        DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("create project dir");
    dir
}

fn write(dir: &Path, rel: &str, content: &str) -> PathBuf {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(&path, content).expect("write fixture");
    path
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read fixture back")
}

/// Run `basilisk format <args>` inside `dir` with an empty `PATH`.
fn run_format(dir: &Path, args: &[&str]) -> Output {
    let empty_path = dir.join(".empty-path");
    std::fs::create_dir_all(&empty_path).expect("create empty PATH dir");
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .current_dir(dir)
        .env("PATH", &empty_path)
        .arg("format")
        .args(args)
        .output()
        .expect("spawn basilisk format")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn exit_code(output: &Output) -> i32 {
    output.status.code().expect("exit code")
}

// ── Write mode ───────────────────────────────────────────────────────────────

/// Write mode rewrites the file to exactly the embedded Ruff output. The
/// input is the same source the LSP no-ruff test formats, so the expected
/// bytes pin CLI/LSP parity ([LSPFMT-CLIENTS] acceptance).
#[test]
fn write_mode_produces_ruff_output_with_no_ruff_on_path() {
    let dir = project_dir("write");
    let file = write(&dir, "app.py", "x=1\ny  =   'two'\n");

    let output = run_format(&dir, &["."]);

    assert_eq!(exit_code(&output), 0, "write mode must exit 0: {output:?}");
    assert_eq!(
        read(&file),
        "x = 1\ny = \"two\"\n",
        "output must be byte-identical to the embedded Ruff formatter"
    );
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("Reformatted 1 file"),
        "summary must count the rewrite: {stdout}"
    );
    // [LSPFMT-PROVENANCE]: the CLI names the engine that produced the bytes.
    assert!(
        stdout.contains("embedded Ruff"),
        "summary must attribute the embedded engine: {stdout}"
    );
}

/// An already-formatted file is left byte-identical and reported unchanged.
#[test]
fn write_mode_leaves_formatted_file_untouched() {
    let dir = project_dir("clean");
    let file = write(&dir, "app.py", "x = 1\n");

    let output = run_format(&dir, &["."]);

    assert_eq!(exit_code(&output), 0);
    assert_eq!(read(&file), "x = 1\n", "clean file must not be rewritten");
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("Reformatted 0 files") && stdout.contains("1 already formatted"),
        "summary must report the file as already formatted: {stdout}"
    );
}

// ── Check mode ───────────────────────────────────────────────────────────────

/// `--check` reports the file and exits 1 without writing anything.
#[test]
fn check_mode_reports_without_writing() {
    let dir = project_dir("check");
    let file = write(&dir, "app.py", "x=1\n");

    let output = run_format(&dir, &["--check", "."]);

    assert_eq!(exit_code(&output), 1, "--check must exit 1 on a diff");
    assert_eq!(read(&file), "x=1\n", "--check must never write");
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("Would reformat") && stdout.contains("app.py"),
        "--check must name the unformatted file: {stdout}"
    );
}

/// `--check` on a formatted tree exits 0.
#[test]
fn check_mode_clean_tree_exits_zero() {
    let dir = project_dir("check_clean");
    let _ = write(&dir, "app.py", "x = 1\n");

    let output = run_format(&dir, &["--check", "."]);

    assert_eq!(exit_code(&output), 0, "clean --check must exit 0");
    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("0 files would be reformatted"),
        "clean --check summary: {stdout}"
    );
}

// ── Multiple paths ───────────────────────────────────────────────────────────

/// A directory argument recurses and an explicit file argument is taken
/// verbatim; both format in one run.
#[test]
fn multiple_paths_mix_directories_and_files() {
    let dir = project_dir("multi");
    let one = write(&dir, "pkg/one.py", "a=1\n");
    let two = write(&dir, "two.py", "b =  2\n");

    let output = run_format(&dir, &["pkg", "two.py"]);

    assert_eq!(exit_code(&output), 0);
    assert_eq!(read(&one), "a = 1\n");
    assert_eq!(read(&two), "b = 2\n");
    assert!(
        stdout_of(&output).contains("Reformatted 2 files"),
        "both paths must be formatted: {}",
        stdout_of(&output)
    );
}

// ── Parse failures ───────────────────────────────────────────────────────────

/// Invalid syntax is refused (never rewritten), the rest of the run still
/// formats, and the exit code is 1.
#[test]
fn parse_failure_exits_one_and_never_rewrites_the_broken_file() {
    let dir = project_dir("parse_fail");
    let broken = write(&dir, "broken.py", "def f(:\n");
    let good = write(&dir, "good.py", "x=1\n");

    let output = run_format(&dir, &["."]);

    assert_eq!(exit_code(&output), 1, "parse failure must exit 1");
    assert_eq!(read(&broken), "def f(:\n", "broken file must be untouched");
    assert_eq!(read(&good), "x = 1\n", "healthy files must still format");
    assert!(
        stdout_of(&output).contains("failed to parse"),
        "summary must surface the parse failure: {}",
        stdout_of(&output)
    );
}

// ── Style configuration ──────────────────────────────────────────────────────

/// `[tool.ruff]` / `[tool.ruff.format]` style options are honoured, exactly
/// as the LSP path reads them ([LSPFMT-ENGINE] config-respecting).
#[test]
fn style_options_from_pyproject_are_honoured() {
    let dir = project_dir("style");
    let _ = write(
        &dir,
        "pyproject.toml",
        "[tool.ruff]\nline-length = 100\n\n[tool.ruff.format]\nquote-style = \"single\"\n",
    );
    let file = write(&dir, "app.py", "x=\"s\"\n");

    let output = run_format(&dir, &["."]);

    assert_eq!(exit_code(&output), 0);
    assert_eq!(
        read(&file),
        "x = 's'\n",
        "quote-style = single must produce single quotes"
    );
}

// ── Formatter disablement ────────────────────────────────────────────────────

/// `formatter = "none"` disables the CLI exactly as it stops the LSP
/// advertising formatting capabilities ([LSPFMT-CONFIG]).
#[test]
fn disabled_formatter_is_a_no_op() {
    let dir = project_dir("disabled");
    let _ = write(&dir, "pyrightconfig.json", "{\"formatter\": \"none\"}");
    let file = write(&dir, "app.py", "x=1\n");

    let output = run_format(&dir, &["."]);

    assert_eq!(exit_code(&output), 0, "disabled formatter must exit 0");
    assert_eq!(read(&file), "x=1\n", "disabled formatter must not write");
    assert!(
        stdout_of(&output).contains("disabled"),
        "the no-op must be explicit, never silent: {}",
        stdout_of(&output)
    );
}

// ── Exclude semantics ────────────────────────────────────────────────────────

/// `[tool.basilisk] exclude` is honoured by the same shared matcher as
/// `check` and `fix` ([CHKARCH-CONFIG-EXCLUDE]).
#[test]
fn excluded_directories_are_skipped() {
    let dir = project_dir("exclude");
    let _ = write(
        &dir,
        "pyproject.toml",
        "[tool.basilisk]\nexclude = [\"vendor\"]\n",
    );
    let vendored = write(&dir, "vendor/gen.py", "x=1\n");
    let app = write(&dir, "app.py", "y=2\n");

    let output = run_format(&dir, &["."]);

    assert_eq!(exit_code(&output), 0);
    assert_eq!(read(&vendored), "x=1\n", "excluded file must be untouched");
    assert_eq!(read(&app), "y = 2\n", "included file must format");
}
