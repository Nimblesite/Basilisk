//! Tests default bundled typeshed behavior for [STUBRES-CUSTOM-TYPESHED].
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
        "bsk_bundled_typeshed_{prefix}_{}_{n}",
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
fn cli_without_typeshed_path_uses_the_bundled_snapshot() {
    let dir = unique_dir("bundled_snapshot");
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
        "the bundled snapshot must suppress unresolved diagnostics, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        !stdout.contains("fractions"),
        "diagnostics must not name a bundled stdlib module, stdout: {stdout}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "the bundled snapshot must let the CLI check pass, stdout: {stdout}, stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
