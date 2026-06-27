//! Tests for [overloads_evaluation]-[literals_semantics] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
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
#[path = "checker/dataclasses_postinit_tests.rs"]
mod dataclasses_postinit;
#[path = "checker/dataclasses_usage_tests.rs"]
mod dataclasses_usage;
#[path = "checker/generics_defaults_2_tests.rs"]
mod generics_defaults_2;
#[path = "checker/generics_defaults_specialization_tests.rs"]
mod generics_defaults_specialization;
#[path = "checker/generics_self_basic_tests.rs"]
mod generics_self_basic;
#[path = "checker/generics_self_protocols_tests.rs"]
mod generics_self_protocols;
#[path = "checker/generics_self_usage_tests.rs"]
mod generics_self_usage;
#[path = "checker/generics_syntax_declarations_tests.rs"]
mod generics_syntax_declarations;
#[path = "checker/generics_typevartuple_args_tests.rs"]
mod generics_typevartuple_args;
#[path = "checker/generics_typevartuple_basic_2_tests.rs"]
mod generics_typevartuple_basic_2;
#[path = "checker/generics_typevartuple_basic_3_tests.rs"]
mod generics_typevartuple_basic_3;
#[path = "checker/generics_typevartuple_callable_tests.rs"]
mod generics_typevartuple_callable;
#[path = "checker/generics_typevartuple_specialization_tests.rs"]
mod generics_typevartuple_specialization;
#[path = "checker/generics_typevartuple_unpack_tests.rs"]
mod generics_typevartuple_unpack;
#[path = "checker/generics_upper_bound_tests.rs"]
mod generics_upper_bound;
#[path = "checker/literals_semantics_tests.rs"]
mod literals_semantics;
#[path = "checker/overloads_evaluation_tests.rs"]
mod overloads_evaluation;
#[path = "checker/protocols_definition_tests.rs"]
mod protocols_definition;
#[path = "checker/protocols_explicit_tests.rs"]
mod protocols_explicit;
#[path = "checker/protocols_merging_tests.rs"]
mod protocols_merging;
#[path = "checker/protocols_modules_tests.rs"]
mod protocols_modules;
#[path = "checker/tuples_type_form_2_tests.rs"]
mod tuples_type_form_2;
#[path = "checker/typeddicts_operations_tests.rs"]
mod typeddicts_operations;
#[path = "checker/typeddicts_usage_tests.rs"]
mod typeddicts_usage;
