//! Tests for [BSK-0001]-[BSK-0025] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
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
#[path = "checker/annotations_typeexpr_tests.rs"]
mod annotations_typeexpr;
#[path = "checker/assignment_compatibility_tests.rs"]
mod assignment_compatibility;
#[path = "checker/callables_annotation_tests.rs"]
mod callables_annotation;
#[path = "checker/calls_argument_type_tests.rs"]
mod calls_argument_type;
#[path = "checker/classes_override_tests.rs"]
mod classes_override;
#[path = "checker/classes_override_2_tests.rs"]
mod classes_override_2;
mod common;
#[path = "checker/dict_key_hashable_tests.rs"]
mod dict_key_hashable;
#[path = "checker/dict_key_hashable_group_tests.rs"]
mod dict_key_hashable_group;
#[path = "checker/imports_unresolved_tests.rs"]
mod imports_unresolved;
#[path = "checker/match_exhaustiveness_tests.rs"]
mod match_exhaustiveness;
#[path = "checker/missing_attribute_annotation_tests.rs"]
mod missing_attribute_annotation;
#[path = "checker/missing_override_decorator_tests.rs"]
mod missing_override_decorator;
#[path = "checker/missing_parameter_annotation_tests.rs"]
mod missing_parameter_annotation;
#[path = "checker/missing_return_annotation_tests.rs"]
mod missing_return_annotation;
#[path = "checker/missing_vararg_annotation_tests.rs"]
mod missing_vararg_annotation;
#[path = "checker/missing_variable_type_tests.rs"]
mod missing_variable_type;
#[path = "checker/names_unbound_tests.rs"]
mod names_unbound;
#[path = "checker/names_undefined_tests.rs"]
mod names_undefined;
#[path = "checker/overloads_consistency_tests.rs"]
mod overloads_consistency;
#[path = "checker/overloads_definitions_tests.rs"]
mod overloads_definitions;
#[path = "checker/returns_compatibility_tests.rs"]
mod returns_compatibility;
#[path = "checker/returns_compatibility_2_tests.rs"]
mod returns_compatibility_2;
