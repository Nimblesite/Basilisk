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

#[path = "resolver/test_basic_functions.rs"]
mod test_basic_functions;

#[path = "resolver/test_function_properties.rs"]
mod test_function_properties;

#[path = "resolver/test_returns.rs"]
mod test_returns;

#[path = "resolver/test_generator.rs"]
mod test_generator;

#[path = "resolver/test_yield.rs"]
mod test_yield;

#[path = "resolver/test_decorators.rs"]
mod test_decorators;
