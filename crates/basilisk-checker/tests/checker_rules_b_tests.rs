//! Tests for [`generics_basic`]-[`aliases_newtype`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
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
#[path = "checker/aliases_implicit_tests.rs"]
mod aliases_implicit;
#[path = "checker/aliases_newtype_tests.rs"]
mod aliases_newtype;
#[path = "checker/annotations_forward_refs_tests.rs"]
mod annotations_forward_refs;
#[path = "checker/calls_argument_count_tests.rs"]
mod calls_argument_count;
#[path = "checker/classes_classvar_tests.rs"]
mod classes_classvar;
mod common;
#[path = "checker/directives_assert_type_tests.rs"]
mod directives_assert_type;
#[path = "checker/directives_cast_tests.rs"]
mod directives_cast;
#[path = "checker/directives_reveal_type_tests.rs"]
mod directives_reveal_type;
#[path = "checker/directives_reveal_type_group_tests.rs"]
mod directives_reveal_type_group;
#[path = "checker/enums_behaviors_tests.rs"]
mod enums_behaviors;
#[path = "checker/enums_behaviors_group_tests.rs"]
mod enums_behaviors_group;
#[path = "checker/enums_members_tests.rs"]
mod enums_members;
#[path = "checker/generics_base_class_tests.rs"]
mod generics_base_class;
#[path = "checker/generics_basic_tests.rs"]
mod generics_basic;
#[path = "checker/generics_basic_2_tests.rs"]
mod generics_basic_2;
#[path = "checker/generics_defaults_tests.rs"]
mod generics_defaults;
#[path = "checker/generics_syntax_compatibility_tests.rs"]
mod generics_syntax_compatibility;
#[path = "checker/qualifiers_annotated_tests.rs"]
mod qualifiers_annotated;
#[path = "checker/qualifiers_final_annotation_tests.rs"]
mod qualifiers_final_annotation;
#[path = "checker/qualifiers_final_decorator_tests.rs"]
mod qualifiers_final_decorator;
#[path = "checker/tuples_type_form_tests.rs"]
mod tuples_type_form;
#[path = "checker/typeddicts_alt_syntax_tests.rs"]
mod typeddicts_alt_syntax;
#[path = "checker/typeddicts_class_syntax_tests.rs"]
mod typeddicts_class_syntax;
#[path = "checker/typeddicts_class_syntax_2_tests.rs"]
mod typeddicts_class_syntax_2;
#[path = "checker/typeddicts_inheritance_tests.rs"]
mod typeddicts_inheritance;
#[path = "checker/typeddicts_required_tests.rs"]
mod typeddicts_required;
