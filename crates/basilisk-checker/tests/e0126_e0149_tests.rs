//! Tests for [BSK-E0126]-[BSK-E0149] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
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
#[path = "checker/e0126_tests.rs"]
mod e0126;
#[path = "checker/e0127_tests.rs"]
mod e0127;
#[path = "checker/e0128_tests.rs"]
mod e0128;
#[path = "checker/e0129_tests.rs"]
mod e0129;
#[path = "checker/e0130_tests.rs"]
mod e0130;
#[path = "checker/e0131_tests.rs"]
mod e0131;
#[path = "checker/e0132_tests.rs"]
mod e0132;
#[path = "checker/e0133_tests.rs"]
mod e0133;
#[path = "checker/e0134_tests.rs"]
mod e0134;
#[path = "checker/e0136_tests.rs"]
mod e0136;
#[path = "checker/e0137_tests.rs"]
mod e0137;
#[path = "checker/e0138_tests.rs"]
mod e0138;
#[path = "checker/e0139_tests.rs"]
mod e0139;
#[path = "checker/e0140_tests.rs"]
mod e0140;
#[path = "checker/e0141_tests.rs"]
mod e0141;
#[path = "checker/e0142_tests.rs"]
mod e0142;
#[path = "checker/e0143_tests.rs"]
mod e0143;
#[path = "checker/e0144_tests.rs"]
mod e0144;
#[path = "checker/e0145_tests.rs"]
mod e0145;
#[path = "checker/e0146_tests.rs"]
mod e0146;
#[path = "checker/e0147_tests.rs"]
mod e0147;
#[path = "checker/e0148_tests.rs"]
mod e0148;
#[path = "checker/e0149_tests.rs"]
mod e0149;
#[path = "checker/w0014_tests.rs"]
mod w0014;
#[path = "checker/w0040_tests.rs"]
mod w0040;
#[path = "checker/w0050_tests.rs"]
mod w0050;
