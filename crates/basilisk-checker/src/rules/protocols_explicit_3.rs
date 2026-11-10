//! Implements [`protocols_explicit_3`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_explicit_3`: `super()` call on abstract protocol method with no default implementation.
//!
//! When a class explicitly implements a `Protocol` and one of its methods
//! calls `super().method_name()`, the parent protocol method must provide a
//! default implementation.  If the parent method is abstract (its body is
//! only `...` or `pass`), calling `super()` on it is an error because there
//! is no concrete implementation to dispatch to.
//!
//! ```python
//! from typing import Protocol
//! from abc import abstractmethod
//!
//! class PColor(Protocol):
//!     @abstractmethod
//!     def draw(self) -> str:
//!         ...
//!
//! class BadColor(PColor):
//!     def draw(self) -> str:
//!         return super().draw()  # E — no default implementation
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Emits `protocols_explicit_3` when a method calls `super().method()` on an abstract
/// protocol method that has no default implementation.
pub(crate) struct SuperCallOnAbstractProtocolMethod;

impl Rule for SuperCallOnAbstractProtocolMethod {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Detection deleted: the previous implementation recognised `Protocol`
        // bases and `@abstractmethod` decorators by comparing source-text
        // spellings against hardcoded strings instead of resolving imports.
        // Pending a resolver-based reimplementation.
    }
}
