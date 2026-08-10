//! The inert-CLI contract, exercised on the real binary.
//!
//! Implements [WITHDRAWAL-INERT]. See
//! docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-INERT
//!
//! These assertions are the whole product surface now: whatever a user or a CI
//! pipeline types, Basilisk must print the approved statement and fail. Unit
//! tests inside `main.rs` cannot prove that — the exit status, the emptiness of
//! stdout, and the fact that no file is touched are properties of the process.

#![expect(
    clippy::expect_used,
    reason = "a test that cannot spawn the binary under test has nothing to assert"
)]

use std::path::Path;
use std::process::{Command, Output};

/// The bytes the binary must print, from the same generated file it compiles in.
const NOTICE: &str = include_str!("../src/withdrawal_notice.txt");

/// `4` — unlisted ([CHKARCH-CLI-EXITCODES]).
const EXIT_UNLISTED: i32 = 4;

/// Every argument shape a user or a stale pipeline could still send: the old
/// subcommands, the flags clap used to own, and nothing at all.
const INVOCATIONS: &[&[&str]] = &[
    &[],
    &["check"],
    &["check", "."],
    &["check", "app.py", "--output", "json"],
    &["analyze", "src/"],
    &["format", "."],
    &["format", "--check"],
    &["fix", ".", "--unsafe"],
    &["adopt"],
    &["unadopt"],
    &["lsp"],
    &["lsp", "--transport", "ws", "--port", "8765"],
    &["mcp"],
    &["typeshed", "download"],
    &["stubs", "status"],
    &["createstub", "widget"],
    &["--help"],
    &["-h"],
    &["help"],
    &["--not-a-real-flag"],
    &["--output", "json"],
];

fn run(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("the basilisk binary must be runnable")
}

/// A project directory holding one file Basilisk would once have rewritten.
fn project() -> Result<tempfile::TempDir, std::io::Error> {
    let dir = tempfile::tempdir()?;
    std::fs::write(
        dir.path().join("pyproject.toml"),
        b"[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n",
    )?;
    std::fs::write(
        dir.path().join("app.py"),
        b"def f(x)  ->None :\n  return x\n",
    )?;
    Ok(dir)
}

/// Every invocation prints the approved notice to stderr, byte for byte.
#[test]
fn every_invocation_prints_the_notice_to_stderr() -> Result<(), Box<dyn std::error::Error>> {
    let dir = project()?;
    for args in INVOCATIONS {
        let output = run(args, dir.path());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            NOTICE,
            "stderr must be the approved notice for `basilisk {}`",
            args.join(" ")
        );
    }
    Ok(())
}

/// Stdout stays empty, always — `--output json > report.json` must yield an
/// empty file, never prose a consumer could parse as findings.
#[test]
fn every_invocation_writes_nothing_to_stdout() -> Result<(), Box<dyn std::error::Error>> {
    let dir = project()?;
    for args in INVOCATIONS {
        let output = run(args, dir.path());
        assert!(
            output.stdout.is_empty(),
            "stdout must stay empty for `basilisk {}`, got {:?}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout)
        );
    }
    Ok(())
}

/// Exit `4`. Never `0` (a pipeline must break), never `1` ("errors found"
/// would be one more incorrect result), never `2`/`3`.
#[test]
fn every_invocation_exits_four() -> Result<(), Box<dyn std::error::Error>> {
    let dir = project()?;
    for args in INVOCATIONS {
        let output = run(args, dir.path());
        assert_eq!(
            output.status.code(),
            Some(EXIT_UNLISTED),
            "`basilisk {}` must exit {EXIT_UNLISTED}",
            args.join(" ")
        );
    }
    Ok(())
}

/// No file is created, deleted, or rewritten — `fix`, `format` and `adopt`
/// used to edit source in place, and an inert binary must not.
#[test]
fn no_invocation_touches_the_workspace() -> Result<(), Box<dyn std::error::Error>> {
    let dir = project()?;
    let source = std::fs::read(dir.path().join("app.py"))?;
    let before = listing(dir.path())?;
    for args in INVOCATIONS {
        let _ = run(args, dir.path());
    }
    assert_eq!(
        std::fs::read(dir.path().join("app.py"))?,
        source,
        "source must be untouched"
    );
    assert_eq!(
        listing(dir.path())?,
        before,
        "no file may be added or removed"
    );
    Ok(())
}

/// The directory's entries, sorted — a cache dir or a rewritten config would
/// show up here.
fn listing(dir: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(names)
}

/// `--version` is the one surface that still answers, and it answers on stdout
/// with exit 0: package managers and installed extensions probe it, and a
/// failure there hides the notice behind a broken install instead of showing it.
#[test]
fn version_still_answers() -> Result<(), Box<dyn std::error::Error>> {
    let dir = project()?;
    for args in [&["--version"][..], &["--version", "--json"][..]] {
        let output = run(args, dir.path());
        assert_eq!(output.status.code(), Some(0), "`{args:?}` must exit 0");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("basilisk"),
            "`{args:?}` must name the product on stdout"
        );
    }
    Ok(())
}

/// The version contract claims no capabilities. Advertising `lsp`/`mcp`/`dap`
/// to a tool that reads the contract would be a false claim about a binary
/// that starts no server.
#[test]
fn version_json_claims_no_capabilities() -> Result<(), Box<dyn std::error::Error>> {
    let dir = project()?;
    let output = run(&["--version", "--json"], dir.path());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for capability in ["\"lsp\"", "\"mcp\"", "\"dap\"", "\"profiler\""] {
        assert!(
            !stdout.contains(capability),
            "the inert binary must not advertise {capability}: {stdout}"
        );
    }
    Ok(())
}
