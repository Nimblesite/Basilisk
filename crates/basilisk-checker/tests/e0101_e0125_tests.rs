//! Tests for [narrowing_typeguard]-[generics_type_erasure] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
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
#[path = "checker/e0101_tests.rs"]
mod e0101;
#[path = "checker/e0102_tests.rs"]
mod e0102;
#[path = "checker/e0103_tests.rs"]
mod e0103;
#[path = "checker/e0104_tests.rs"]
mod e0104;
#[path = "checker/e0105_tests.rs"]
mod e0105;
#[path = "checker/e0106_tests.rs"]
mod e0106;
#[path = "checker/e0107_tests.rs"]
mod e0107;
#[path = "checker/e0108_tests.rs"]
mod e0108;
#[path = "checker/e0109_tests.rs"]
mod e0109;
#[path = "checker/e0110_tests.rs"]
mod e0110;
#[path = "checker/e0111_tests.rs"]
mod e0111;
#[path = "checker/e0112_tests.rs"]
mod e0112;
#[path = "checker/e0113_tests.rs"]
mod e0113;
#[path = "checker/e0114_tests.rs"]
mod e0114;
#[path = "checker/e0115_tests.rs"]
mod e0115;
#[path = "checker/e0116_tests.rs"]
mod e0116;
#[path = "checker/e0117_tests.rs"]
mod e0117;
#[path = "checker/e0118_tests.rs"]
mod e0118;
#[path = "checker/e0119_tests.rs"]
mod e0119;
#[path = "checker/e0120_tests.rs"]
mod e0120;
#[path = "checker/e0121_tests.rs"]
mod e0121;
#[path = "checker/e0122_tests.rs"]
mod e0122;
#[path = "checker/e0123_tests.rs"]
mod e0123;
#[path = "checker/e0124_tests.rs"]
mod e0124;
#[path = "checker/e0125_tests.rs"]
mod e0125;
