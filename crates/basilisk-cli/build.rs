//! Build metadata for the Shipwright `--version` contract.
//!
//! Emits the `SHIPWRIGHT_*` env vars the `shipwright` crate reads at
//! compile time. Mirrors `build_info.rs.example` from the upstream crate,
//! with a guaranteed `SHIPWRIGHT_GIT_DIRTY` (the version-contract tests
//! assert `gitDirty` is always present in the JSON payload).

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SECS_PER_DAY: u64 = 86_400;
const SECS_PER_HOUR: u64 = 3_600;
const SECS_PER_MINUTE: u64 = 60;

fn main() {
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
    rfc3339_from_secs(secs)
}

fn rfc3339_from_secs(secs: u64) -> String {
    let days = secs / SECS_PER_DAY;
    let seconds_of_day = secs % SECS_PER_DAY;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / SECS_PER_HOUR;
    let minute = seconds_of_day % SECS_PER_HOUR / SECS_PER_MINUTE;
    let second = seconds_of_day % SECS_PER_MINUTE;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

// Howard Hinnant's civil_from_days (public domain).
#[expect(
    clippy::arithmetic_side_effects,
    reason = "epoch arithmetic on u64 day counts cannot overflow within representable timestamps"
)]
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let adjusted = days + 719_468;
    let era = adjusted / 146_097;
    let day_of_era = adjusted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}
