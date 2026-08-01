//! Tests for [CHKARCH-COMMANDS] / [CHKARCH-CONFIG-MODEL]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-COMMANDS
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! E2E tests for the check/analyze command partition through the real binary.
//!
//! One rule universe, partitioned once by provenance tag: `basilisk check`
//! emits only `pep`-tagged rules and always runs them; `basilisk analyze`
//! emits only the rest and runs them only when configuration resolves them to
//! a non-disabled severity. A configuration that resolves a `pep` rule to
//! `disabled` is invalid and exits 2 ([CHKARCH-CLI-EXITCODES]).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// A throwaway directory unique to this process and call.
fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bsk_scope_{prefix}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Run `basilisk <subcommand> <path> --output json --color never`.
fn run(subcommand: &str, path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg(subcommand)
        .arg(path)
        .args(["--output", "json", "--color", "never"])
        .output()
        .expect("spawn basilisk")
}

/// Run `basilisk <subcommand> <path> --color never` in the default text format.
fn run_text(subcommand: &str, path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg(subcommand)
        .arg(path)
        .args(["--color", "never"])
        .output()
        .expect("spawn basilisk")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The diagnostic codes in a JSON run's output.
fn json_codes(output: &Output) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(&stdout(output)).expect("valid JSON");
    value
        .as_array()
        .expect("JSON output is an array")
        .iter()
        .map(|d| d["code"].as_str().expect("code is a string").to_owned())
        .collect()
}

/// Source that violates the opt-in annotation house rules but no pep rule.
const HOUSE_DEBT_ONLY: &str = "def foo(x):\n    return x\n";

/// [CHKARCH-COMMANDS]: on a bare tree (no `[tool.basilisk]` anywhere),
/// `analyze` runs nothing — no entry, no check — even on code full of
/// house-rule debt.
#[test]
fn analyze_runs_nothing_on_bare_tree() {
    let dir = unique_dir("bare");
    let py = dir.join("m.py");
    std::fs::write(&py, HOUSE_DEBT_ONLY).expect("write module");

    let out = run("analyze", &py);
    assert_eq!(
        out.status.code(),
        Some(0),
        "a bare tree must analyze clean, stdout: {}",
        stdout(&out)
    );
    assert!(
        json_codes(&out).is_empty(),
        "a bare tree must produce zero analyze diagnostics, got: {}",
        stdout(&out)
    );
}

/// [CHKARCH-CONFIG-MODEL]: one written `rule-tags` line (`"basilisk" =
/// "error"`) selects and grades every house rule — `analyze` then fires them.
#[test]
fn analyze_fires_house_rule_selected_by_tag_entry() {
    let dir = unique_dir("tag");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk.rule-tags]\n\"basilisk\" = \"error\"\n",
    )
    .expect("write config");
    let py = dir.join("m.py");
    std::fs::write(&py, HOUSE_DEBT_ONLY).expect("write module");

    let out = run("analyze", &py);
    assert_eq!(
        out.status.code(),
        Some(1),
        "tag-selected house errors must exit 1, stdout: {}",
        stdout(&out)
    );
    let codes = json_codes(&out);
    assert!(
        codes.iter().any(|code| code == "BSK-0001"),
        "the `basilisk` tag entry must select BSK-0001 under analyze, got: {codes:?}"
    );
    assert!(
        codes
            .iter()
            .all(|code| !basilisk_checker::is_pep_rule(code)),
        "analyze must emit only non-pep diagnostics ([CHKARCH-COMMANDS]), got: {codes:?}"
    );
}

/// [CHKARCH-COMMANDS]: `check` never emits house diagnostics — even when
/// configuration explicitly selects them at `error`.
#[test]
fn check_never_emits_house_diagnostics_even_when_configured() {
    let dir = unique_dir("checkscope");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\"BSK-0002\" = \"error\"\n",
    )
    .expect("write config");
    let py = dir.join("m.py");
    std::fs::write(&py, HOUSE_DEBT_ONLY).expect("write module");

    let out = run("check", &py);
    assert_eq!(
        out.status.code(),
        Some(0),
        "check must exit 0 — the debt is analyze-scope, stdout: {}",
        stdout(&out)
    );
    let codes = json_codes(&out);
    assert!(
        codes.is_empty(),
        "check must emit no house diagnostics ([CHKARCH-COMMANDS]), got: {codes:?}"
    );

    // The same project's debt IS visible to `analyze` — the partition, not a
    // silent drop.
    let analyze = run("analyze", &py);
    assert_eq!(analyze.status.code(), Some(1));
}

/// [CHKARCH-COMMANDS]: `check` emits pep diagnostics on a bare tree with the
/// documented JSON shape — the surface the conformance harness invokes.
#[test]
fn check_emits_pep_diagnostics_with_stable_json_shape() {
    let dir = unique_dir("pepjson");
    let py = dir.join("m.py");
    std::fs::write(&py, "def bad() -> int:\n    return \"x\"\n").expect("write module");

    let out = run("check", &py);
    assert_eq!(out.status.code(), Some(1), "pep errors must exit 1");
    let value: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("valid JSON");
    let diagnostics = value.as_array().expect("array");
    assert!(!diagnostics.is_empty(), "a pep diagnostic must be emitted");
    for diagnostic in diagnostics {
        for key in ["path", "line", "col", "severity", "message", "code"] {
            assert!(
                diagnostic.get(key).is_some(),
                "JSON diagnostics must carry `{key}`, got: {diagnostic}"
            );
        }
        let code = diagnostic["code"].as_str().expect("code is a string");
        assert!(
            basilisk_checker::is_pep_rule(code),
            "check must emit only pep-tagged codes, got: {code}"
        );
    }
}

