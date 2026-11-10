//! External tests for [TYPEINF], [TYPEINF-OVERVIEW], [TYPEINF-INFERRED],
//! [TYPEINF-ALGO], [TYPEINF-VARS], [TYPEINF-SUBTYPING],
//! [TYPEINF-SUBTYPING-IMPL], [TYPEINF-SPECIAL], [TYPEINF-IMPL],
//! [TYPEINF-EXCEEDS], [TYPEINF-EXCEEDS-NOUNKNOWN], and
//! [TYPEINF-EXCEEDS-CONTAINERS]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md.
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
#[path = "checker/guards_exemption_tests.rs"]
mod guards_exemption;
#[path = "checker/inference_tests.rs"]
mod inference;
#[path = "checker/types_tests.rs"]
mod types;
