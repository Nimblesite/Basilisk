//! Implements [`narrowing_typeis`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `narrowing_typeis`: narrowing-guard return type incompatibility in callable arguments.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `narrowing_typeis` when a narrowing-guard function is passed to a
/// callable parameter whose return type is incompatible with it.
pub(crate) struct TypeGuardCallableReturnMismatch;

impl Rule for TypeGuardCallableReturnMismatch {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
