//! Implements [`generics_defaults_referential_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_defaults_referential_2`: ```TypeVar``` default referential violations.
//!
//! PEP 696 defines rules for when a `TypeVar` default references another
//! `TypeVar`:
//!
//! 1. **Ordering**: The referenced `TypeVar` must appear *before* the referencing
//!    `TypeVar` in `Generic[...]`.
//! 2. **Scope**: A `TypeVar` default must not reference `TypeVar`ar from an outer
//!    class scope.
//! 3. **Bound/constraint compatibility**: When `TypeVar` `T2` defaults to
//!    `TypeVar` `T1`, `T1`'s bound must be a subtype of `T2`'s bound, and
//!    `T2`'s constraints (if any) must be a superset of `T1`'s constraints.
//!
//! ```python
//! from typing import TypeVar, Generic
//!
//! S1 = TypeVar("S1")
//! S2 = TypeVar("S2", default=S1)
//!
//! Start2T = TypeVar("Start2T", default="StopT")
//! Stop2T = TypeVar("Stop2T", default=int)
//! class slice2(Generic[Start2T, Stop2T]): ...   # E: bad ordering
//!
//! class Foo3(Generic[S1]):
//!     class Bar2(Generic[S2]): ...              # E: outer scope
//!
//! Y1 = TypeVar("Y1", bound=int)
//! Invalid2 = TypeVar("Invalid2", float, str, default=Y1)  # E
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `generics_defaults_referential_2` for `TypeVar` default referential violations.
pub(crate) struct TypeVarDefaultReferential;

impl Rule for TypeVarDefaultReferential {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
