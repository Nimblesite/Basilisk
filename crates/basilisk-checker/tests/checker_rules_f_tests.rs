//! Tests for [`literals_literalstring`]-[`generics_syntax_scoping`] from [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
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
#[path = "checker/annotations_generators_2_tests.rs"]
mod annotations_generators_2;
#[path = "checker/callables_kwargs_tests.rs"]
mod callables_kwargs;
#[path = "checker/callables_protocol_2_tests.rs"]
mod callables_protocol_2;
#[path = "checker/callables_subtyping_tests.rs"]
mod callables_subtyping;
mod common;
#[path = "checker/constructors_call_type_tests.rs"]
mod constructors_call_type;
#[path = "checker/dataclasses_transform_class_tests.rs"]
mod dataclasses_transform_class;
#[path = "checker/dataclasses_transform_meta_tests.rs"]
mod dataclasses_transform_meta;
#[path = "checker/explicit_any_tests.rs"]
mod explicit_any;
#[path = "checker/generics_base_class_2_tests.rs"]
mod generics_base_class_2;
#[path = "checker/generics_base_class_3_tests.rs"]
mod generics_base_class_3;
#[path = "checker/generics_basic_3_tests.rs"]
mod generics_basic_3;
#[path = "checker/generics_defaults_referential_2_tests.rs"]
mod generics_defaults_referential_2;
#[path = "checker/generics_syntax_scoping_tests.rs"]
mod generics_syntax_scoping;
#[path = "checker/generics_typevartuple_specialization_2_tests.rs"]
mod generics_typevartuple_specialization_2;
#[path = "checker/generics_variance_inference_tests.rs"]
mod generics_variance_inference;
#[path = "checker/lambda_missing_annotations_tests.rs"]
mod lambda_missing_annotations;
#[path = "checker/literals_literalstring_tests.rs"]
mod literals_literalstring;
#[path = "checker/literals_semantics_2_tests.rs"]
mod literals_semantics_2;
#[path = "checker/namedtuples_usage_tests.rs"]
mod namedtuples_usage;
#[path = "checker/protocols_class_objects_2_tests.rs"]
mod protocols_class_objects_2;
#[path = "checker/protocols_generic_tests.rs"]
mod protocols_generic;
#[path = "checker/protocols_variance_2_tests.rs"]
mod protocols_variance_2;
#[path = "checker/redundant_annotation_rule_tests.rs"]
mod redundant_annotation_rule;
#[path = "checker/specialtypes_type_tests.rs"]
mod specialtypes_type;
#[path = "checker/tuples_index_2_tests.rs"]
mod tuples_index_2;
#[path = "checker/tuples_type_compat_tests.rs"]
mod tuples_type_compat;
