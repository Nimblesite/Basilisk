//! Implements [`protocols_variance_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_variance_2`: Protocol `TypeVar` variance mismatch.
//!
//! When a generic protocol class declares a `TypeVar` as invariant but the
//! inferred variance (from method parameter and return positions) is strictly
//! covariant or contravariant, a diagnostic is emitted recommending the more
//! specific variance.
//!
//! PEP 544 specifies that type checkers should warn when the inferred variance
//! of a type variable used in a protocol differs from its declared variance.
//!
//! ```python
//! from typing import Protocol, TypeVar
//!
//! T = TypeVar("T")  # invariant
//!
//! class MyProto(Protocol[T]):  # E — T should be covariant
//!     def method(self) -> T: ...
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `protocols_variance_2` for protocol `TypeVar` variance mismatches.
pub(crate) struct ProtocolVarianceMismatch;

impl Rule for ProtocolVarianceMismatch {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Detection deleted: the previous implementation recognised `Protocol`
        // bases by comparing source-text spellings against a hardcoded string
        // instead of resolving imports. Pending a resolver-based
        // reimplementation.
    }
}
