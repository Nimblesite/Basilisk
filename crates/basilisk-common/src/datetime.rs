//! Implements [CHKARCH-ARCH-BUILD-VERSIONINFO].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-BUILD-VERSIONINFO
//!
//! Minimal RFC 3339 / ISO 8601 UTC formatting without `chrono`.
//!
//! This is the single source of truth for converting a Unix timestamp into a
//! `YYYY-MM-DDThh:mm:ssZ` string. It is shared by the build scripts (which
//! stamp `SHIPWRIGHT_BUILD_TIME`) and the profiler (which timestamps live
//! samples), so the calendar arithmetic exists in exactly one place. Pure and
//! dependency-free, it upholds this crate's zero-dependency / WASM contract.

const SECS_PER_DAY: u64 = 86_400;
const SECS_PER_HOUR: u64 = 3_600;
const SECS_PER_MINUTE: u64 = 60;

/// Format a Unix timestamp (seconds since the epoch) as an RFC 3339 UTC string,
/// e.g. `2026-06-18T12:34:56Z`.
#[must_use]
pub fn rfc3339_from_secs(secs: u64) -> String {
    let days = secs / SECS_PER_DAY;
    let seconds_of_day = secs % SECS_PER_DAY;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / SECS_PER_HOUR;
    let minute = seconds_of_day % SECS_PER_HOUR / SECS_PER_MINUTE;
    let second = seconds_of_day % SECS_PER_MINUTE;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

// Howard Hinnant's `civil_from_days` (public domain): convert a count of days
// since the Unix epoch into a `(year, month, day)` triple.
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
