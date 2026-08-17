//! Tests for [CHKARCH-CONFIG-INCLUDE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-INCLUDE
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! Coarse end-to-end tests for `[tool.basilisk] include` as the default check
//! roots (issue #37): a no-args `basilisk check` must walk only the configured
//! include roots instead of the whole repository, so files the user excluded
//! by omission (vendored/generated trees) can no longer crash the process.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// A throwaway directory unique to this process and call.
fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bsk_include_{prefix}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Run `basilisk check` with no path arguments from inside `dir`.
fn check_no_args(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .current_dir(dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk")
}

/// Lay down a project with `[tool.basilisk] include = ["src/", "tests/"]`,
/// clean included sources, and `gen/` content OUTSIDE the include roots.
fn write_include_project(dir: &Path, generated: &str) {
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ninclude = [\"src/\", \"tests/\"]\nexclude = [\"**/migrations/**\"]\n",
    )
    .expect("write pyproject");
    std::fs::create_dir_all(dir.join("src")).expect("mkdir src");
    std::fs::create_dir_all(dir.join("tests")).expect("mkdir tests");
    std::fs::create_dir_all(dir.join("gen")).expect("mkdir gen");
    std::fs::write(
        dir.join("src/main.py"),
        "def add(a: int, b: int) -> int:\n    return a + b\n",
    )
    .expect("write src");
    std::fs::write(
        dir.join("tests/test_main.py"),
        "def test_add() -> None:\n    assert 1 + 1 == 2\n",
    )
    .expect("write tests");
    std::fs::write(dir.join("gen/deep.py"), generated).expect("write gen");
}

/// Issue #37 repro: a deeply nested expression in a file outside the include
/// roots overflowed the stack because the no-args run walked the whole repo.
#[test]
fn no_args_honors_include_and_does_not_overflow() {
    let dir = unique_dir("overflow");
    let deep = format!("x = {}1{}\n", "(".repeat(20000), ")".repeat(20000));
    write_include_project(&dir, &deep);

    let output = check_no_args(&dir);

    // A stack-overflow abort yields no exit code on Unix; the fixed binary
    // must exit cleanly without ever parsing gen/deep.py.
    assert_eq!(
        output.status.code(),
        Some(0),
        "no-args check must honor include and exit 0, got status {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The assertive pair for the include semantics: diagnostics inside include
/// roots still fire, and files outside them are not checked at all.
#[test]
fn no_args_checks_include_roots_only() {
    let dir = unique_dir("roots");
    write_include_project(&dir, "def broken() -> int:\n    return \"nope\"\n");
    // Add a real error inside an include root.
    std::fs::write(
        dir.join("src/bad.py"),
        "def bad() -> int:\n    return \"oops\"\n",
    )
    .expect("write bad");

    let output = check_no_args(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("bad.py"),
        "errors inside include roots must still be reported, got: {stdout}"
    );
    assert!(
        !stdout.contains("deep.py"),
        "files outside include roots must not be checked, got: {stdout}"
    );
}

/// Lay down a project whose `include` is `src/` only, with a fixable
/// `BSK-0050` violation inside the include root and an identical one inside a
/// vendored virtualenv that the config's `exclude` does not name.
fn write_fix_include_project(dir: &Path) {
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ninclude = [\"src/\"]\nexclude = [\"**/migrations/**\"]\n\n[tool.basilisk.rules]\n\"BSK-0050\" = \"warning\"\n",
    )
    .expect("write pyproject");
    let vendored = dir.join("venv/lib/python3.13/site-packages/dep");
    std::fs::create_dir_all(dir.join("src")).expect("mkdir src");
    std::fs::create_dir_all(&vendored).expect("mkdir vendored");
    std::fs::write(dir.join("venv/pyvenv.cfg"), "home = /usr\n").expect("write pyvenv.cfg");
    std::fs::write(dir.join("src/main.py"), "x: int = 42\n").expect("write src");
    std::fs::write(vendored.join("mod.py"), "y: int = 42\n").expect("write vendored");
}

/// Issue #333: `basilisk fix` defaulted `PATHS` to `.` instead of falling back
/// to the configured `include` roots like `check`/`analyze`, so a no-args run
/// walked — and **rewrote** — third-party sources inside `venv/`.
#[test]
fn fix_no_args_honors_include_and_never_rewrites_vendored_files() {
    let dir = unique_dir("fix_roots");
    write_fix_include_project(&dir);
    let vendored = dir.join("venv/lib/python3.13/site-packages/dep/mod.py");

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("fix")
        .args(["--rules", "BSK-0050"])
        .current_dir(&dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk");

    assert_eq!(
        std::fs::read_to_string(&vendored).expect("read vendored"),
        "y: int = 42\n",
        "a no-args fix must never mutate files outside the include roots, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("src/main.py")).expect("read src"),
        "x = 42\n",
        "a no-args fix must still fix files inside the include roots, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Explicit CLI paths override the configured include roots.
#[test]
fn explicit_paths_override_include() {
    let dir = unique_dir("explicit");
    write_include_project(&dir, "def broken() -> int:\n    return \"nope\"\n");

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("gen")
        .current_dir(&dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("deep.py"),
        "explicit paths must win over include, got: {stdout}"
    );
}
