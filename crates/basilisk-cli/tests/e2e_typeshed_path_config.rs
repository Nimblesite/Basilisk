//! Tests for [STUBRES-CUSTOM-TYPESHED].
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
        "bsk_typeshed_path_{prefix}_{}_{n}",
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
fn cli_uses_typeshed_path_and_absent_stdlib_falls_through() {
    let dir = unique_dir("fake_stdlib");
    let typeshed = dir.join("fake-typeshed");
    let stdlib = typeshed.join("stdlib");
    std::fs::create_dir_all(&stdlib).expect("create fake stdlib");
    std::fs::write(
        stdlib.join("os.pyi"),
        "def uname() -> str: ...\n",
    )
    .expect("write fake os stub");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ntypeshed-path = \"fake-typeshed\"\n",
    )
    .expect("write pyproject");
    std::fs::write(
        dir.join("app.py"),
        "from os import uname\nfrom fractions import Fraction\n\nsystem_name: str = uname()\nmissing = Fraction(1, 2)\n",
    )
    .expect("write app");

    let output = check_app(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("imports_unresolved"),
        "absent stdlib module must fall through to imports_unresolved, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stdout.contains("fractions"),
        "diagnostic must name the stdlib module absent from custom typeshed, stdout: {stdout}"
    );
    assert!(
        !stdout.contains("`os`") && !stdout.contains("`uname`"),
        "custom typeshed os.pyi must resolve the uname import without diagnostics, stdout: {stdout}"
    );
    assert_ne!(
        output.status.code(),
        Some(0),
        "absent stdlib module must fail the CLI check"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
