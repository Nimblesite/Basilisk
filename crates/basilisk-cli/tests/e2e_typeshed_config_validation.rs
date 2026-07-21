//! End-to-end tests for typeshed configuration validation
//! ([STUBRES-TYPESHED-CONFIG]).
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CONFIG
//!
//! The contract under test, exactly as a user experiences it through the
//! real binary: every invalid typeshed setting FAILS CLOSED. The CLI exits
//! with the distinct configuration-error code 2 (never the diagnostics
//! code 1, never a silent 0), prints a redacted, user-facing reason on
//! stderr, and emits no diagnostics at all — a broken pin must never
//! silently substitute another source ([STUBRES-TYPESHED-OFFLINE]).
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

/// [STUBRES-TYPESHED-PIN] / [TYPESHEDRT-SEGREGATION], through the real
/// binary: a well-formed pin that is not on this machine is VALID
/// configuration — the check TANKS HARD (exit 3, the spec's `NO SOURCE`
/// status line naming the recovery command, zero diagnostics) and the
/// checker NEVER attempts to download anything: the isolated store is left
/// byte-empty.
#[test]
fn a_valid_missing_pin_tanks_hard_and_never_downloads() {
    let dir = unique_dir("missing_pin");
    let pin = "0123456789012345678901234567890123456789";
    let output = check_with_config(
        &dir,
        &format!("typeshed-commit = \"{pin}\"\ntypeshed-store-path = \"store\"\n"),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(3),
        "a missing pin is a hard failure, not a config error and never a pass, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stderr.contains("NO SOURCE") && stderr.contains(pin),
        "stderr must carry the loud NO SOURCE line naming the pin, stderr: {stderr}"
    );
    assert!(
        stderr.contains("basilisk typeshed download"),
        "stderr must name the explicit recovery command, stderr: {stderr}"
    );
    assert!(
        !stdout.contains("error["),
        "no diagnostics may be emitted when the source is missing, stdout: {stdout}"
    );
    let store = dir.join("store");
    let store_is_untouched = !store.exists()
        || std::fs::read_dir(&store).is_ok_and(|mut entries| entries.next().is_none());
    assert!(
        store_is_untouched,
        "the checker must never write to or fetch into the store"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The retired download-policy keys (`typeshed-url`, `typeshed-cache`,
/// `typeshed-verify`, `typeshed-cache-path`) are no longer configuration:
/// they change nothing, trigger no download machinery, and their values are
/// never echoed back ([STUBRES-TYPESHED-CONFIG] redaction).
#[test]
fn retired_download_policy_keys_are_inert_and_never_echoed() {
    let dir = unique_dir("retired_keys");
    let output = check_with_config(
        &dir,
        "typeshed-url = \"http://internal-host.corp.example/secret-{sha}.zip\"\ntypeshed-cache = false\ntypeshed-verify = false\ntyped-cache-path = \"secret-cache\"\n",
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "retired keys must not alter the bundled-default check, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        !stderr.contains("internal-host.corp.example")
            && !stdout.contains("internal-host.corp.example"),
        "a retired mirror value must never be echoed back, stderr: {stderr}"
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

/// A well-formed pin clears configuration validation: whatever happens next
/// is source resolution (`NO SOURCE`), never a configuration error.
#[test]
fn valid_full_sha_clears_validation_and_fails_only_on_resolution() {
    let dir = unique_dir("valid_pin_shape");
    let output = check_with_config(
        &dir,
        "typeshed-commit = \"0123456789012345678901234567890123456789\"\ntypeshed-store-path = \"store\"\n",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stderr.contains("configuration error"),
        "a well-formed pin must clear validation — any failure past this point is resolution, not configuration, stderr: {stderr}"
    );
    assert_ne!(
        output.status.code(),
        Some(0),
        "a pin that is not on this machine must not silently pass ([STUBRES-TYPESHED-PIN] fail-closed), stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
