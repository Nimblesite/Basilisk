//! Build metadata stamping for Shipwright version output.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SECS_PER_DAY: i64 = 86_400;
const SECS_PER_HOUR: i64 = 3_600;
const SECS_PER_MINUTE: i64 = 60;

fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    println!("cargo:rustc-env=BASILISK_BUILD_TIME={}", build_time());
    println!("cargo:rustc-env=BASILISK_TARGET={}", build_target());
    println!("cargo:rustc-env=BASILISK_RUSTC_VERSION={}", rustc_version());

    if let Some(sha) = git_sha() {
        println!("cargo:rustc-env=BASILISK_GIT_SHA={sha}");
    }
    println!("cargo:rustc-env=BASILISK_GIT_DIRTY={}", git_dirty());
}

fn build_time() -> String {
    let seconds = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_else(current_unix_time);
    format_unix_time(seconds)
}

fn current_unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| i64::try_from(duration.as_secs()).unwrap_or(0))
}

fn format_unix_time(seconds: i64) -> String {
    let days = seconds.div_euclid(SECS_PER_DAY);
    let seconds_of_day = seconds.rem_euclid(SECS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / SECS_PER_HOUR;
    let minute = seconds_of_day % SECS_PER_HOUR / SECS_PER_MINUTE;
    let second = seconds_of_day % SECS_PER_MINUTE;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let adjusted = days + 719_468;
    let era = if adjusted >= 0 {
        adjusted
    } else {
        adjusted - 146_096
    } / 146_097;
    let day_of_era = adjusted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

fn rustc_version() -> String {
    command_stdout("rustc", &["--version"]).unwrap_or_else(|| "rustc unknown".to_owned())
}

fn build_target() -> String {
    std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned())
}

fn git_sha() -> Option<String> {
    command_stdout("git", &["rev-parse", "HEAD"]).map(|sha| sha.chars().take(40).collect())
}

fn git_dirty() -> bool {
    Command::new("git")
        .args(["diff", "--quiet", "--ignore-submodules", "--"])
        .status()
        .is_ok_and(|status| !status.success())
}

fn command_stdout(command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_owned())
}
