//! Tests pyright-compatibility config spellings for `basilisk check`.
//!
//! Config-file priority and spellings are specified in
//! docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CONFIG-PRI:
//! `pyrightconfig.json` first, then `pyproject.toml` `[tool.basilisk]` or,
//! failing that, `[tool.pyright]` — first-file-wins, no per-field merging.
//! `extra-paths` entries feed manual-path import resolution step 1 of
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561-MAPPING.
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
        "bsk_pyright_compat_{prefix}_{}_{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write the shared fixture: a `vendor/` directory holding `vendored_mod.py`
/// plus an `app.py` that imports and uses it. Only an `extra-paths` entry
/// pointing at `vendor` can make the import resolve.
fn write_vendored_fixture(dir: &Path) {
    std::fs::create_dir_all(dir.join("vendor")).expect("create vendor dir");
    std::fs::write(
        dir.join("vendor").join("vendored_mod.py"),
        "value: int = 1\n",
    )
    .expect("write vendored module");
    std::fs::write(
        dir.join("app.py"),
        "import vendored_mod\n\ntotal: int = vendored_mod.value\n",
    )
    .expect("write app");
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

/// Assert the vendored import resolved: exit 0 and no unresolved diagnostic.
fn assert_import_resolved(output: &Output, config_description: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("imports_unresolved"),
        "{config_description} must make `import vendored_mod` resolve via the \
         vendor/ extra path, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{config_description} must let the CLI check pass, stdout: {stdout}, \
         stderr: {stderr}"
    );
}

/// `pyrightconfig.json` with `extraPaths` is the highest-priority config file
/// ([#ANALYSIS-CONFIG-PRI] file tier, entry 1) and its entries feed manual
/// extra-path resolution ([#STUBRES-PEP561-MAPPING] step 1).
#[test]
fn pyrightconfig_extra_paths_resolve_vendored_import() {
    let dir = unique_dir("pyrightconfig_json");
    write_vendored_fixture(&dir);
    std::fs::write(
        dir.join("pyrightconfig.json"),
        "{ \"extraPaths\": [\"vendor\"] }\n",
    )
    .expect("write pyrightconfig.json");

    let output = check_app(&dir);
    assert_import_resolved(&output, "pyrightconfig.json `extraPaths`");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Negative control: with no config file at all, `vendor/` is not on any
/// search path, so the same import must fail with `imports_unresolved`
/// ([#STUBRES-PEP561-MAPPING]: a module matching no step is unresolved).
#[test]
fn missing_config_leaves_vendored_import_unresolved() {
    let dir = unique_dir("no_config");
    write_vendored_fixture(&dir);

    let output = check_app(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("imports_unresolved"),
        "without any config the vendored import must be reported unresolved, \
         stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stdout.contains("vendored_mod"),
        "the diagnostic must name the unresolved module, stdout: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "an unresolved import is an error, so the CLI must exit 1, \
         stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// With no `[tool.basilisk]` table, `pyproject.toml` `[tool.pyright]` is the
/// compatibility fallback ([#ANALYSIS-CONFIG-PRI] file tier, entry 2), and it
/// accepts pyright's camelCase `extraPaths` spelling.
#[test]
fn tool_pyright_extra_paths_fallback_resolves_import() {
    let dir = unique_dir("tool_pyright");
    write_vendored_fixture(&dir);
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.pyright]\nextraPaths = [\"vendor\"]\n",
    )
    .expect("write pyproject");

    let output = check_app(&dir);
    assert_import_resolved(&output, "the `[tool.pyright]` fallback table");

    let _ = std::fs::remove_dir_all(&dir);
}

/// `[tool.basilisk]` accepts pyright's camelCase `extraPaths` alias alongside
/// the native kebab-case spelling (`workspace_config_from_toml` in
/// crates/basilisk-lsp/src/config.rs).
#[test]
fn camel_case_extra_paths_in_tool_basilisk_resolve_import() {
    let dir = unique_dir("basilisk_camel");
    write_vendored_fixture(&dir);
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk]\nextraPaths = [\"vendor\"]\n",
    )
    .expect("write pyproject");

    let output = check_app(&dir);
    assert_import_resolved(&output, "`[tool.basilisk]` camelCase `extraPaths`");

    let _ = std::fs::remove_dir_all(&dir);
}

/// The native kebab-case spelling, `extra-paths`, in `[tool.basilisk]`.
#[test]
fn kebab_case_extra_paths_in_tool_basilisk_resolve_import() {
    let dir = unique_dir("basilisk_kebab");
    write_vendored_fixture(&dir);
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk]\nextra-paths = [\"vendor\"]\n",
    )
    .expect("write pyproject");

    let output = check_app(&dir);
    assert_import_resolved(&output, "`[tool.basilisk]` kebab-case `extra-paths`");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Whole-file precedence, not per-field merging ([#ANALYSIS-CONFIG-PRI]):
/// when `pyrightconfig.json` exists it supplies the ENTIRE workspace config,
/// so its `extraPaths` win even though a `[tool.basilisk]` table is present
/// with other keys and no extra paths of its own.
#[test]
fn pyrightconfig_json_takes_priority_over_pyproject() {
    let dir = unique_dir("priority");
    write_vendored_fixture(&dir);
    std::fs::write(
        dir.join("pyrightconfig.json"),
        "{ \"extraPaths\": [\"vendor\"] }\n",
    )
    .expect("write pyrightconfig.json");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[tool.basilisk]\npython-version = \"3.12\"\n",
    )
    .expect("write pyproject");

    let output = check_app(&dir);
    assert_import_resolved(
        &output,
        "pyrightconfig.json (priority over a `[tool.basilisk]` table lacking extra paths)",
    );

    let _ = std::fs::remove_dir_all(&dir);
}
