//! Implements [`protocols_subtyping`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_subtyping`: Protocol attribute tuple element type mismatch.
//!
//! When a class explicitly implements a `Protocol` and assigns to a
//! `self.attr` in `__init__` where `attr` is declared as `tuple[T1, T2, ...]`
//! in the protocol, each element of the assigned tuple must have a compatible
//! type.  If a parameter used in the tuple has a different type than the
//! corresponding element type in the protocol's annotation, Basilisk reports
//! the mismatch.
//!
//! ```python
//! from typing import Protocol
//!
//! class RGB(Protocol):
//!     rgb: tuple[int, int, int]
//!
//! class Point(RGB):
//!     def __init__(self, red: int, green: int, blue: str) -> None:
//!         self.rgb = red, green, blue  # E — 'blue' must be 'int'
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `protocols_subtyping` when a tuple assignment to a protocol attribute has
/// element types that don't match the protocol's declaration.
pub(crate) struct ProtocolTupleElementMismatch;

impl Rule for ProtocolTupleElementMismatch {
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
