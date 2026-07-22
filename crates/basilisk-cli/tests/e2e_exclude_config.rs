//! End-to-end tests for `exclude` semantics ([CHKARCH-CONFIG-EXCLUDE]).
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-EXCLUDE
//!
//! The contract under test, exactly as a user experiences it through the
//! real binary:
//! - With no `exclude` key, [`basilisk_config::DEFAULT_EXCLUDES`] applies:
//!   vendored/cache trees (`node_modules`, `site-packages`, `build`, …) are
//!   never scanned.
//! - Setting `exclude` REPLACES the defaults entirely — it does not extend
//!   them. A project that still wants `node_modules` skipped must re-add it.
//! - Hidden (`.`-prefixed) directories are always skipped regardless.
//! - Virtualenvs are skipped structurally, by their PEP 405 `pyvenv.cfg`
//!   marker, regardless of directory name or `exclude` configuration.
//! - Patterns are gitignore-style: a bare name matches at any depth; an
//!   anchored `dir/**` pattern excludes the whole subtree.
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
/// `returns_compatibility` diagnostics when the file is scanned.
const BAD_PY: &str = "def f() -> int:\n    return \"bad\"\n";

fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bsk_exclude_config_{prefix}_{}_{n}",
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

fn pyproject_with(dir: &Path, basilisk_table: &str) {
    write(
        dir,
        "pyproject.toml",
        &format!(
            "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\n{basilisk_table}"
        ),
    );
}

