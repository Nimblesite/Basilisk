//! Tests for [literals_parameterizations]-[generics_self_attributes] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
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
#[path = "checker/aliases_type_statement_tests.rs"]
mod aliases_type_statement;
mod common;
#[path = "checker/constructors_call_new_tests.rs"]
mod constructors_call_new;
#[path = "checker/dataclasses_frozen_tests.rs"]
mod dataclasses_frozen;
#[path = "checker/dataclasses_hash_tests.rs"]
mod dataclasses_hash;
#[path = "checker/dataclasses_kwonly_tests.rs"]
mod dataclasses_kwonly;
#[path = "checker/dataclasses_match_args_tests.rs"]
mod dataclasses_match_args;
#[path = "checker/dataclasses_order_tests.rs"]
mod dataclasses_order;
#[path = "checker/directives_assert_type_2_tests.rs"]
mod directives_assert_type_2;
#[path = "checker/enums_expansion_tests.rs"]
mod enums_expansion;
#[path = "checker/enums_member_values_tests.rs"]
mod enums_member_values;
#[path = "checker/enums_members_2_tests.rs"]
mod enums_members_2;
#[path = "checker/generics_self_attributes_tests.rs"]
mod generics_self_attributes;
#[path = "checker/generics_typevartuple_basic_tests.rs"]
mod generics_typevartuple_basic;
#[path = "checker/historical_positional_tests.rs"]
mod historical_positional;
#[path = "checker/literals_parameterizations_tests.rs"]
mod literals_parameterizations;
#[path = "checker/literals_parameterizations_2_tests.rs"]
mod literals_parameterizations_2;
#[path = "checker/namedtuples_define_functional_tests.rs"]
mod namedtuples_define_functional;
#[path = "checker/namedtuples_type_compat_tests.rs"]
mod namedtuples_type_compat;
#[path = "checker/overloads_basic_tests.rs"]
mod overloads_basic;
#[path = "checker/qualifiers_annotated_2_tests.rs"]
mod qualifiers_annotated_2;
#[path = "checker/qualifiers_final_annotation_2_tests.rs"]
mod qualifiers_final_annotation_2;
#[path = "checker/specialtypes_never_tests.rs"]
mod specialtypes_never;
#[path = "checker/specialtypes_never_2_tests.rs"]
mod specialtypes_never_2;
#[path = "checker/specialtypes_promotions_tests.rs"]
mod specialtypes_promotions;
#[path = "checker/typeddicts_readonly_tests.rs"]
mod typeddicts_readonly;
