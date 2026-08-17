//! Tests for [STUBRES-TYPESHED-VERSION].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-VERSION
//!
//! Platform target evidence justifies an interpreter launch ONLY for an
//! EXPLICITLY selected interpreter (`python-interpreter` config or
//! `BASILISK_PYTHON`), which can deliberately point at a shim reporting a
//! non-host target. Auto-discovered interpreters — a workspace venv, a bare
//! `python3` on `PATH` — execute on this host by definition, so their
//! `sys.platform` answer is always the host constant. Launching one anyway
//! costs a full interpreter start-up on EVERY `check`, so auto-discovery must
//! resolve the platform without spawning anything.
//!
//! Covers `load_cli_workspace_config` in
//! `crates/basilisk-cli/src/pipeline/typeshed.rs`.
#![cfg(unix)]
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bsk_no_python_spawn_{prefix}_{}_{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write an executable interpreter shim named `name` that records every
/// invocation in `sentinel` and otherwise answers exactly like a real
/// interpreter probe.
///
/// The shim ANSWERS correctly on purpose: the test must fail because the
/// interpreter was launched, never because the launch produced a bad value.
fn install_interpreter_shim(bin_dir: &Path, name: &str, sentinel: &Path) {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::create_dir_all(bin_dir).expect("create shim dir");
    let shim = bin_dir.join(name);
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\necho invoked >> '{}'\necho darwin\n",
            sentinel.display()
        ),
    )
    .expect("write interpreter shim");
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755))
        .expect("make shim executable");
}

fn install_python_shim(bin_dir: &Path, sentinel: &Path) {
    install_interpreter_shim(bin_dir, "python3", sentinel);
}

