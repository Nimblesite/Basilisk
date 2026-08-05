//! Implements [`protocols_explicit_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_explicit_2`: Calling `super().method()` on an abstract method with no default
//! implementation.
//!
//! When a Protocol (or ABC) declares a method as `@abstractmethod` with only an
//! ellipsis (`...`) or `pass` body, calling `super().method()` from a subclass
//! is invalid because there is no concrete implementation to delegate to.
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

/// Emits `protocols_explicit_2` when a subclass method calls `super().method()` on a method
/// that is abstract and has no default implementation (body is `...` or `pass`).
pub(crate) struct SuperAbstractCall;

impl Rule for SuperAbstractCall {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Detection deleted: the previous implementation recognised
        // `@abstractmethod` by comparing the decorator's source-text spelling
        // against a hardcoded string instead of resolving imports. Pending a
        // resolver-based reimplementation.
    }
}
