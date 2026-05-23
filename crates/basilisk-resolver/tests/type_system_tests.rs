//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
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

#[path = "resolver/test_bounded_typevar.rs"]
mod test_bounded_typevar;

#[path = "resolver/test_typevar_calls.rs"]
mod test_typevar_calls;

#[path = "resolver/test_pep695.rs"]
mod test_pep695;

#[path = "resolver/test_type_alias.rs"]
mod test_type_alias;

#[path = "resolver/test_newtype.rs"]
mod test_newtype;

#[path = "resolver/test_literal_enum.rs"]
mod test_literal_enum;

#[path = "resolver/test_multiple_unbounded.rs"]
mod test_multiple_unbounded;

#[path = "resolver/test_generic_subscript.rs"]
mod test_generic_subscript;

#[path = "resolver/test_base_subscript_01.rs"]
mod test_base_subscript_01;

#[path = "resolver/test_base_subscript_02.rs"]
mod test_base_subscript_02;

#[path = "resolver/test_base_subscript_03.rs"]
mod test_base_subscript_03;
