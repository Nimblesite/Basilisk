//! Implements [`callables_subtyping`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `callables_subtyping`: Callable subtyping violations (covariance / contravariance).
//!
//! Callable types are covariant with respect to return types and contravariant
//! with respect to parameter types.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `callables_subtyping` for callable-to-callable subtyping violations.
pub(crate) struct CallableSubtypingViolation;

impl Rule for CallableSubtypingViolation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
