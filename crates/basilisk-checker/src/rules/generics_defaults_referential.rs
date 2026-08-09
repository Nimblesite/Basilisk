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

//! Implements [`generics_defaults_referential`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_defaults_referential`: Invalid `TypeVar` default referencing another `TypeVar`.
//!
//! PEP 696 specifies constraints on `TypeVar` defaults that reference other `TypeVars`:
//!
//! 1. **Ordering**: When `TypeVar` T2 has default=T1, T1 must appear before T2 in generic parameter list
//! 2. **Outer scope references**: `TypeVar` cannot use a `TypeVar` from outer scope as default
//! 3. **Bound compatibility**: When T2 has default=T1, T1's bound must be a subtype of T2's bound
//! 4. **Constraint superset**: When T2 has default=T1 and T2 has constraints, T1's constraints must be a subset of T2's constraints
//!
//! ```python
//! from typing import TypeVar
//!
//! # Ordering violation
//! T2 = TypeVar("T2", default=T1)  # E — T1 not defined yet
//! T1 = TypeVar("T1")
//!
//! # Outer scope violation  
//! class Outer:
//!     T1 = TypeVar("T1")
//!     class Inner:
//!         T2 = TypeVar("T2", default=T1)  # E — T1 from outer scope
//!
//! # Bound compatibility violation
//! X1 = TypeVar("X1", bound=int)
//! Invalid1 = TypeVar("Invalid1", default=X1, bound=str)  # E — int is not a subtype of str
//!
//! # Constraint superset violation
//! Y1 = TypeVar("Y1", int, str)
//! Invalid2 = TypeVar("Invalid2", bool, complex, default=Y1)  # E — {bool, complex} is not a superset of {int, str}
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; this error code is retained for the rebuild — \
              see tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
const CODE: ErrorCode = ErrorCode {
    code: "generics_defaults_referential",
    docs_url: "https://www.basilisk-python.dev/errors/generics_defaults_referential",
};

/// Emits `generics_defaults_referential` for `TypeVar` default referential violations.
pub(crate) struct TypeVarDefaultReferential;

impl Rule for TypeVarDefaultReferential {
    // ##########################################################################
    // # DELETED AND GONE — `is_constraint_subset`, `format_constraints`,
    // # `check_ordering`, `check_bound_compatibility`,
    // # `check_constraint_compatibility`, `check_default_bound_vs_constraints`,
    // # `check_default_constraints_vs_bound`.
    // #
    // # NO PANIC SHELLS: their only caller (`check`, below) was deleted too, so
    // # there are no call sites left to keep visible — and each of them WAS the
    // # string comparison, so keeping them under `#[expect(dead_code)]` would
    // # leave the defect sitting in the tree waiting to be re-wired.
    // # DO NOT RECREATE ANY OF THEM.
    // ##########################################################################

    // ##########################################################################
    // # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
    // #
    // # The whole PEP 696 referential-default rule ran on RENDERED TYPE NAMES:
    // # `bound_type_name`, `default_type_name` and `constraint_type_names` are
    // # recorded by the resolver only when the value "is a simple name", so
    // # `bound=list[int]` or `default=Foo[int]` never reached the rule at all,
    // # and what did reach it was compared with `==` and set membership.
    // #
    // # Its four helpers (`check_ordering`, `check_bound_compatibility`,
    // # `check_constraint_compatibility`, `check_default_bound_vs_constraints`,
    // # `check_default_constraints_vs_bound`) all settled their verdicts that way,
    // # several of them through the DELETED string-keyed
    // # `SubtypingContext::is_subtype`.
    // #
    // # A default fitting a bound is ASSIGNABILITY; a default matching a
    // # constraint is EQUIVALENCE; a constraint set fitting another is a SUBSET
    // # relation over types. All three are `TypeNode` relations, and the
    // # `default=`/`bound=` argument EXPRESSIONS are in the AST.
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
            "basilisk-checker: `generics_defaults_referential::check` was DELETED because \
         every PEP 696 verdict it produced came from comparing RENDERED TYPE NAMES, on \
         resolver fields that silently drop any bound or default that is not a bare \
         name. It panics because the real implementation — the `bound=`/`default=` \
         expressions lowered to `TypeNode` and related with `assignable`/`equivalent` — \
         DOES NOT EXIST YET. Do not restore the string comparisons and do not skip the \
         check in its place."
        )
    }
}
