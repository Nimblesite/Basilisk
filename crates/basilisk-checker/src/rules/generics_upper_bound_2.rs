//! Implements [`generics_upper_bound_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_upper_bound_2`: `TypeVar` bound violation at call site.
//!
//! When a function has a parameter typed with a `TypeVar` that has a `bound`,
//! and a call passes an argument whose type is not a subtype of that bound,
//! this rule reports the mismatch.
//!
//! ```python
//! TLiteral = TypeVar("TLiteral", bound=LiteralString)
//!
//! def literal_identity(s: TLiteral) -> TLiteral:
//!     return s
//!
//! def func5(s: str):
//!     literal_identity(s)  # E — str is not a subtype of LiteralString
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "generics_upper_bound_2",
    docs_url: "https://www.basilisk-python.dev/errors/generics_upper_bound_2",
};

/// Emits `generics_upper_bound_2` when a call-site argument type violates a `TypeVar`'s bound.
pub(crate) struct TypeVarBoundCallViolation;

impl Rule for TypeVarBoundCallViolation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
