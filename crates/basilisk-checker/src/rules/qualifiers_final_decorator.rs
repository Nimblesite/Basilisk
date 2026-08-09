//! Implements [`qualifiers_final_decorator`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `qualifiers_final_decorator`: `@final` decorator violations.
//!
//! Two violations are detected:
//!
//! 1. **Inheriting from a `@final` class** — a class decorated with `@final`
//!    cannot be subclassed.
//!
//! 2. **Overriding a `@final` method** — a method decorated with `@final`
//!    in a base class cannot be overridden in a subclass.



use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
const CODE: ErrorCode = ErrorCode {
    code: "qualifiers_final_decorator",
    docs_url: "https://www.basilisk-python.dev/errors/qualifiers_final_decorator",
};

/// Emits `qualifiers_final_decorator` for `@final` decorator violations.
pub(crate) struct FinalViolation;

impl Rule for FinalViolation {
    // ##################################################################
    // # DELETED BODY. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
    // #
    // # `class_map.get(base_name.as_str())` decided PEP 591 @final inheritance by rendered base name, and `imported_final_methods.get(base_name.as_str())` did the same for @final methods.
    // #
    // # `ClassInfo::bases` holds RENDERED SIMPLE NAMES ("complex
    // # expressions ignored") and the lookup map is keyed on
    // # `ClassInfo::name`, so an aliased base MISSED, a dotted base
    // # collided with any local class sharing its trailing word, and two
    // # classes with one rendered name were a single entry.
    // #
    // # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
    // ##################################################################
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    panic!(
        "basilisk-checker: `qualifiers_final_decorator::check` was DELETED because it identified base classes by \
         their RENDERED NAMES. It panics because the real implementation — base \
         expressions resolved through the binding table — DOES NOT EXIST YET. Do not \
         restore the name lookup and do not substitute a default answer."
    )
}
}
