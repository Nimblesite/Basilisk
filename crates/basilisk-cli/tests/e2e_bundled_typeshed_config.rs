//! Tests default runtime typeshed behavior for [STUBRES-CUSTOM-TYPESHED].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED
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
        "bsk_runtime_typeshed_{prefix}_{}_{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
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

#[test]
fn cli_without_typeshed_path_activates_the_default_runtime_source() {
    let dir = unique_dir("default_source");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\n",
    )
    .expect("write pyproject");
    std::fs::write(
        dir.join("app.py"),
        "from fractions import Fraction\n\nvalue = Fraction(1, 2)\n",
    )
    .expect("write app");

    let output = check_app(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains("imports_unresolved"),
        "the default runtime source must suppress unresolved diagnostics, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        !stdout.contains("fractions"),
        "diagnostics must not name a resolved stdlib module, stdout: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the default runtime source must let the CLI check pass, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Regression for GitHub #330: a member the ACTIVE Typeshed stub does not
/// declare must be reported on a plain-imported stdlib module.
///
/// `imports_module_attribute` documents plain imports backed by an
/// authoritative local stub **or the active step-3 Typeshed source** as in
/// scope, and the LSP's cross-module query populates both. The CLI's
/// single-file pipeline captured only user-stub module APIs, so the Typeshed
/// half was silently dropped and `basilisk check` exited 0 on a call that
/// cannot exist.
#[test]
fn cli_flags_a_member_the_active_typeshed_stub_does_not_declare() {
    let dir = unique_dir("typeshed_module_attribute");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\n",
    )
    .expect("write pyproject");
    std::fs::write(
        dir.join("app.py"),
        "import json\n\npayload = json.parse_body(\"{}\")\n",
    )
    .expect("write app");

    let output = check_app(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("imports_module_attribute"),
        "the active Typeshed `json` stub declares no `parse_body`, so the CLI must report it, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stdout.contains("parse_body"),
        "the diagnostic must name the missing member, stdout: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "a missing Typeshed member is an error, so the CLI must exit 1, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The other half of GitHub #330, and the guard against re-introducing #312:
/// members the active Typeshed stub DOES declare — including names it
/// re-exports rather than defines — must never be flagged. Capturing the
/// Typeshed module API is only correct if it captures the module's full export
/// set; a partial capture turns every valid re-export into a false positive.
#[test]
fn cli_accepts_members_the_active_typeshed_stub_declares_or_reexports() {
    let dir = unique_dir("typeshed_module_attribute_valid");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\n",
    )
    .expect("write pyproject");
    std::fs::write(
        dir.join("app.py"),
        "import json\n\ntext = json.dumps({})\nvalue = json.loads(text)\ndecoder = json.JSONDecoder\nerror = json.JSONDecodeError\n",
    )
    .expect("write app");

    let output = check_app(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stdout.contains("imports_module_attribute"),
        "`dumps`/`loads`/`JSONDecoder` are declared and `JSONDecodeError` is re-exported by the active `json` stub — none may be flagged, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
