//! Tests for [CHKARCH-CLI] / [CHKARCH-CLI-COMMANDS] / [CHKARCH-COMMANDS].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
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
//! `check` and `analyze` share the pipeline and output machinery and differ
//! only in scope ([CHKARCH-COMMANDS]): house-rule (`BSK-…`) diagnostics only
//! ever surface through `analyze`, so the staged-project tests below drive
//! `analyze`; pep diagnostics drive `check`.
//!
//! Exit code contract ([CHKARCH-CLI-EXITCODES]):
//!   0 — clean, no errors
//!   1 — error diagnostics found
//!   2 — invalid configuration
//!   3 — internal error (bad path, I/O failure)
//!
//! Parse-failure regressions below pin
//! [#384](https://github.com/Nimblesite/Basilisk/issues/384). This is CLI
//! behavior rather than a Python typing rule, so its authority is the linked
//! Basilisk CLI contract, not a fabricated PEP citation.

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
    run_subcommand("check", paths, &[])
}

fn run_check_with_args(
    paths: &[&str],
    extra_args: &[&str],
) -> Result<Output, Box<dyn std::error::Error>> {
    run_subcommand("check", paths, extra_args)
}

fn run_subcommand(
    subcommand: &str,
    paths: &[&str],
    extra_args: &[&str],
) -> Result<Output, Box<dyn std::error::Error>> {
    let mut cmd = binary();
    let _ = cmd.arg(subcommand);
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

/// Stage `rels` (fixture-relative paths) into a fresh isolated project dir that
/// ships a `pyproject.toml` opting into the annotation house rules. Those rules
/// (`BSK-0001`/`BSK-0002`/…) are analyze-scope and OFF by default — the
/// binary's default config is pure PEP conformance — so a project that wants
/// them enables them in config and these tests do too. Returns the project dir
/// (caller removes it) and the staged absolute paths. No modes; this is
/// configuration. See [CHKARCH-CONFIGURATION-ONLY], [CHKARCH-COMMANDS].
fn stage_project(
    rels: &[&str],
) -> Result<(std::path::PathBuf, Vec<String>), Box<dyn std::error::Error>> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bsk_cli_bin_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\"BSK-0002\" = \"error\"\n",
    )?;
    let mut staged = Vec::with_capacity(rels.len());
    for rel in rels {
        let name = Path::new(rel)
            .file_name()
            .ok_or("fixture has no file name")?;
        let dest = dir.join(name);
        let _ = std::fs::copy(fixture(rel), &dest)?;
        staged.push(dest.to_string_lossy().into_owned());
    }
    Ok((dir, staged))
}

/// `basilisk analyze` over fixtures staged into a house-rules-enabled project
/// — house-rule diagnostics are analyze-scope ([CHKARCH-COMMANDS]).
fn run_analyze_staged(rels: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    run_analyze_staged_with_args(rels, &[])
}

/// `basilisk analyze <args>` over fixtures staged into a house-rules-enabled
/// project.
fn run_analyze_staged_with_args(
    rels: &[&str],
    extra_args: &[&str],
) -> Result<Output, Box<dyn std::error::Error>> {
    let (dir, staged) = stage_project(rels)?;
    let staged_refs: Vec<&str> = staged.iter().map(String::as_str).collect();
    let out = run_subcommand("analyze", &staged_refs, extra_args);
    let _ = std::fs::remove_dir_all(&dir);
    out
}

// ── Shipwright version contract ─────────────────────────────────────────────

