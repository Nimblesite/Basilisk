//! BSK-E0083: `TypeVarTuple` must be unpacked with `*` operator.
//!
//! When a `TypeVarTuple` is used in a type annotation, it must be unpacked
//! using the `*` operator. Using a `TypeVarTuple` without unpacking is invalid.
//!
//! ```python
//! # BAD
//! Ts = TypeVarTuple("Ts")
//! def func(arg: Ts) -> None:  # E: TypeVarTuple must be unpacked
//!     ...
//!
//! # GOOD
//! def func(arg: *Ts) -> None:  # OK
//!     ...
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Placeholder for BSK-E0083: TypeVarTuple unpacking validation (not yet implemented).
pub(crate) struct TypeVarTupleUnpackRequired;

impl Rule for TypeVarTupleUnpackRequired {
    fn check(&self, _module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
        // Not yet implemented.
    }
}
