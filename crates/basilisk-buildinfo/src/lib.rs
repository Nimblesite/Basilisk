//! Implements [CHKARCH-ARCH-BUILD-VERSIONINFO].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-BUILD-VERSIONINFO
//!
//! Shared build-script logic for the Shipwright `--version` contract.
//!
//! Every binary crate that ships a `--version` payload (`basilisk-cli`,
//! `basilisk-profiler-helper`) emits the same `SHIPWRIGHT_*` env vars the
//! `shipwright` crate reads at compile time. Rather than copy a `build.rs`
//! per crate, each build script is a one-liner delegating to
//! [`emit_version_env`]. The calendar arithmetic lives in
//! [`basilisk_common::datetime`] so it is not duplicated either.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Emit the `SHIPWRIGHT_*` build-metadata env vars for the calling build script.
///
/// Mirrors `build_info.rs.example` from the upstream `shipwright` crate, with a
/// guaranteed `SHIPWRIGHT_GIT_DIRTY` (the version-contract tests assert
/// `gitDirty` is always present in the JSON payload). Call this from a crate's
/// `build.rs` `main`.
pub fn emit_version_env() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=SHIPWRIGHT_GIT_SHA");

    let sha = env_or_cmd(
        "SHIPWRIGHT_GIT_SHA",
        "git",
        &["rev-parse", "--short=10", "HEAD"],
    )
    .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=SHIPWRIGHT_GIT_SHA={sha}");

    // Tests require gitDirty to be present in JSON output, so always emit
    // a definitive value. Default to "false" when git is unreachable
    // (cross-compile environments without .git).
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .is_ok_and(|o| !o.stdout.is_empty());
    println!(
        "cargo:rustc-env=SHIPWRIGHT_GIT_DIRTY={}",
        if dirty { "true" } else { "false" }
    );

    println!(
        "cargo:rustc-env=SHIPWRIGHT_BUILD_TIME={}",
        rfc3339_now_or_source_date_epoch()
    );

    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=SHIPWRIGHT_TARGET={target}");
    }

    let toolchain = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_default();
    println!("cargo:rustc-env=SHIPWRIGHT_TOOLCHAIN={toolchain}");
}

fn env_or_cmd(env_key: &str, command: &str, args: &[&str]) -> Option<String> {
    if let Ok(value) = std::env::var(env_key) {
        if !value.is_empty() {
            return Some(value);
        }
    }
    let output = Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn rfc3339_now_or_source_date_epoch() -> String {
    let secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        });
    basilisk_common::datetime::rfc3339_from_secs(secs)
}
