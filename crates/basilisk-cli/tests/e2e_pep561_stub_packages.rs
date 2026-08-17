//! End-to-end tests for PEP 561 stub-distribution semantics through the real
//! `basilisk check` binary ([STUBRES-PEP561], [STUBRES-PEP561-NORMATIVE],
//! [STUBRES-PEP561-MAPPING], [STUBRES-RESOLUTION-FLOW]).
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561-NORMATIVE
//!
//! The contract under test, exactly as the pinned typing specification's
//! "Distributing type information" chapter states it and exactly as a user
//! experiences it out of the box:
//!
//! - step 4: an installed `foopkg-stubs` distribution supersedes the inline
//!   `foopkg` install, and the `*-stubs` name alone is a source of typing
//!   information (no `py.typed` needed);
//! - a module miss in a COMPLETE stub distribution is terminal — resolution
//!   never falls through to the inline package;
//! - a stub distribution is partial only when its `py.typed` holds the exact
//!   line `partial` — any other content leaves it complete;
//! - a stub-only namespace package (no `__init__.pyi`) continues to step 5;
//! - step 5: an installed package resolves whether or not it ships `py.typed`.
//!
//! Resolution logic under test:
//! `crates/basilisk-checker/src/imports/resolve.rs`
//! (`try_resolve_stub_package`, `stub_package_miss_allows_fallback`,
//! `has_partial_marker`). The fixture venv (`.venv/lib/python3.12/
//! site-packages`) is discovered by `resolve_site_packages` in
//! `crates/basilisk-lsp/src/import_resolver.rs`; `VIRTUAL_ENV` is removed so
//! only the fixture's own venv is consulted.
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
    let counter = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bsk_pep561_stub_pkgs_{prefix}_{}_{counter}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Create the project skeleton — `pyproject.toml` plus a discoverable venv —
/// and return the venv's `site-packages` directory, the stage on which every
/// PEP 561 step-4/step-5 fixture is built.
fn write_project_with_site_packages(project_dir: &Path) -> PathBuf {
    std::fs::write(
        project_dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject");
    let site_packages = project_dir
        .join(".venv")
        .join("lib")
        .join("python3.12")
        .join("site-packages");
    std::fs::create_dir_all(&site_packages).expect("create site-packages");
    site_packages
}

/// Write `app.py` with the given source and run `basilisk check app.py`.
fn check_app(project_dir: &Path, app_source: &str) -> Output {
    std::fs::write(project_dir.join("app.py"), app_source).expect("write app");
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(project_dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk")
}

/// Write the shared step-4-vs-step-5 fixture: a `foo-stubs` distribution that
/// LACKS submodule `bar`, alongside an inline `foo` package that HAS
/// `bar.py` (and opts in via `py.typed`). Whether `import foo.bar` resolves
/// is then decided purely by the stub distribution's completeness:
///
/// - `stubs_marker`: content for `foo-stubs/py.typed`, or `None` for no marker
/// - `stubs_have_init`: whether `foo-stubs/__init__.pyi` exists (a regular
///   package) or not (a stub-only namespace package)
fn write_stubs_missing_submodule_fixture(
    site_packages: &Path,
    stubs_marker: Option<&str>,
    stubs_have_init: bool,
) {
    let stubs_dir = site_packages.join("foo-stubs");
    std::fs::create_dir_all(&stubs_dir).expect("create foo-stubs");
    if stubs_have_init {
        std::fs::write(stubs_dir.join("__init__.pyi"), "value: int\n").expect("write stubs init");
    } else {
        std::fs::write(stubs_dir.join("other.pyi"), "flag: bool\n")
            .expect("write namespace stub member");
    }
    if let Some(marker_contents) = stubs_marker {
        std::fs::write(stubs_dir.join("py.typed"), marker_contents).expect("write stubs marker");
    }

    let inline_dir = site_packages.join("foo");
    std::fs::create_dir_all(&inline_dir).expect("create inline foo");
    std::fs::write(inline_dir.join("__init__.py"), "").expect("write inline init");
    std::fs::write(inline_dir.join("bar.py"), "flag: bool = True\n").expect("write inline bar");
    std::fs::write(inline_dir.join("py.typed"), "").expect("write inline marker");
}

/// Assert the import resolved: no `imports_unresolved` diagnostic and exit 0.
fn assert_import_resolved(output: &Output, fixture_description: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains("imports_unresolved"),
        "{fixture_description} must resolve the import, stdout: {stdout}, \
         stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{fixture_description} must let the CLI check pass, stdout: {stdout}, \
         stderr: {stderr}"
    );
}

/// Assert the import missed: an `imports_unresolved` diagnostic naming the
/// module, and exit 1.
fn assert_import_unresolved(output: &Output, module_name: &str, fixture_description: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("imports_unresolved"),
        "{fixture_description} must report `imports_unresolved`, stdout: \
         {stdout}, stderr: {stderr}"
    );
    assert!(
        stdout.contains(module_name),
        "the diagnostic must name the unresolved module `{module_name}`, \
         stdout: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "{fixture_description} is a diagnostics failure, so the CLI must exit \
         1, stdout: {stdout}, stderr: {stderr}"
    );
}

