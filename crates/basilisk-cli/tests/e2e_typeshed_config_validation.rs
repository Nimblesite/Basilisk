//! End-to-end tests for typeshed configuration validation
//! ([STUBRES-TYPESHED-CONFIG]).
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CONFIG
//!
//! The contract under test, exactly as a user experiences it through the
//! real binary: every invalid typeshed setting FAILS CLOSED. The CLI exits
//! with the distinct configuration-error code 2 (never the diagnostics
//! code 1, never a silent 0), prints a redacted, user-facing reason on
//! stderr, and emits no diagnostics at all — a broken pin must never
//! silently substitute another source ([STUBRES-TYPESHED-ACQUIRE]).
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
        "bsk_typeshed_validation_{prefix}_{}_{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write a one-file project whose only variable is the `[tool.basilisk]`
/// typeshed table, then run `basilisk check app.py` in it.
fn check_with_config(dir: &Path, basilisk_table: &str) -> Output {
    std::fs::write(
        dir.join("pyproject.toml"),
        format!(
            "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\n{basilisk_table}"
        ),
    )
    .expect("write pyproject");
    std::fs::write(dir.join("app.py"), "value: int = 1\n").expect("write app");
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk")
}

/// Assert the fail-closed contract shared by every invalid typeshed setting:
/// exit code 2, the exact reason on stderr, and zero diagnostics on stdout.
fn assert_fails_closed(output: &Output, expected_reason: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(2),
        "an invalid typeshed setting is a configuration error (exit 2), not a diagnostics failure, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stderr.contains(expected_reason),
        "stderr must carry the user-facing reason `{expected_reason}`, stderr: {stderr}"
    );
    assert!(
        !stdout.contains("error["),
        "a config error must produce no diagnostics — the check never ran, stdout: {stdout}"
    );
}

#[test]
fn short_typeshed_commit_fails_closed() {
    let dir = unique_dir("short_commit");
    let output = check_with_config(&dir, "typeshed-commit = \"abc123\"\n");
    assert_fails_closed(
        &output,
        "typeshed-commit must be a full 40-character hex SHA",
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn full_length_non_hex_typeshed_commit_fails_closed() {
    let dir = unique_dir("non_hex_commit");
    let output = check_with_config(
        &dir,
        "typeshed-commit = \"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz\"\n",
    );
    assert_fails_closed(
        &output,
        "typeshed-commit must be a full 40-character hex SHA",
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn typeshed_url_without_sha_placeholder_fails_closed() {
    let dir = unique_dir("url_no_sha");
    let output = check_with_config(
        &dir,
        "typeshed-url = \"https://mirror.example.com/typeshed.zip\"\n",
    );
    assert_fails_closed(
        &output,
        "typeshed-url must be HTTPS with exactly one {sha} placeholder",
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn plain_http_typeshed_url_fails_closed() {
    let dir = unique_dir("url_http");
    let output = check_with_config(
        &dir,
        "typeshed-url = \"http://mirror.example.com/{sha}.zip\"\n",
    );
    assert_fails_closed(
        &output,
        "typeshed-url must be HTTPS with exactly one {sha} placeholder",
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn typeshed_url_with_two_sha_placeholders_fails_closed() {
    let dir = unique_dir("url_two_sha");
    let output = check_with_config(
        &dir,
        "typeshed-url = \"https://mirror.example.com/{sha}/{sha}.zip\"\n",
    );
    assert_fails_closed(
        &output,
        "typeshed-url must be HTTPS with exactly one {sha} placeholder",
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn typeshed_path_with_commit_pin_fails_closed() {
    let dir = unique_dir("path_commit_conflict");
    std::fs::create_dir_all(dir.join("ts/stdlib")).expect("create custom tree");
    let output = check_with_config(
        &dir,
        "typeshed-path = \"ts\"\ntypeshed-commit = \"0123456789012345678901234567890123456789\"\n",
    );
    assert_fails_closed(
        &output,
        "typeshed-path and typeshed-commit are mutually exclusive",
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_error_reason_never_echoes_the_configured_mirror_value() {
    let dir = unique_dir("redacted_mirror");
    let secret = "http://internal-host.corp.example/secret-{sha}.zip";
    let output = check_with_config(&dir, &format!("typeshed-url = \"{secret}\"\n"));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(2),
        "the invalid mirror must fail closed, stderr: {stderr}"
    );
    assert!(
        !stderr.contains("internal-host.corp.example"),
        "the configured mirror value must never be echoed back ([STUBRES-TYPESHED-CONFIG] redaction), stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn valid_full_sha_paired_with_valid_mirror_passes_validation() {
    let dir = unique_dir("valid_pin_shape");
    // An unreachable-but-well-formed mirror: validation must ACCEPT the
    // shape; the later transport failure is a different, non-config error.
    let output = check_with_config(
        &dir,
        "typeshed-commit = \"0123456789012345678901234567890123456789\"\ntypeshed-url = \"https://127.0.0.1:1/{sha}.zip\"\ntypeshed-cache = false\n",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stderr.contains("configuration error"),
        "a well-formed pin + mirror must clear validation — any failure past this point is transport, not configuration, stderr: {stderr}"
    );
    assert_ne!(
        output.status.code(),
        Some(0),
        "an unreachable mirror with an exact pin must not silently pass ([STUBRES-TYPESHED-ACQUIRE] fail-closed), stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