#[test]
fn version_plain_matches_shipwright_contract() -> Result<(), Box<dyn std::error::Error>> {
    let out = binary().arg("--version").output()?;
    assert_eq!(out.status.code(), Some(0), "--version must exit 0");
    // Line 1 is the Shipwright contract; line 2 lists the embedded formatter
    // engine ([LSPFMT-PROVENANCE]).
    assert_eq!(
        stdout(&out).trim(),
        concat!("basilisk ", env!("CARGO_PKG_VERSION"), "\nRuff formatter: ").to_owned()
            + basilisk_lsp::formatting::EMBEDDED_RUFF_FORMATTER_VERSION,
        "--version must emit '<component-id> <semver>' then the embedded engine line"
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
    assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["kind"], "lsp");
    assert_eq!(value["language"], "rust");
    assert_eq!(value["product"], "basilisk");
    assert!(
        value.get("buildTime").is_some(),
        "buildTime must be present"
    );
    assert!(value.get("gitDirty").is_some(), "gitDirty must be present");
    assert!(
        out.stderr.is_empty(),
        "--version --json must not emit diagnostics to stderr"
    );
    Ok(())
}

// ── Exit codes ───────────────────────────────────────────────────────────────
// Exercises [CHKARCH-CLI-EXITCODES]: 0 = clean, 1 = errors, 2 = invalid
// configuration (see e2e_scope.rs), 3 = internal error.

#[test]
fn exit_0_for_clean_file() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&[&fixture("clean/fully_typed_module.py")])?;
    assert_eq!(out.status.code(), Some(0), "clean file must exit 0");
    Ok(())
}

#[test]
fn exit_1_for_file_with_errors() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
    assert_eq!(out.status.code(), Some(1), "file with errors must exit 1");
    Ok(())
}

#[test]
fn exit_3_for_nonexistent_path() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_check(&["/nonexistent/path/does_not_exist.py"])?;
    assert_eq!(out.status.code(), Some(3), "bad path must exit 3");
    Ok(())
}

