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

#[path = "resolver/test_imports.rs"]
mod test_imports;

#[path = "resolver/test_isinstance.rs"]
mod test_isinstance;

#[path = "resolver/test_module_calls.rs"]
mod test_module_calls;

#[path = "resolver/test_module_level.rs"]
mod test_module_level;

#[path = "resolver/test_control_flow.rs"]
mod test_control_flow;

#[path = "resolver/test_conditional_assigns.rs"]
mod test_conditional_assigns;

#[path = "resolver/test_local_assigns.rs"]
mod test_local_assigns;

#[path = "resolver/test_self_assigns.rs"]
mod test_self_assigns;

#[path = "resolver/test_collect_calls.rs"]
mod test_collect_calls;

#[path = "resolver/test_collect_from_stmt.rs"]
mod test_collect_from_stmt;

#[path = "resolver/test_match.rs"]
mod test_match;

#[path = "resolver/test_historical_positional.rs"]
mod test_historical_positional;
