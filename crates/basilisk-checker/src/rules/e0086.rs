//! BSK-E0086: Multiple `TypeVarTuple` declarations in generic.
//!
//! Only a single `TypeVarTuple` may appear in a type parameter list.
//! Using multiple `TypeVarTuple` declarations is invalid.
//!
//! ```python
//! # BAD
//! Ts1 = TypeVarTuple("Ts1")
//! Ts2 = TypeVarTuple("Ts2")
//! class Array3(Generic[*Ts1, *Ts2]):  # E: multiple TypeVarTuples not allowed
//!     ...
//!
//! # GOOD
//! class Array(Generic[*Ts]):  # OK
//!     ...
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits BSK-E0086 when multiple `TypeVarTuples` are used in a generic.
pub(crate) struct MultipleTypeVarTuplesInGeneric;

impl Rule for MultipleTypeVarTuplesInGeneric {
    fn check(&self, _module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
        // TODO: Implement multiple TypeVarTuple validation
        // This rule should check for multiple TypeVarTuples in generic declarations
    }
}