//! Tests for [BSK-E0001]-[BSK-E0025] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    missing_docs,
    clippy::needless_raw_string_hashes,
    clippy::uninlined_format_args,
    dead_code
)]
mod common;
#[path = "checker/e0001_tests.rs"]
mod e0001;
#[path = "checker/e0002_tests.rs"]
mod e0002;
#[path = "checker/e0003_tests.rs"]
mod e0003;
#[path = "checker/e0004_tests.rs"]
mod e0004;
#[path = "checker/e0005_tests.rs"]
mod e0005;
#[path = "checker/e0010_tests.rs"]
mod e0010;
#[path = "checker/e0011_tests.rs"]
mod e0011;
#[path = "checker/e0012_tests.rs"]
mod e0012;
#[path = "checker/e0013_tests.rs"]
mod e0013;
#[path = "checker/e0014_tests.rs"]
mod e0014;
#[path = "checker/e0015_tests.rs"]
mod e0015;
#[path = "checker/e0016_tests.rs"]
mod e0016;
#[path = "checker/e0017_tests.rs"]
mod e0017;
#[path = "checker/e0018_tests.rs"]
mod e0018;
#[path = "checker/e0019_tests.rs"]
mod e0019;
#[path = "checker/e0020_tests.rs"]
mod e0020;
#[path = "checker/e0021_tests.rs"]
mod e0021;
#[path = "checker/e0022_tests.rs"]
mod e0022;
#[path = "checker/e0022_e0023_tests.rs"]
mod e0022_e0023;
#[path = "checker/e0023_tests.rs"]
mod e0023;
#[path = "checker/e0024_tests.rs"]
mod e0024;
#[path = "checker/e0025_tests.rs"]
mod e0025;
