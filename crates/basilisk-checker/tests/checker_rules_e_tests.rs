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
#[path = "checker/aliases_recursive_tests.rs"]
mod aliases_recursive;
#[path = "checker/annotations_generators_tests.rs"]
mod annotations_generators;
#[path = "checker/callables_protocol_tests.rs"]
mod callables_protocol;
mod common;
#[path = "checker/constructors_call_init_tests.rs"]
mod constructors_call_init;
#[path = "checker/dataclasses_slots_tests.rs"]
mod dataclasses_slots;
#[path = "checker/directives_deprecated_tests.rs"]
mod directives_deprecated;
#[path = "checker/generics_defaults_referential_tests.rs"]
mod generics_defaults_referential;
#[path = "checker/generics_scoping_tests.rs"]
mod generics_scoping;
#[path = "checker/generics_syntax_declarations_2_tests.rs"]
mod generics_syntax_declarations_2;
#[path = "checker/generics_type_erasure_tests.rs"]
mod generics_type_erasure;
#[path = "checker/generics_upper_bound_2_tests.rs"]
mod generics_upper_bound_2;
#[path = "checker/generics_variance_tests.rs"]
mod generics_variance;
#[path = "checker/namedtuples_define_class_tests.rs"]
mod namedtuples_define_class;
#[path = "checker/narrowing_typeguard_tests.rs"]
mod narrowing_typeguard;
#[path = "checker/narrowing_typeis_tests.rs"]
mod narrowing_typeis;
#[path = "checker/narrowing_typeis_2_tests.rs"]
mod narrowing_typeis_2;
#[path = "checker/protocols_class_objects_tests.rs"]
mod protocols_class_objects;
#[path = "checker/protocols_definition_2_tests.rs"]
mod protocols_definition_2;
#[path = "checker/protocols_explicit_2_tests.rs"]
mod protocols_explicit_2;
#[path = "checker/protocols_explicit_3_tests.rs"]
mod protocols_explicit_3;
#[path = "checker/protocols_runtime_checkable_tests.rs"]
mod protocols_runtime_checkable;
#[path = "checker/protocols_runtime_checkable_2_tests.rs"]
mod protocols_runtime_checkable_2;
#[path = "checker/protocols_subtyping_tests.rs"]
mod protocols_subtyping;
#[path = "checker/protocols_variance_tests.rs"]
mod protocols_variance;
#[path = "checker/tuples_index_tests.rs"]
mod tuples_index;
