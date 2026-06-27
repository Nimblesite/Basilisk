//! Tests for [generics_basic]-[aliases_newtype] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
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
#[path = "checker/e0026_tests.rs"]
mod e0026;
#[path = "checker/e0027_tests.rs"]
mod e0027;
#[path = "checker/e0029_tests.rs"]
mod e0029;
#[path = "checker/e0030_tests.rs"]
mod e0030;
#[path = "checker/e0031_tests.rs"]
mod e0031;
#[path = "checker/e0032_tests.rs"]
mod e0032;
#[path = "checker/e0033_tests.rs"]
mod e0033;
#[path = "checker/e0033_e0039_tests.rs"]
mod e0033_e0039;
#[path = "checker/e0034_tests.rs"]
mod e0034;
#[path = "checker/e0035_tests.rs"]
mod e0035;
#[path = "checker/e0036_tests.rs"]
mod e0036;
#[path = "checker/e0037_tests.rs"]
mod e0037;
#[path = "checker/e0038_tests.rs"]
mod e0038;
#[path = "checker/e0039_tests.rs"]
mod e0039;
#[path = "checker/e0040_tests.rs"]
mod e0040;
#[path = "checker/e0040_e0046_tests.rs"]
mod e0040_e0046;
#[path = "checker/e0041_tests.rs"]
mod e0041;
#[path = "checker/e0042_tests.rs"]
mod e0042;
#[path = "checker/e0043_tests.rs"]
mod e0043;
#[path = "checker/e0044_tests.rs"]
mod e0044;
#[path = "checker/e0045_tests.rs"]
mod e0045;
#[path = "checker/e0046_tests.rs"]
mod e0046;
#[path = "checker/e0047_tests.rs"]
mod e0047;
#[path = "checker/e0048_tests.rs"]
mod e0048;
#[path = "checker/e0049_tests.rs"]
mod e0049;
#[path = "checker/e0050_tests.rs"]
mod e0050;