// A file the parser cannot read is the one case where the run has nothing to
// say about the file's contents. `--output json` used to answer that with `[]`
// — byte-for-byte the answer a clean file gets — so every machine consumer
// (CI gate, editor, review bot) read "no problems found" for a file that was
// never checked at all. The exit code was the only signal, and a consumer that
// reads the report rather than the status never saw it.
#[test]
fn json_output_reports_a_file_that_failed_to_parse() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("basilisk-json-failure-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let variants: [(&str, &[u8]); 2] = [
        ("src.py", b"def calculate_result()\n"),
        (
            "renamed_and_reformatted.py",
            b"def renamed_operation(\n    first_value,\n    second_value\n\n",
        ),
    ];

    for (name, source) in variants {
        let malformed = dir.join(name);
        std::fs::write(&malformed, source)?;
        let malformed = malformed.to_string_lossy().into_owned();

        let out = run_check_with_args(&[&malformed], &["--output", "json"])?;
        let rendered = stdout(&out);

        assert_eq!(
            out.status.code(),
            Some(1),
            "a user syntax error is a source diagnostic, never an internal checker failure"
        );
        assert_ne!(
            rendered.trim(),
            "[]",
            "JSON must never report an unparseable file the way it reports a clean one"
        );

        let value: Value = serde_json::from_str(&rendered)?;
        let items = value
            .as_array()
            .ok_or("JSON output must stay a flat array of entries")?;
        assert_eq!(
            items.len(),
            1,
            "each failed file must produce exactly one entry"
        );
        let entry = items.first().ok_or("entry missing")?;
        assert_eq!(
            entry["severity"], "error",
            "a file that cannot be parsed is an error"
        );
        assert!(
            entry["path"]
                .as_str()
                .is_some_and(|path| path.ends_with(name)),
            "the entry must name the file that failed: {entry}"
        );
        assert!(
            entry["message"]
                .as_str()
                .is_some_and(|message| message.contains("syntax error")),
            "the entry must say why the file could not be parsed: {entry}"
        );
        assert!(
            entry["code"].is_null(),
            "no checker rule produced this entry, so it must not claim a rule code: {entry}"
        );
        assert!(
            entry["line"].as_u64().is_some_and(|line| line >= 1),
            "the entry must carry a 1-based line: {entry}"
        );
        assert!(
            entry["col"].as_u64().is_some_and(|col| col >= 1),
            "the entry must carry a 1-based column: {entry}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

// The failure must not cost the run the diagnostics it did produce, and the
// clean peer's entries must stay exactly as they were.
#[test]
fn json_output_keeps_valid_diagnostics_beside_a_parse_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let (dir, staged) = stage_project(&["errors/e0001_single_param.py"])?;
    let malformed = dir.join("malformed.py");
    std::fs::write(&malformed, b"def hi()\n")?;
    let valid = staged.first().ok_or("staged diagnostic fixture missing")?;
    let malformed = malformed.to_string_lossy().into_owned();

    let out = run_subcommand("analyze", &[valid, &malformed], &["--output", "json"])?;
    let rendered = stdout(&out);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        out.status.code(),
        Some(1),
        "a user syntax error must not mask ordinary diagnostics behind internal-failure exit 3"
    );
    let value: Value = serde_json::from_str(&rendered)?;
    let items = value.as_array().ok_or("JSON output must stay an array")?;
    assert!(
        items.len() >= 2,
        "both the diagnostic and the failure must appear"
    );

    let coded: Vec<&Value> = items
        .iter()
        .filter(|item| !item["code"].is_null())
        .collect();
    let failures: Vec<&Value> = items.iter().filter(|item| item["code"].is_null()).collect();
    assert!(
        coded.iter().any(|item| item["code"] == "BSK-0001"),
        "the valid peer's diagnostics must survive the failure: {rendered}"
    );
    assert_eq!(
        failures.len(),
        1,
        "exactly one file failed to parse: {rendered}"
    );
    assert!(
        failures.first().is_some_and(|item| item["path"]
            .as_str()
            .is_some_and(|p| p.ends_with("malformed.py"))),
        "the failure entry must name the file that failed: {rendered}"
    );
    for item in coded {
        assert!(
            item["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("BSK-")),
            "a rule-produced entry keeps its code: {item}"
        );
    }
    Ok(())
}

// A clean run must be untouched by the change: still an empty array, still 0.
#[test]
fn json_output_for_a_clean_file_is_still_an_empty_array() -> Result<(), Box<dyn std::error::Error>>
{
    let out = run_check_with_args(
        &[&fixture("clean/fully_typed_module.py")],
        &["--output", "json"],
    )?;
    assert_eq!(out.status.code(), Some(0), "a clean file must exit 0");
    assert_eq!(stdout(&out).trim(), "[]", "a clean file reports no entries");
    Ok(())
}

#[test]
fn syntax_failure_exits_one_without_dropping_valid_diagnostics(
) -> Result<(), Box<dyn std::error::Error>> {
    let (dir, staged) = stage_project(&["errors/e0001_single_param.py"])?;
    let malformed = dir.join("malformed.py");
    std::fs::write(&malformed, b"def broken(:\n")?;
    let valid = staged.first().ok_or("staged diagnostic fixture missing")?;
    let malformed = malformed.to_string_lossy().into_owned();

    // `analyze` renders the staged house-rule debt ([CHKARCH-COMMANDS]);
    // the malformed peer must fail the run without dropping it.
    let mixed = run_subcommand("analyze", &[valid, &malformed], &[])?;
    let malformed_only = run_check(&[&malformed])?;
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        mixed.status.code(),
        Some(1),
        "a mixed run containing source errors must exit 1, not internal-failure 3"
    );
    assert!(
        stdout(&mixed).contains("BSK-0001"),
        "a valid peer's diagnostics must still be rendered: {}",
        stdout(&mixed)
    );
    assert_eq!(
        malformed_only.status.code(),
        Some(1),
        "a malformed requested file is a source error and must exit 1"
    );
    assert!(
        !stdout(&malformed_only).contains("No issues found"),
        "analysis failure must never print a clean-success message: {}",
        stdout(&malformed_only)
    );
    Ok(())
}