/// A stub-only distribution is a source of typing information by its `-stubs`
/// name alone ([#STUBRES-PEP561-NORMATIVE]: "For stub-only packages adding a
/// `py.typed` marker is not needed"): `foo-stubs/__init__.pyi` with NO inline
/// `foo` package anywhere resolves `import foo` at step 4.
#[test]
fn stub_only_package_resolves_without_inline_package_or_marker() {
    let project_dir = unique_dir("stub_only");
    let site_packages = write_project_with_site_packages(&project_dir);
    let stubs_dir = site_packages.join("foo-stubs");
    std::fs::create_dir_all(&stubs_dir).expect("create foo-stubs");
    std::fs::write(stubs_dir.join("__init__.pyi"), "value: int\n").expect("write stubs init");

    let output = check_app(&project_dir, "import foo\n\nresult = foo.value\n");
    assert_import_resolved(
        &output,
        "a stub-only `foo-stubs` distribution with no inline `foo` install",
    );

    let _ = std::fs::remove_dir_all(&project_dir);
}

/// Negative control for every resolving fixture in this file: with an EMPTY
/// venv site-packages, `import foo` matches no resolution step, so it must
/// fail as `imports_unresolved` ([#STUBRES-PEP561-MAPPING]: a module matching
/// no step is unresolved). This proves the fixture venv — not some ambient
/// interpreter — is what the resolving tests exercise.
#[test]
fn empty_site_packages_leaves_import_unresolved() {
    let project_dir = unique_dir("empty_venv");
    let _site_packages = write_project_with_site_packages(&project_dir);

    let output = check_app(&project_dir, "import foo\n");
    assert_import_unresolved(&output, "foo", "an empty fixture site-packages");

    let _ = std::fs::remove_dir_all(&project_dir);
}

/// A COMPLETE stub distribution (regular package, no partial marker) is
/// terminal on a miss ([#STUBRES-PEP561-MAPPING]: "A complete step-4 package
/// stops on a miss"): `foo-stubs` lacks submodule `bar`, so `import foo.bar`
/// must be unresolved even though inline `foo/bar.py` exists with `py.typed`.
///
/// This is simultaneously the step-4 priority proof ("Stub packages - these
/// packages SHOULD supersede any installed inline package"): the diagnostic
/// can only fire if the `-stubs` distribution was consulted BEFORE the inline
/// package — a merged or inline-first search would have found `foo/bar.py`.
#[test]
fn complete_stub_package_miss_is_terminal_despite_inline_submodule() {
    let project_dir = unique_dir("complete_terminal");
    let site_packages = write_project_with_site_packages(&project_dir);
    write_stubs_missing_submodule_fixture(&site_packages, None, true);

    let output = check_app(&project_dir, "import foo.bar\n");
    assert_import_unresolved(
        &output,
        "foo.bar",
        "a submodule miss in a complete `foo-stubs` distribution",
    );

    let _ = std::fs::remove_dir_all(&project_dir);
}

