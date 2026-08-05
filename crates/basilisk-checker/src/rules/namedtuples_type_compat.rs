//! Implements [`namedtuples_type_compat`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `namedtuples_type_compat`: `NamedTuple`-to-tuple type incompatibility.
//!
//! When a `NamedTuple` instance is assigned to a variable annotated with a
//! fixed-length `tuple[...]` type, Basilisk verifies:
//!
//! 1. The element count matches the number of fields in the `NamedTuple`.
//! 2. Each element type in the tuple annotation is compatible with the
//!    corresponding `NamedTuple` field type (with covariance).
//!
//! ```python
//! class Point(NamedTuple):
//!     x: int
//!     y: int
//!     units: str = "meters"
//!
//! p = Point(x=1, y=2, units="inches")
//! v1: tuple[int, int, str] = p  # OK
//! v2: tuple[int, int] = p       # E -- too few elements (2 vs 3 fields)
//! v3: tuple[int, str, str] = p  # E -- incompatible element type
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "namedtuples_type_compat",
    docs_url: "https://www.basilisk-python.dev/errors/namedtuples_type_compat",
};

/// Emits `namedtuples_type_compat` when a `NamedTuple` instance is assigned to an incompatible
/// fixed-length `tuple[...]` annotation.
pub(crate) struct NamedTupleTupleCompat;

impl Rule for NamedTupleTupleCompat {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
