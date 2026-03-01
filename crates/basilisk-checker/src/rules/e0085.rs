//! BSK-E0085: `TypeVarTuple` argument count mismatch.
//!
//! When a constructor with `TypeVarTuple` parameters is called, the number of
//! arguments must match the expected count inferred from the `TypeVarTuple`.
//!
//! ```python
//! Ts = TypeVarTuple("Ts")
//!
//! class Array(Generic[*Ts]):
//!     def __init__(self, shape: tuple[*Ts]) -> None: ...
//!
//! Array[Height, Width]((Height(1), Width(2)))  # OK
//! Array[Height, Width](Height(1))              # E: expected 2 arguments, got 1
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0085",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0085",
};

/// Emits BSK-E0085 when a constructor call has incorrect argument count for TypeVarTuple.
pub(crate) struct TypeVarTupleArgCountMismatch;

impl Rule for TypeVarTupleArgCountMismatch {
    fn check(&self, _module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
        // TODO: Implement TypeVarTuple argument count validation
        // This rule should check constructor argument counts against TypeVarTuple expectations
    }
}