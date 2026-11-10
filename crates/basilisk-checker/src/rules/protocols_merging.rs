//! Implements [`protocols_merging`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_merging`: Non-Protocol base class in a Protocol definition.
//!
//! Per PEP 544, a Protocol class may only inherit from other Protocol classes
//! (with the exception of `object`). Inheriting from a non-Protocol concrete
//! class is a violation.
//!
//! ```python
//! from typing import Protocol
//!
//! class Base:
//!     x: int = 0
//!
//! class BadProto(Base, Protocol):  # E — Base is not a Protocol
//!     def method(self) -> int: ...
//! ```

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::Diagnostic;

/// Emits `protocols_merging` when a Protocol class inherits from a non-Protocol base.
pub(crate) struct NonProtocolBaseInProtocol;

impl Rule for NonProtocolBaseInProtocol {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Detection deleted: the previous implementation recognised `Protocol`
        // bases and stdlib protocol classes by comparing source-text spellings
        // against hardcoded string arrays instead of resolving imports.
        // Pending a resolver-based reimplementation.
    }
}
