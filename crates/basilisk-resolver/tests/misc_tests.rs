#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    missing_docs
)]

mod common;

#[path = "resolver/test_docstring.rs"]
mod test_docstring;

#[path = "resolver/test_reveal_type.rs"]
mod test_reveal_type;

#[path = "resolver/test_rhs_classification.rs"]
mod test_rhs_classification;

#[path = "resolver/test_stub_body.rs"]
mod test_stub_body;

#[path = "resolver/test_float_param.rs"]
mod test_float_param;
