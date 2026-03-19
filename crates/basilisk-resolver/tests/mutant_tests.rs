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

#[path = "resolver/test_mutant_alias.rs"]
mod test_mutant_alias;

#[path = "resolver/test_mutant_annotation.rs"]
mod test_mutant_annotation;

#[path = "resolver/test_mutant_class_info.rs"]
mod test_mutant_class_info;

#[path = "resolver/test_mutant_classify_rhs.rs"]
mod test_mutant_classify_rhs;

#[path = "resolver/test_mutant_collect.rs"]
mod test_mutant_collect;

#[path = "resolver/test_mutant_generic_params.rs"]
mod test_mutant_generic_params;

#[path = "resolver/test_mutant_special_calls.rs"]
mod test_mutant_special_calls;

#[path = "resolver/test_mutant_typeddict.rs"]
mod test_mutant_typeddict;

#[path = "resolver/test_mutant_typevar.rs"]
mod test_mutant_typevar;

#[path = "resolver/test_mutant_visitor.rs"]
mod test_mutant_visitor;

#[path = "resolver/test_visitor_coverage.rs"]
mod test_visitor_coverage;

#[path = "resolver/test_coverage.rs"]
mod test_coverage;