/// `PATH` with `bin_dir` prepended, so shims win over real interpreters.
fn path_with(bin_dir: &Path) -> String {
    format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// A project that selects no interpreter must be checked without launching
/// one. [STUBRES-TYPESHED-VERSION]
#[test]
fn check_without_a_selected_interpreter_never_spawns_python() {
    let dir = unique_dir("unselected");
    let bin_dir = dir.join("fakebin");
    let sentinel = dir.join("python-was-spawned");
    install_python_shim(&bin_dir, &sentinel);

    std::fs::write(dir.join("app.py"), "x: int = 1\n").expect("write app.py");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\n",
    )
    .expect("write pyproject.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(&dir)
        .env("PATH", path_with(&bin_dir))
        .env_remove("BASILISK_PYTHON")
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk");

    assert!(
        output.status.success(),
        "check must succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !sentinel.exists(),
        "basilisk spawned a Python interpreter for a project that selects none — \
         the platform is knowable from the host without paying an interpreter start-up \
         on every check (sentinel recorded: {})",
        std::fs::read_to_string(&sentinel)
            .unwrap_or_default()
            .trim()
    );
}

/// An auto-discovered workspace venv is a host binary: its `sys.platform`
/// answer is always the host constant, so discovering one must not trigger an
/// interpreter launch either. [STUBRES-TYPESHED-VERSION]
#[test]
fn check_with_only_a_workspace_venv_never_spawns_python() {
    let dir = unique_dir("venv");
    let venv_bin = dir.join(".venv").join("bin");
    let sentinel = dir.join("python-was-spawned");
    install_python_shim(&venv_bin, &sentinel);
    // `resolve_python` discovers `.venv/bin/python`; alias the shim to it.
    let _bytes = std::fs::copy(venv_bin.join("python3"), venv_bin.join("python"))
        .expect("alias venv python");

    std::fs::write(dir.join("app.py"), "x: int = 1\n").expect("write app.py");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\n",
    )
    .expect("write pyproject.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(&dir)
        .env_remove("BASILISK_PYTHON")
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk");

    assert!(
        output.status.success(),
        "check must succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !sentinel.exists(),
        "basilisk launched the auto-discovered venv interpreter — a venv binary runs \
         on this host, so its sys.platform is the host constant and the launch buys \
         nothing (sentinel recorded: {})",
        std::fs::read_to_string(&sentinel)
            .unwrap_or_default()
            .trim()
    );
}

/// The saving must not cost accuracy: an EXPLICITLY selected interpreter is
/// still interrogated, because its platform can differ from the host.
/// [STUBRES-TYPESHED-VERSION]
#[test]
fn check_with_an_explicitly_selected_interpreter_still_probes_it() {
    let dir = unique_dir("selected");
    let bin_dir = dir.join("fakebin");
    let sentinel = dir.join("python-was-spawned");
    install_python_shim(&bin_dir, &sentinel);

    std::fs::write(dir.join("app.py"), "x: int = 1\n").expect("write app.py");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\n",
    )
    .expect("write pyproject.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(&dir)
        .env("BASILISK_PYTHON", bin_dir.join("python3"))
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk");

    assert!(
        output.status.success(),
        "check must succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        sentinel.exists(),
        "an explicitly selected interpreter must still be interrogated for its platform"
    );
}

/// Version evidence with no venv resolves third-party imports against the
/// named `python3.X` interpreter's `site-packages`. For a conventional layout
/// (`<prefix>/bin/python3.X` + `<prefix>/lib/python3.X/site-packages`) that
/// directory is encoded in the filesystem — recovering it must not launch the
/// interpreter. [ANALYSIS-CROSSLSP-IMPORT]
#[test]
fn check_resolves_conventional_site_packages_without_spawning_the_versioned_interpreter() {
    let dir = unique_dir("conventional_prefix");
    let prefix = dir.join("prefix");
    let sentinel = dir.join("python-was-spawned");
    install_interpreter_shim(&prefix.join("bin"), "python3.12", &sentinel);
    let package = prefix
        .join("lib")
        .join("python3.12")
        .join("site-packages")
        .join("mypkg");
    std::fs::create_dir_all(&package).expect("create site-packages package");
    std::fs::write(package.join("__init__.py"), "value: int = 1\n").expect("write package");
    std::fs::write(package.join("py.typed"), "").expect("write py.typed");

    std::fs::write(
        dir.join("app.py"),
        "import mypkg\n\nnumber: int = mypkg.value\n",
    )
    .expect("write app.py");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\nrequires-python = \">=3.12\"\n",
    )
    .expect("write pyproject.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(&dir)
        .env("PATH", path_with(&prefix.join("bin")))
        .env_remove("BASILISK_PYTHON")
        .env_remove("VIRTUAL_ENV")
        .env_remove("PYTHONPATH")
        .output()
        .expect("spawn basilisk");

    assert!(
        !sentinel.exists(),
        "basilisk launched `python3.12` to find site-packages that a conventional \
         `<prefix>/lib/python3.12/site-packages` layout already encodes (sentinel \
         recorded: {})",
        std::fs::read_to_string(&sentinel)
            .unwrap_or_default()
            .trim()
    );
    assert!(
        output.status.success(),
        "the direct layout inspection must actually resolve `mypkg`; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The saving must not cost coverage of custom layouts: an interpreter whose
/// installation does NOT follow the `bin/` + `lib/` convention is still probed
/// for its real `sys.path`. [ANALYSIS-CROSSLSP-IMPORT]
#[test]
fn check_with_a_custom_interpreter_layout_still_probes_sys_path() {
    let dir = unique_dir("custom_prefix");
    let flat = dir.join("flat");
    let sentinel = dir.join("python-was-spawned");
    install_interpreter_shim(&flat, "python3.12", &sentinel);

    std::fs::write(dir.join("app.py"), "import mypkg\n").expect("write app.py");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\nrequires-python = \">=3.12\"\n",
    )
    .expect("write pyproject.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(&dir)
        .env("PATH", path_with(&flat))
        .env_remove("BASILISK_PYTHON")
        .env_remove("VIRTUAL_ENV")
        .env_remove("PYTHONPATH")
        .output()
        .expect("spawn basilisk");

    assert!(
        output.status.code().is_some(),
        "check must run to completion; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        sentinel.exists(),
        "a custom-layout interpreter has no conventional site-packages to inspect — \
         the sys.path probe remains the authoritative fallback"
    );
}
