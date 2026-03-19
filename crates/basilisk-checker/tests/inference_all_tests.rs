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
#[path = "checker/collection_inference_tests.rs"]
mod collection_inference;
mod common;
#[path = "checker/inference_tests.rs"]
mod inference;
#[path = "checker/inference_flow_tests.rs"]
mod inference_flow;
#[path = "checker/types_tests.rs"]
mod types;