fn check_dot(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg(".")
        .current_dir(dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn default_excludes_skip_vendored_and_cache_directories() {
    let dir = unique_dir("defaults");
    pyproject_with(&dir, "");
    write(&dir, "node_modules/bad.py", BAD_PY);
    write(&dir, "site-packages/bad.py", BAD_PY);
    write(&dir, "build/bad.py", BAD_PY);
    write(&dir, "dist/bad.py", BAD_PY);
    write(&dir, "__pycache__/bad.py", BAD_PY);
    write(&dir, "ok.py", "value: int = 1\n");

    let output = check_dot(&dir);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains("returns_compatibility"),
        "DEFAULT_EXCLUDES directories must never be scanned when `exclude` is unset, stdout: {stdout}, stderr: {stderr}"
    );
    for skipped in [
        "node_modules",
        "site-packages",
        "build",
        "dist",
        "__pycache__",
    ] {
        assert!(
            !stdout.contains(skipped),
            "no diagnostic may name the default-excluded `{skipped}` tree, stdout: {stdout}"
        );
    }
    assert_eq!(
        output.status.code(),
        Some(0),
        "a project whose only defects sit inside default-excluded trees must pass, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn setting_exclude_replaces_the_default_list_entirely() {
    let dir = unique_dir("replace");
    pyproject_with(&dir, "exclude = [\"generated\"]\n");
    write(&dir, "node_modules/bad.py", BAD_PY);
    write(&dir, "generated/bad.py", BAD_PY);

    let output = check_dot(&dir);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("returns_compatibility"),
        "`exclude = [\"generated\"]` replaces the defaults, so node_modules must now be scanned and its defect reported, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stdout.contains("node_modules"),
        "the diagnostic must point into the now-scanned node_modules tree, stdout: {stdout}"
    );
    assert!(
        !stdout.contains("generated"),
        "the user's own `generated` entry must still be excluded, stdout: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "a defect in a no-longer-excluded tree is a real finding, so the CLI must exit 1, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn re_adding_a_default_entry_restores_its_exclusion() {
    let dir = unique_dir("readd");
    pyproject_with(&dir, "exclude = [\"generated\", \"node_modules\"]\n");
    write(&dir, "node_modules/bad.py", BAD_PY);
    write(&dir, "generated/bad.py", BAD_PY);
    write(&dir, "ok.py", "value: int = 1\n");

    let output = check_dot(&dir);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains("returns_compatibility"),
        "explicitly re-added default entries must be excluded again, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "both excluded trees are skipped, so the check must pass, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hidden_directories_are_always_skipped_even_with_custom_exclude() {
    let dir = unique_dir("hidden");
    pyproject_with(&dir, "exclude = [\"generated\"]\n");
    write(&dir, ".hidden/bad.py", BAD_PY);
    write(&dir, "ok.py", "value: int = 1\n");

    let output = check_dot(&dir);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains(".hidden") && !stdout.contains("returns_compatibility"),
        "`.`-prefixed directories are skipped regardless of the user's `exclude` list, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a defect inside a hidden directory must never fail the check, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bare_name_pattern_matches_at_any_depth() {
    let dir = unique_dir("any_depth");
    pyproject_with(&dir, "exclude = [\"generated\"]\n");
    write(&dir, "a/generated/bad.py", BAD_PY);
    write(&dir, "ok.py", "value: int = 1\n");

    let output = check_dot(&dir);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains("returns_compatibility"),
        "a bare `generated` pattern must also exclude the nested a/generated tree, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "nothing outside excluded trees is defective, so the check must pass, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn anchored_glob_excludes_the_whole_subtree() {
    let dir = unique_dir("anchored");
    pyproject_with(&dir, "exclude = [\"vendor/**\"]\n");
    write(&dir, "vendor/sub/bad.py", BAD_PY);
    write(&dir, "ok.py", "value: int = 1\n");

    let output = check_dot(&dir);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains("returns_compatibility"),
        "`vendor/**` must exclude every file beneath vendor/, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the only defect sits inside the excluded subtree, so the check must pass, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Lay down a project whose custom `exclude` replaces the defaults (so no
/// `venv`/`site-packages` entry survives) with a real virtualenv beside the
/// sources, marked by PEP 405's `pyvenv.cfg`.
fn write_project_with_virtualenv(dir: &Path, vendored: &str) {
    pyproject_with(
        dir,
        "exclude = [\"generated\"]\n\n[tool.basilisk.rules]\n\"BSK-0050\" = \"warning\"\n",
    );
    write(dir, "venv/pyvenv.cfg", "home = /usr\n");
    write(
        dir,
        "venv/lib/python3.13/site-packages/dep/mod.py",
        vendored,
    );
    write(dir, "src/main.py", "x: int = 42\n");
}

/// Issue #341: a virtualenv is skipped today only because `venv`/`.venv`/
/// `site-packages` happen to be literal entries in `DEFAULT_EXCLUDES` — and any
/// custom `exclude` replaces that list wholesale. `fix` mutates files, so the
/// gap rewrites third-party installed packages. The venv must be pruned
/// structurally, by its `pyvenv.cfg` marker, whatever `exclude` says.
#[test]
fn fix_never_rewrites_inside_a_virtualenv_when_custom_exclude_replaces_defaults() {
    let dir = unique_dir("venv_fix");
    write_project_with_virtualenv(&dir, "y: int = 42\n");

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("fix")
        .args(["--rules", "BSK-0050"])
        .current_dir(&dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        std::fs::read_to_string(dir.join("venv/lib/python3.13/site-packages/dep/mod.py"))
            .expect("read vendored"),
        "y: int = 42\n",
        "`fix` must never mutate third-party sources inside a virtualenv, however \
         `exclude` is configured, stdout: {}, stderr: {stderr}",
        stdout_of(&output)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("src/main.py")).expect("read src"),
        "x = 42\n",
        "the project's own sources must still be fixed, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The read-only half of the same walk: `check` must not report diagnostics
/// from inside a virtualenv either, or the editor and CLI disagree about which
/// files exist ([CHKARCH-CONFIG-EXCLUDE]).
#[test]
fn check_does_not_scan_inside_a_virtualenv_when_custom_exclude_replaces_defaults() {
    let dir = unique_dir("venv_check");
    write_project_with_virtualenv(&dir, BAD_PY);

    let output = check_dot(&dir);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains("returns_compatibility"),
        "a defect inside a virtualenv must never be reported, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the only defect sits inside the virtualenv, so the check must pass, \
         stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The structural skip prunes *traversal into* a virtualenv; it does not
/// override an explicit request. Pointing the CLI straight at a path inside one
/// still checks it, mirroring the walk's existing depth-0 root exemption.
#[test]
fn an_explicit_path_inside_a_virtualenv_is_still_checked() {
    let dir = unique_dir("venv_explicit");
    write_project_with_virtualenv(&dir, BAD_PY);

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("venv/lib/python3.13/site-packages/dep")
        .current_dir(&dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk");
    let stdout = stdout_of(&output);

    assert!(
        stdout.contains("returns_compatibility"),
        "an explicitly requested path must still be checked, stdout: {stdout}, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn excluded_defect_does_not_mask_a_real_one_outside() {
    let dir = unique_dir("mixed");
    pyproject_with(&dir, "exclude = [\"generated\"]\n");
    write(&dir, "generated/bad.py", BAD_PY);
    write(&dir, "src/real_bug.py", BAD_PY);

    let output = check_dot(&dir);
    let stdout = stdout_of(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("real_bug.py") && stdout.contains("returns_compatibility"),
        "the defect outside the excluded tree must still be reported, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        !stdout.contains("generated"),
        "the excluded tree must contribute no diagnostics, stdout: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "one real defect means exit 1, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
