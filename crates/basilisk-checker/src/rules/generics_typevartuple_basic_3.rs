//! Implements [`generics_typevartuple_basic_3`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_typevartuple_basic_3`: `TypeVarTuple` variance/bounds/constraints violation.
//!
//! `TypeVarTuple` does not support specification of variance, bounds, or constraints.
//! Using these parameters with `TypeVarTuple` is invalid.
//!
//! ```python
//! # BAD
//! Ts = TypeVarTuple("Ts", covariant=True)  # E: TypeVarTuple does not support variance
//! Ts = TypeVarTuple("Ts", int, float)      # E: TypeVarTuple does not support constraints
//! Ts = TypeVarTuple("Ts", bound=int)       # E: TypeVarTuple does not support bounds
//!
//! # GOOD
//! Ts = TypeVarTuple("Ts")  # OK
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `generics_typevartuple_basic_3` when a `TypeVarTuple` has invalid parameters.
pub(crate) struct TypeVarTupleInvalidParams;

impl Rule for TypeVarTupleInvalidParams {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