/// [#384](https://github.com/Nimblesite/Basilisk/issues/384): the human-readable
/// report has the same obligation as JSON under [CHKARCH-CLI-OUTPUT-FAILURES].
/// A syntax error must be visible, located, and included in the summary.
#[test]
fn text_output_reports_and_counts_a_file_that_failed_to_parse(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("basilisk-text-failure-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let variants: [(&str, &[u8]); 2] = [
        ("broken.py", b"value = object()\nvalue.\n"),
        (
            "reformatted.py",
            b"renamed_symbol = (\n    object()\n)\nrenamed_symbol.\n",
        ),
    ];
    for (name, source) in variants {
        let malformed = dir.join(name);
        std::fs::write(&malformed, source)?;
        let malformed = malformed.to_string_lossy().into_owned();

        let out = run_check(&[&malformed])?;
        let rendered = stdout(&out);

        assert_eq!(
            out.status.code(),
            Some(1),
            "a user syntax error must use the source-diagnostic exit code"
        );
        assert!(
            rendered.contains(name),
            "the report body must name the unanalysable file: {rendered}"
        );
        assert!(
            rendered.to_ascii_lowercase().contains("syntax"),
            "the report body must identify the syntax failure: {rendered}"
        );
        assert!(
            rendered.contains(":") && !rendered.contains("byte range"),
            "the syntax failure must use an actionable line/column location, not a byte range: {rendered}"
        );
        assert!(
            rendered.contains("1 diagnostic") || rendered.contains("1 error"),
            "the summary must count the unanalysable file: {rendered}"
        );
        assert!(
            !rendered.contains("No issues found"),
            "an unanalysable file must never render as clean: {rendered}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
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
// Exercises [CHKARCH-CLI-OUTPUT] (human-readable text default) and the
// rustc-standard layout of [CHKARCH-DIAGEXP-QUALITY]: code, `-->` location,
// source snippet, caret underline, and `= help:`/`= note:`/`= see:` lines.

#[test]
fn output_contains_error_code_e0001() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
    assert!(
        stdout(&out).contains("BSK-0001"),
        "output must contain BSK-0001, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_error_code_e0002() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0002_single_func.py"])?;
    assert!(
        stdout(&out).contains("BSK-0002"),
        "output must contain BSK-0002, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_rustc_style_arrow() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
    assert!(
        stdout(&out).contains("-->"),
        "output must contain --> location marker, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_source_snippet() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
    let text = stdout(&out);
    assert!(
        text.contains("def process(data)"),
        "output must contain the source line, got:\n{text}"
    );
    Ok(())
}

#[test]
fn output_contains_caret_underline() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
    assert!(
        stdout(&out).contains('^'),
        "output must contain caret underline, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_help_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
    assert!(
        stdout(&out).contains("= help:"),
        "output must contain help annotation, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_note_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
    assert!(
        stdout(&out).contains("= note:"),
        "output must contain note annotation, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_see_url() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
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
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
    assert!(
        stdout(&out).contains("1:13"),
        "output must contain line:col 1:13, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_contains_diagnostic_summary() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
    assert!(
        stdout(&out).contains("diagnostic"),
        "output must contain summary line, got:\n{}",
        stdout(&out)
    );
    Ok(())
}

#[test]
fn output_shows_correct_error_count() -> Result<(), Box<dyn std::error::Error>> {
    // missing_both.py has 3 x BSK-0001 + 2 x BSK-0002 = 5 errors
    let out = run_analyze_staged(&["missing_both.py"])?;
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
    let out = run_analyze_staged(&[
        "errors/e0001_single_param.py",
        "errors/e0002_single_func.py",
    ])?;
    let text = stdout(&out);
    assert!(text.contains("BSK-0001"), "must flag BSK-0001");
    assert!(text.contains("BSK-0002"), "must flag BSK-0002");
    assert_eq!(out.status.code(), Some(1));
    Ok(())
}

#[test]
fn clean_and_error_file_together_exits_1() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&[
        "clean/fully_typed_module.py",
        "errors/e0001_single_param.py",
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

#[test]
fn bound_method_does_not_collide_with_same_named_function() -> Result<(), Box<dyn std::error::Error>>
{
    let out = run_check(&[&fixture("clean/typed_optional.py")])?;
    assert_eq!(
        out.status.code(),
        Some(0),
        "`haystack.find(needle)` must not be checked against the module function `find(haystack, needle)`:\n{}",
        stdout(&out)
    );
    Ok(())
}

// ── Output severity label ─────────────────────────────────────────────────────

#[test]
fn output_severity_label_is_error() -> Result<(), Box<dyn std::error::Error>> {
    let out = run_analyze_staged(&["errors/e0001_single_param.py"])?;
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
    let out =
        run_analyze_staged_with_args(&["errors/e0001_single_param.py"], &["--color", "always"])?;
    let text = stdout(&out);
    assert!(
        text.contains(BOLD_RED),
        "--color always must emit bold-red ANSI for error label, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_error_code() -> Result<(), Box<dyn std::error::Error>> {
    let out =
        run_analyze_staged_with_args(&["errors/e0001_single_param.py"], &["--color", "always"])?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD}[BSK-0001]")),
        "--color always must emit bold error code, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_arrow() -> Result<(), Box<dyn std::error::Error>> {
    let out =
        run_analyze_staged_with_args(&["errors/e0001_single_param.py"], &["--color", "always"])?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_BLUE}-->")),
        "--color always must emit bold-blue arrow, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_pipe() -> Result<(), Box<dyn std::error::Error>> {
    let out =
        run_analyze_staged_with_args(&["errors/e0001_single_param.py"], &["--color", "always"])?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_BLUE}|")),
        "--color always must emit bold-blue pipe, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_caret_underline() -> Result<(), Box<dyn std::error::Error>> {
    let out =
        run_analyze_staged_with_args(&["errors/e0001_single_param.py"], &["--color", "always"])?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_RED}^")),
        "--color always must emit bold-red caret underline, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_help_label() -> Result<(), Box<dyn std::error::Error>> {
    let out =
        run_analyze_staged_with_args(&["errors/e0001_single_param.py"], &["--color", "always"])?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_CYAN}help")),
        "--color always must emit bold-cyan help label, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_note_label() -> Result<(), Box<dyn std::error::Error>> {
    let out =
        run_analyze_staged_with_args(&["errors/e0001_single_param.py"], &["--color", "always"])?;
    let text = stdout(&out);
    assert!(
        text.contains(&format!("{BOLD_CYAN}note")),
        "--color always must emit bold-cyan note label, got:\n{text}"
    );
    Ok(())
}

#[test]
fn color_always_emits_ansi_for_see_label() -> Result<(), Box<dyn std::error::Error>> {
    let out =
        run_analyze_staged_with_args(&["errors/e0001_single_param.py"], &["--color", "always"])?;
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

// ── Tracing log ANSI (issue #23) ──────────────────────────────────────────────

/// Regression for issue #23: when the `basilisk` binary runs as a subprocess
/// (e.g. the LSP launched by the VS Code extension), its stderr is a pipe, not
/// a terminal. The structured `tracing` logs written to stderr must NOT contain
/// raw ANSI colour escape sequences in that case — otherwise the VS Code output
/// channel renders them as garbage (as reported in the bug). `Command::output`
/// captures stderr through a pipe, faithfully reproducing the non-terminal case.
#[test]
fn tracing_logs_emit_no_ansi_on_piped_stderr() -> Result<(), Box<dyn std::error::Error>> {
    // BASILISK_LOG=info guarantees `run_check` emits at least its
    // "loaded config" info line to stderr, so stderr is non-empty.
    let out = binary()
        .arg("check")
        .arg(fixture("clean/fully_typed_module.py"))
        .env("BASILISK_LOG", "info")
        .output()?;

    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        !stderr.is_empty(),
        "BASILISK_LOG=info must produce tracing output on stderr so this test is meaningful"
    );
    assert!(
        !stderr.contains('\u{1b}'),
        "tracing logs on piped (non-terminal) stderr must contain no ANSI escapes, got:\n{stderr}"
    );
    Ok(())
}
