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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use basilisk_config::{
    ConfigurationUpdate, RuleConfigUpdate, TypeshedConfigKey, TypeshedConfigUpdate,
};

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
    run_check(dir)
}

/// Run `basilisk check app.py` in `dir` against whatever configuration is on
/// disk, with the ambient venv scrubbed.
fn run_check(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk")
}

/// The typeshed half of a configuration update, as both pin-writing surfaces
/// build it.
fn typeshed_update(entries: &[(TypeshedConfigKey, Option<&str>)]) -> ConfigurationUpdate {
    ConfigurationUpdate {
        rules: RuleConfigUpdate::default(),
        typeshed: TypeshedConfigUpdate {
            entries: entries
                .iter()
                .map(|(key, setting)| (*key, setting.map(str::to_owned)))
                .collect::<BTreeMap<_, _>>(),
        },
    }
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
        "typeshed-url = \"http://internal-host.corp.example/secret-{sha}.zip\"\ntypeshed-cache = false\ntypeshed-verify = false\ntypeshed-cache-path = \"secret-cache\"\n",
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "retired keys must not alter the bundled-default check, stdout: {stdout}, stderr: {stderr}"
    );
    for retired_value in ["internal-host.corp.example", "secret-cache"] {
        assert!(
            !stderr.contains(retired_value) && !stdout.contains(retired_value),
            "a retired key's value must never be echoed back: `{retired_value}`, stdout: {stdout}, stderr: {stderr}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// [STUBRES-TYPESHED-DOWNLOAD] agreement between the two pin-writing
/// surfaces: `basilisk typeshed download` (`write_pin` in
/// `crates/basilisk-cli/src/typeshed_cli.rs`) and the LSP's Download latest
/// button (`pin_update` in `crates/basilisk-lsp/src/typeshed_download.rs`)
/// build the SAME two-entry update — set `typeshed-commit`, retire
/// `typeshed-path` — because the two step-3 sources are mutually exclusive
/// ([STUBRES-TYPESHED]). This drives that shared `basilisk-config`
/// transaction over a workspace that names a custom folder and hands the
/// result to the real binary. The commit-only half-update, which would leave
/// both sources named, is refused wholesale — so a surface that skipped the
/// retirement would download bytes and then fail to pin them.
#[test]
fn pinning_a_downloaded_commit_retires_the_custom_folder() {
    let pin = "0123456789012345678901234567890123456789";
    let dir = unique_dir("pin_retires_path");
    std::fs::create_dir_all(dir.join("ts").join("stdlib")).expect("create custom tree");
    std::fs::write(dir.join("app.py"), "value: int = 1\n").expect("write app");
    std::fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ntypeshed-path = \"ts\"\ntypeshed-store-path = \"store\"\n",
    )
    .expect("write pyproject");

    let document = basilisk_config::discover_config_document(&dir).expect("discover config");
    let commit_only = typeshed_update(&[(TypeshedConfigKey::TypeshedCommit, Some(pin))]);
    assert!(
        basilisk_config::build_configuration_patch(&document, &commit_only).is_err(),
        "a pin write that keeps the custom folder names two sources at once and must be refused"
    );

    let patch = basilisk_config::build_configuration_patch(
        &document,
        &typeshed_update(&[
            (TypeshedConfigKey::TypeshedCommit, Some(pin)),
            (TypeshedConfigKey::TypeshedPath, None),
        ]),
    )
    .expect("the retiring pin write is a valid transaction");
    basilisk_config::apply_config_patch(&patch).expect("apply the pin write");

    let written = std::fs::read_to_string(dir.join("pyproject.toml")).expect("read pyproject");
    assert!(
        written.contains(&format!("typeshed-commit = \"{pin}\"")),
        "the resolved pin must be written: {written}"
    );
    assert!(
        !written.contains("typeshed-path"),
        "the custom folder must be retired by the same write: {written}"
    );
    assert!(
        written.contains("typeshed-store-path = \"store\""),
        "unrelated typeshed settings must survive untouched: {written}"
    );

    // The real binary reads exactly one source out of the written file: the
    // pin. It fails on resolution (`NO SOURCE`), never on configuration.
    let output = run_check(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("mutually exclusive"),
        "the written configuration must name a single source, stderr: {stderr}"
    );
    assert_eq!(
        output.status.code(),
        Some(3),
        "the lone pin is valid configuration whose bytes are absent, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stderr.contains("NO SOURCE") && stderr.contains(pin),
        "stderr must carry the NO SOURCE line naming the freshly written pin, stderr: {stderr}"
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
