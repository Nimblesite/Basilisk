//! BSK-E0084: `TypeVarTuple` variance/bounds/constraints violation.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0084",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0084",
};

/// Emits BSK-E0084 when a `TypeVarTuple` has invalid parameters.
pub(crate) struct TypeVarTupleInvalidParams;

impl Rule for TypeVarTupleInvalidParams {
    fn check(&self, _module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
        // TODO: Implement TypeVarTuple parameter validation
        // This rule should check for variance, bounds, constraints on TypeVarTuple
    }
}