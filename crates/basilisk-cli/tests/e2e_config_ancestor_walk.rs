//! End-to-end tests for per-file rule-config discovery
//! ([CHKARCH-CONFIG-DISCOVERY]).
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-DISCOVERY
//!
//! The contract under test, exactly as a user experiences it through the
//! real binary (GitHub #311):
//! - A FILE argument discovers its rule config from ancestor directories —
//!   the nearest `[tool.basilisk]` table that decides a rule wins, however
//!   deep the checked file sits below it.
//! - Diagnostics NEVER depend on argument order: every checked file resolves
//!   its own ancestor chain, not the first argument's.
//! - A `pyproject.toml` WITHOUT `[tool.basilisk]` contributes nothing and
//!   does not stop the walk (Ruff `[tool.ruff]` semantics).
//! - Scalar keys resolve nearest-first: a child `python-version` overrides
//!   an ancestor's for files under the child.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// A function body whose return type mismatch always produces
/// `returns_compatibility` AND `returns_compatibility_2` diagnostics when
/// the file is checked, so configs downgrade both codes together.
const BAD_PY: &str = "def f() -> int:\n    return \"bad\"\n";

fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bsk_ancestor_walk_{prefix}_{}_{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write(dir: &Path, rel: &str, contents: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(path, contents).expect("write file");
}

fn check(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .args(args)
        .current_dir(dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The blank-line-separated diagnostic records that mention `file_marker`.
/// A record couples the `severity[code]:` header with its `--> path` line,
/// so severity can be asserted per file. The docs URL inside a record
/// contains the substring `errors/`, so severity checks must match the
/// `severity[` header prefix, never a bare `error`.
fn records_for(stdout: &str, file_marker: &str) -> Vec<String> {
    stdout
        .split("\n\n")
        .map(str::trim_start)
        .filter(|block| block.contains(file_marker))
        .map(str::to_owned)
        .collect()
}

#[test]
fn file_argument_discovers_rule_config_from_ancestor_directories() {
    let dir = unique_dir("ancestor_discovery");
    write(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk.rules]\nreturns_compatibility = \"warning\"\nreturns_compatibility_2 = \"warning\"\n",
    );
    write(&dir, "pkg/app.py", BAD_PY);

    let output = check(&dir, &["pkg/app.py"]);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("warning[returns_compatibility"),
        "the root table's downgrade must reach a file passed as a nested FILE argument (GitHub #311 headline), stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        !stdout.contains("error["),
        "the downgraded diagnostics must not be reported as errors, stdout: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a warning-only run must exit 0 — the ancestor downgrade was honored, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn diagnostics_do_not_depend_on_argument_order() {
    let dir = unique_dir("argument_order");
    write(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n",
    );
    write(
        &dir,
        "a/pyproject.toml",
        "[tool.basilisk.rules]\nreturns_compatibility = \"warning\"\nreturns_compatibility_2 = \"warning\"\n",
    );
    write(&dir, "a/x.py", BAD_PY);
    write(&dir, "b/y.py", BAD_PY);

    for args in [["a/x.py", "b/y.py"], ["b/y.py", "a/x.py"]] {
        let output = check(&dir, &args);
        let stdout = stdout_of(&output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let x_records = records_for(&stdout, "x.py");
        let y_records = records_for(&stdout, "y.py");

        assert!(
            !x_records.is_empty()
                && x_records.iter().all(|r| r.starts_with("warning[")),
            "a/x.py must ALWAYS take a/'s downgrade regardless of argument order {args:?} (GitHub #311), stdout: {stdout}, stderr: {stderr}"
        );
        assert!(
            !y_records.is_empty() && y_records.iter().all(|r| r.starts_with("error[")),
            "b/y.py must ALWAYS keep the default error severity regardless of argument order {args:?}, stdout: {stdout}, stderr: {stderr}"
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "the error in b/y.py must fail the run in both orders, stdout: {stdout}, stderr: {stderr}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pyproject_without_tool_basilisk_does_not_stop_the_walk() {
    let dir = unique_dir("walk_through");
    write(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk.rules]\nreturns_compatibility = \"warning\"\nreturns_compatibility_2 = \"warning\"\n",
    );
    write(
        &dir,
        "mid/pyproject.toml",
        "[project]\nname = \"mid\"\nversion = \"0.1.0\"\n",
    );
    write(&dir, "mid/pkg/app.py", BAD_PY);

    let output = check(&dir, &["mid/pkg/app.py"]);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("warning[returns_compatibility") && !stdout.contains("error["),
        "a bare [project] pyproject in mid/ must not stop the walk — the ROOT downgrade still applies (Ruff semantics, [CHKARCH-CONFIG-DISCOVERY]), stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the run holds only the downgraded warning, so it must pass, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nearest_python_version_wins_over_ancestor() {
    let dir = unique_dir("scalar_nearest");
    write(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\npython-version = \"3.12\"\n",
    );
    write(
        &dir,
        "legacy/pyproject.toml",
        "[tool.basilisk]\npython-version = \"3.9\"\n",
    );
    // tomllib joined the stdlib in 3.11: visible on 3.9, absent-flagged there.
    write(&dir, "legacy/app.py", "import tomllib\n");
    write(&dir, "app.py", "import tomllib\n");

    let legacy = check(&dir, &["legacy/app.py"]);
    let legacy_stdout = stdout_of(&legacy);
    let legacy_stderr = String::from_utf8_lossy(&legacy.stderr);
    assert_eq!(
        legacy.status.code(),
        Some(1),
        "under legacy/'s python-version = 3.9 the tomllib import must be flagged — the CHILD scalar wins over the root's 3.12 ([CHKARCH-CONFIG-DISCOVERY] scalar merge), stdout: {legacy_stdout}, stderr: {legacy_stderr}"
    );
    assert!(
        legacy_stdout.contains("tomllib"),
        "the diagnostic must name the version-gated module, stdout: {legacy_stdout}"
    );

    let root = check(&dir, &["app.py"]);
    let root_stdout = stdout_of(&root);
    let root_stderr = String::from_utf8_lossy(&root.stderr);
    assert_eq!(
        root.status.code(),
        Some(0),
        "at the root the 3.12 target applies and tomllib is clean — proof the 3.9 came from legacy/'s table, not global state, stdout: {root_stdout}, stderr: {root_stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