// ── the check/analyze split is never silent ([CHKARCH-CLI-SCOPE-NOTICE]) ─────

/// Refs #334. `analyze` is the ONLY command that runs the opt-in rule layer,
/// so a user who cannot see it in `basilisk --help` cannot discover that their
/// configured rules were never evaluated. Discoverability is the fix; the
/// subcommand existing but being unlisted is what made 66 configured errors
/// invisible for the life of a CI pipeline.
#[test]
fn top_level_help_lists_every_rule_running_command() {
    let out = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("--help")
        .output()
        .expect("spawn basilisk");
    let text = stdout(&out);

    for command in ["check", "analyze", "fix"] {
        assert!(
            text.lines()
                .any(|line| line.trim_start().starts_with(&format!("{command} "))),
            "`basilisk --help` must list the `{command}` subcommand, got: {text}"
        );
    }
}

/// Refs #334. A clean `check` on a project whose configuration selects
/// analyze-scope rules must say so. Otherwise a silent clean run is
/// indistinguishable from a real one: the reporter's project graded eight rule
/// tags `error`, ran `check` in CI, saw "All checked. No issues found." — and
/// 66 configured errors were never evaluated for the life of the pipeline.
#[test]
fn check_reports_configured_rules_its_scope_did_not_run() {
    let dir = unique_dir("noticeclean");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk.rule-tags]\n\"basilisk\" = \"error\"\n",
    )
    .expect("write config");
    let py = dir.join("m.py");
    std::fs::write(&py, HOUSE_DEBT_ONLY).expect("write module");

    let out = run_text("check", &py);
    assert_eq!(
        out.status.code(),
        Some(0),
        "the debt is analyze-scope, so check still exits 0, stdout: {}",
        stdout(&out)
    );
    let text = stdout(&out);
    assert!(
        text.contains("All checked. No issues found."),
        "check must still report its own clean result, got: {text}"
    );
    assert!(
        text.contains("basilisk analyze"),
        "a clean check must point at `basilisk analyze` when configuration \
         selects rules this scope never ran ([CHKARCH-CLI-SCOPE-NOTICE]), got: {text}"
    );
}

/// The notice is a fact about *this* project, not boilerplate: a bare tree
/// selects no analyze-scope rule, so a clean `check` says nothing extra.
#[test]
fn check_stays_quiet_when_configuration_selects_no_analyze_rule() {
    let dir = unique_dir("noticebare");
    let py = dir.join("m.py");
    std::fs::write(&py, HOUSE_DEBT_ONLY).expect("write module");

    let text = stdout(&run_text("check", &py));
    assert!(
        !text.contains("basilisk analyze"),
        "a bare tree selects nothing, so check must not advertise analyze, got: {text}"
    );
}

/// `analyze` already ran those rules — it must never tell the user to run it.
#[test]
fn analyze_never_advertises_itself() {
    let dir = unique_dir("noticeanalyze");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk.rule-tags]\n\"basilisk\" = \"error\"\n",
    )
    .expect("write config");
    let py = dir.join("m.py");
    std::fs::write(&py, HOUSE_DEBT_ONLY).expect("write module");

    let text = stdout(&run_text("analyze", &py));
    assert!(
        !text.contains("basilisk analyze"),
        "analyze ran the configured rules; pointing at itself is noise, got: {text}"
    );
}

/// [CHKARCH-CONFIG-MODEL]: a config resolving a pep rule to `disabled` is
/// invalid — both commands fail with exit 2 and a stderr explanation, before
/// checking.
#[test]
fn pep_disable_is_a_config_error() {
    let dir = unique_dir("pepdisable");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk.rules]\n\"imports_unresolved\" = \"disabled\"\n",
    )
    .expect("write config");
    let py = dir.join("m.py");
    std::fs::write(&py, "x: int = 1\n").expect("write module");

    for subcommand in ["check", "analyze"] {
        let out = run(subcommand, &py);
        assert_eq!(
            out.status.code(),
            Some(2),
            "`{subcommand}` must exit 2 on a pep-disable config ([CHKARCH-CLI-EXITCODES])"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("imports_unresolved"),
            "`{subcommand}` must name the offending code on stderr, got: {stderr}"
        );
    }
}

/// A tag entry can also invalidly disable pep rules — `rule-tags."pep" =
/// "disabled"` resolves every pep rule to disabled and must exit 2.
#[test]
fn pep_tag_disable_is_a_config_error() {
    let dir = unique_dir("peptagdisable");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk.rule-tags]\n\"pep\" = \"disabled\"\n",
    )
    .expect("write config");
    let py = dir.join("m.py");
    std::fs::write(&py, "x: int = 1\n").expect("write module");

    let out = run("check", &py);
    assert_eq!(
        out.status.code(),
        Some(2),
        "a pep tag-disable must exit 2 ([CHKARCH-CONFIG-MODEL])"
    );
}