/// A stub distribution whose `py.typed` holds the exact line `partial` is
/// partial ([#STUBRES-PEP561-NORMATIVE]: "If a stub package distribution is
/// partial it MUST include `partial\n` in a `py.typed` file"): the same
/// submodule miss as the terminal test now continues to steps 5–6 and
/// resolves `import foo.bar` through inline `foo/bar.py`.
#[test]
fn partial_marker_continues_submodule_miss_to_inline_package() {
    let project_dir = unique_dir("partial_continues");
    let site_packages = write_project_with_site_packages(&project_dir);
    write_stubs_missing_submodule_fixture(&site_packages, Some("partial\n"), true);

    let output = check_app(&project_dir, "import foo.bar\n");
    assert_import_resolved(
        &output,
        "a submodule miss in a `partial\\n`-marked `foo-stubs` distribution",
    );

    let _ = std::fs::remove_dir_all(&project_dir);
}

/// The partial marker is the exact line `partial` — nothing looser. A
/// `py.typed` containing `partially\n` does NOT mark the distribution
/// partial, so the same miss stays terminal exactly like the marker-free
/// complete distribution.
#[test]
fn inexact_partial_marker_word_leaves_distribution_complete() {
    let project_dir = unique_dir("inexact_marker");
    let site_packages = write_project_with_site_packages(&project_dir);
    write_stubs_missing_submodule_fixture(&site_packages, Some("partially\n"), true);

    let output = check_app(&project_dir, "import foo.bar\n");
    assert_import_unresolved(
        &output,
        "foo.bar",
        "a `py.typed` reading `partially` (not the exact `partial` line)",
    );

    let _ = std::fs::remove_dir_all(&project_dir);
}

/// A stub-only NAMESPACE package — identified by the absence of
/// `__init__.pyi` ([#STUBRES-PEP561-NORMATIVE]: "Typecheckers should identify
/// namespace packages by the absence of `__init__.pyi`") — is never terminal:
/// the miss continues to step 5 and `import foo.bar` resolves through the
/// inline package, with no partial marker required.
#[test]
fn namespace_stub_package_miss_continues_to_inline_package() {
    let project_dir = unique_dir("namespace_continues");
    let site_packages = write_project_with_site_packages(&project_dir);
    write_stubs_missing_submodule_fixture(&site_packages, None, false);

    let output = check_app(&project_dir, "import foo.bar\n");
    assert_import_resolved(
        &output,
        "a submodule miss in a stub-only namespace package (no __init__.pyi)",
    );

    let _ = std::fs::remove_dir_all(&project_dir);
}

/// Step 5 resolves an installed package with NO `py.typed` and NO stub
/// distribution: `py.typed` controls downstream provenance classification,
/// not existence ([#STUBRES-RESOLUTION-FLOW]: an untyped `.py` hit is the
/// terminal `UntypedImport` resolution, not `imports_unresolved`), and the
/// out-of-the-box configuration emits no diagnostic for using it.
#[test]
fn untyped_installed_package_resolves_without_diagnostics() {
    let project_dir = unique_dir("untyped_inline");
    let site_packages = write_project_with_site_packages(&project_dir);
    let inline_dir = site_packages.join("foo");
    std::fs::create_dir_all(&inline_dir).expect("create inline foo");
    std::fs::write(inline_dir.join("__init__.py"), "value: int = 1\n").expect("write inline init");

    let output = check_app(&project_dir, "import foo\n\nresult = foo.value\n");
    assert_import_resolved(
        &output,
        "an installed untyped package (no py.typed, no stub distribution)",
    );

    let _ = std::fs::remove_dir_all(&project_dir);
}
