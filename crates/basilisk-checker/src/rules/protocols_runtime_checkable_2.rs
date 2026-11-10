//! Implements [`protocols_runtime_checkable_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_runtime_checkable_2`: Protocol `isinstance`/`issubclass` violations.
//!
//! Per PEP 544:
//! - A protocol can be used as the second argument to `isinstance()` or
//!   `issubclass()` **only** if it is decorated with `@runtime_checkable`.
//! - `issubclass()` can only be used with **non-data** protocols (protocols
//!   that define only methods, not data attributes).
//! - Type checkers should reject an `isinstance()` or `issubclass()` call if
//!   there is an unsafe overlap between the type of the first argument and
//!   the protocol.
//!
//! ```python
//! from typing import Protocol, runtime_checkable
//!
//! class Proto1(Protocol):
//!     name: str
//!
//! @runtime_checkable
//! class Proto2(Protocol):
//!     name: str
//!     def method(self) -> int: ...
//!
//! isinstance(x, Proto1)            # E — not @runtime_checkable
//! issubclass(x, Proto2)            # E — data protocol in issubclass
//! isinstance(Concrete(), Proto3)   # E — unsafe overlap
//! ```

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::Diagnostic;

/// Emits `protocols_runtime_checkable_2` for protocol `isinstance`/`issubclass` violations:
/// not-runtime-checkable, data protocol with issubclass, and unsafe overlap.
pub(crate) struct ProtocolUnsafeOverlap;

impl Rule for ProtocolUnsafeOverlap {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Detection deleted: the previous implementation recognised `Protocol`
        // bases and the `@runtime_checkable` decorator by comparing source-text
        // spellings against hardcoded strings instead of resolving imports.
        // Pending a resolver-based reimplementation.
    }
}
