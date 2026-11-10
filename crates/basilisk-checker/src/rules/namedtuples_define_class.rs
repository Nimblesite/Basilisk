//! Implements [`namedtuples_define_class`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `namedtuples_define_class`: `NamedTuple` class definition errors.
//!
//! Detects several categories of `NamedTuple` definition errors:
//!
//! 1. **Underscore field names**: Field names starting with `_` are illegal in
//!    `NamedTuple` definitions (the runtime raises `ValueError`).
//!
//! 2. **Default ordering**: Fields with default values must come after all fields
//!    without defaults (same rule as the runtime enforces).
//!
//! 3. **Subclass field conflict**: A `NamedTuple` subclass cannot redefine fields
//!    that exist in the base `NamedTuple`.
//!
//! 4. **Multiple inheritance**: `NamedTuple` does not support inheriting from
//!    multiple bases (other than `Generic[...]`).

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "namedtuples_define_class",
    docs_url: "https://www.basilisk-python.dev/errors/namedtuples_define_class",
};

/// Emits `namedtuples_define_class` for `NamedTuple` class definition errors.
pub(crate) struct NamedTupleDefError;

impl Rule for NamedTupleDefError {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
