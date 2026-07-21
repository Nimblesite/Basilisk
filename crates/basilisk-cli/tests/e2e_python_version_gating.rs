//! E2E tests: the `python-version` config key gates stdlib module
//! availability against typeshed's `stdlib/VERSIONS` ranges.
//!
//! Covers [STUBRES-TYPESHED-VERSION] (docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md
//! "Target Python version") and [CHKARCH-VERSION-TARGET]
//! (docs/specs/CHECKER-ARCHITECTURE-SPEC.md "Target Version and Platform").
//!
//! Each test drives the real binary end to end: a temp project with a
//! `[tool.basilisk]` `python-version` in `pyproject.toml`, an `app.py`
//! importing a version-gated stdlib module, then asserts on stdout AND the
//! exit code. VERSIONS ranges exercised: `tomllib: 3.11-` (introduced),
//! `distutils: 3.0-3.11` (removed in 3.12), `wsgiref.types: 3.11-`
//! (version-gated submodule of an always-present package).
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bsk_python_version_gating_{prefix}_{}_{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write a project whose `[tool.basilisk]` sets `version_key = "version"`
/// and whose `app.py` contains `source`.
fn write_project(dir: &Path, version_key: &str, version: &str, source: &str) {
    std::fs::write(
        dir.join("pyproject.toml"),
        format!(
            "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\n{version_key} = \"{version}\"\n"
        ),
    )
    .expect("write pyproject");
    std::fs::write(dir.join("app.py"), source).expect("write app");
}

fn check_app(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk")
}

/// Assert the check flagged `module` as unresolved and exited 1.
fn assert_import_flagged(output: &Output, module: &str, context: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("imports_unresolved"),
        "{context}: `{module}` must be reported as unresolved, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stdout.contains(module),
        "{context}: the diagnostic must name `{module}`, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "{context}: an unresolved import is an error, so the CLI must exit 1, stdout: {stdout}, stderr: {stderr}"
    );
}

/// Assert the check resolved `module` cleanly and exited 0.
fn assert_import_resolved(output: &Output, module: &str, context: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("imports_unresolved"),
        "{context}: `{module}` must resolve without diagnostics, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        !stdout.contains(module),
        "{context}: diagnostics must not name the resolved module `{module}`, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{context}: a resolved stdlib import must let the CLI check pass, stdout: {stdout}, stderr: {stderr}"
    );
}

/// `tomllib` is `3.11-` in typeshed's `stdlib/VERSIONS`: targeting 3.9 the
/// module does not exist yet, so the import must be flagged as unresolved.
#[test]
fn python_version_before_module_introduction_flags_import() {
    let dir = unique_dir("tomllib_39");
    write_project(&dir, "python-version", "3.9", "import tomllib\n");

    let output = check_app(&dir);
    assert_import_flagged(
        &output,
        "tomllib",
        "python-version = \"3.9\" predates tomllib (3.11- per VERSIONS)",
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Targeting 3.12 the same `tomllib` import sits inside its `3.11-` VERSIONS
/// range, so the check must pass cleanly.
#[test]
fn python_version_at_or_after_introduction_resolves_import() {
    let dir = unique_dir("tomllib_312");
    write_project(&dir, "python-version", "3.12", "import tomllib\n");

    let output = check_app(&dir);
    assert_import_resolved(
        &output,
        "tomllib",
        "python-version = \"3.12\" is within tomllib's 3.11- range",
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// `distutils` is `3.0-3.11` in VERSIONS (removed from the stdlib in 3.12):
/// the upper bound must gate too — flagged when targeting 3.12, resolved when
/// targeting 3.10.
#[test]
fn removed_module_gated_by_version() {
    let flagged_dir = unique_dir("distutils_312");
    write_project(&flagged_dir, "python-version", "3.12", "import distutils\n");
    let flagged = check_app(&flagged_dir);
    assert_import_flagged(
        &flagged,
        "distutils",
        "python-version = \"3.12\" is past distutils' 3.0-3.11 range",
    );
    let _ = std::fs::remove_dir_all(&flagged_dir);

    let resolved_dir = unique_dir("distutils_310");
    write_project(
        &resolved_dir,
        "python-version",
        "3.10",
        "import distutils\n",
    );
    let resolved = check_app(&resolved_dir);
    assert_import_resolved(
        &resolved,
        "distutils",
        "python-version = \"3.10\" is within distutils' 3.0-3.11 range",
    );
    let _ = std::fs::remove_dir_all(&resolved_dir);
}

/// The camelCase `pythonVersion` alias (pyright spelling, accepted by
/// `workspace_config_from_toml` in crates/basilisk-lsp/src/config.rs) must
/// gate exactly like `python-version`: flag `tomllib` at 3.9, resolve it
/// at 3.12.
#[test]
fn camel_case_python_version_alias_gates_identically() {
    let flagged_dir = unique_dir("camel_39");
    write_project(&flagged_dir, "pythonVersion", "3.9", "import tomllib\n");
    let flagged = check_app(&flagged_dir);
    assert_import_flagged(
        &flagged,
        "tomllib",
        "pythonVersion = \"3.9\" (camelCase alias) predates tomllib (3.11-)",
    );
    let _ = std::fs::remove_dir_all(&flagged_dir);

    let resolved_dir = unique_dir("camel_312");
    write_project(&resolved_dir, "pythonVersion", "3.12", "import tomllib\n");
    let resolved = check_app(&resolved_dir);
    assert_import_resolved(
        &resolved,
        "tomllib",
        "pythonVersion = \"3.12\" (camelCase alias) is within tomllib's 3.11- range",
    );
    let _ = std::fs::remove_dir_all(&resolved_dir);
}

/// Submodule gating: `wsgiref.types` is `3.11-` in VERSIONS while its parent
/// package `wsgiref` is `3.0-`. Targeting 3.9 the parent must resolve but the
/// submodule import must be flagged; targeting 3.12 the submodule resolves.
#[test]
fn submodule_version_gating() {
    let parent_dir = unique_dir("wsgiref_parent_39");
    write_project(&parent_dir, "python-version", "3.9", "import wsgiref\n");
    let parent = check_app(&parent_dir);
    assert_import_resolved(
        &parent,
        "wsgiref",
        "python-version = \"3.9\" is within the parent package's 3.0- range",
    );
    let _ = std::fs::remove_dir_all(&parent_dir);

    let flagged_dir = unique_dir("wsgiref_types_39");
    write_project(
        &flagged_dir,
        "python-version",
        "3.9",
        "import wsgiref.types\n",
    );
    let flagged = check_app(&flagged_dir);
    assert_import_flagged(
        &flagged,
        "wsgiref.types",
        "python-version = \"3.9\" predates the wsgiref.types submodule (3.11-)",
    );
    let _ = std::fs::remove_dir_all(&flagged_dir);

    let resolved_dir = unique_dir("wsgiref_types_312");
    write_project(
        &resolved_dir,
        "python-version",
        "3.12",
        "import wsgiref.types\n",
    );
    let resolved = check_app(&resolved_dir);
    assert_import_resolved(
        &resolved,
        "wsgiref.types",
        "python-version = \"3.12\" is within wsgiref.types' 3.11- range",
    );
    let _ = std::fs::remove_dir_all(&resolved_dir);
}
