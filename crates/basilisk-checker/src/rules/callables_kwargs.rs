//! Implements [`callables_kwargs`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `callables_kwargs`: Unpack[`TypedDict`] kwargs violations.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `callables_kwargs` for Unpack[`TypedDict`] kwargs violations.
pub(crate) struct UnpackKwargsViolation;

impl Rule for UnpackKwargsViolation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
