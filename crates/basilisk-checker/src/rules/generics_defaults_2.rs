// ############################################################################
// # BROKEN — THIS FILE DOES NOT COMPILE. DO NOT "FIX" IT BY RESTORING TEXT   #
// # MATCHING.                                                                #
// #                                                                          #
// # Deleted helper this file called:                                         #
// #   crate::subtyping (SubtypingContext::is_subtype / name_subtype)
// #                                                                          #
// # That helper decided types from the SPELLING of source text (lowercased   #
// # annotation strings, `"int"`/`"str"`/`"object"` literal matching, `|`     #
// # splitting, `starts_with("tuple[")`). It was deleted, not replaced.       #
// #                                                                          #
// # The call sites below are LEFT BROKEN ON PURPOSE. They are the map of     #
// # what must be rebuilt on the resolved AST — resolved bindings, canonical  #
// # `TypeNode`, and `assignable`/`equivalent` — or made to abstain.          #
// #                                                                          #
// # Restoring the deleted helper, vendoring a copy of it, or re-deriving a   #
// # type from source text anywhere below is FORBIDDEN.                       #
// #                                                                          #
// # Evidence and the failing tests that pin the real behaviour:              #
// #   docs/RULE-VALIDITY-REPORT.md                                           #
// #   crates/basilisk-checker/tests/legacy_annotation_text_parser_pin_tests.rs
// #   crates/basilisk-checker/tests/pep_spelling_invariance_pin_tests.rs     #
// ############################################################################

//! Implements [`generics_defaults_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_defaults_2`: Incompatible `TypeVar` bound or constraint with its default.
//!
//! PEP 696 specifies two constraints on `TypeVar` defaults:
//!
//! 1. If both `bound` and `default` are specified, the default must be a subtype
//!    of the bound. The numeric subtype hierarchy is `bool <: int <: float <: complex`.
//!
//! 2. For constrained `TypeVar`s, the default must be one of the constraints exactly.
//!    (Even a subtype is disallowed — `float` is a subtype of `complex` but if the
//!    constraints are `[float, str]` and the default is `complex`, that is an error.)
//!
//! ```python
//! from typing import TypeVar
//!
//! Ok1 = TypeVar("Ok1", bound=float, default=int)     # OK — int <: float
//! Invalid1 = TypeVar("Invalid1", bound=str, default=int)  # E — int is not <: str
//!
//! Ok2 = TypeVar("Ok2", float, str, default=float)    # OK
//! Invalid2 = TypeVar("Invalid2", float, str, default=int)  # E — int not in {float, str}
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
const CODE: ErrorCode = ErrorCode {
    code: "generics_defaults_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_defaults_2",
};

/// Emits `generics_defaults_2` for `TypeVar` bound/constraint vs default incompatibilities.
pub(crate) struct TypeVarDefaultIncompatible;

impl Rule for TypeVarDefaultIncompatible {
    // ##########################################################################
    // # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
    // #
    // #   tv.constraint_type_names.iter().any(|c| c == default_name)
    // #   subtyping.is_subtype(default_name, bound_name)      // DELETED helper
    // #
    // # PEP 696 conformance decided by STRING EQUALITY between rendered type
    // # names. `TypeVarCallInfo` records these only when the value "is a simple
    // # name", so `default=list[int]` never reached the check at all; what did
    // # reach it compared `int` and `Int` (aliased import) as unequal, and two
    // # unrelated classes sharing a rendered name as equal.
    // #
    // # A default matching a constraint is TYPE EQUIVALENCE; a default fitting a
    // # bound is ASSIGNABILITY. Both are `TypeNode` relations.
    // #
    // # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
    // ##########################################################################
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        panic!(
            "basilisk-checker: `generics_defaults_2::check` was DELETED because it settled \
         PEP 696 default-vs-bound and default-vs-constraint conformance by comparing \
         RENDERED TYPE NAMES as strings. It panics because the real implementation — \
         both sides lowered to `TypeNode` through the binding table and related with \
         `assignable`/`equivalent` — DOES NOT EXIST YET. Do not restore the string \
         comparison and do not skip the check in its place."
        )
    }
}
