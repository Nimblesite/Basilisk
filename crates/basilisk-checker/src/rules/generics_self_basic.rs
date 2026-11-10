//! Implements [`generics_self_basic`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `generics_self_basic`: `Self` type violations in generics.
//!
//! This rule detects two kinds of `Self` type violations:
//!
//! 1. **Return type mismatch**: A method (or classmethod) annotated `-> Self`
//!    returns a concrete class constructor call (e.g. `return Shape()`) instead
//!    of `self`, `cls()`, or another `Self`-compatible expression. In a
//!    subclass, `Self` resolves to the subclass type, so returning the parent
//!    class constructor is a type error.
//!
//! 2. **`Self` is not subscriptable**: `Self` cannot be parameterized (e.g.
//!    `Self[int]`). It already captures the full generic specialization of the
//!    enclosing class.
//!
//! ```python
//! from typing import Self
//!
//! class Shape:
//!     def method2(self) -> Self:
//!         return Shape()  # E — should return self, not Shape()
//!
//!     @classmethod
//!     def cls_method2(cls) -> Self:
//!         return Shape()  # E — should return cls(), not Shape()
//!
//! class Container(Generic[T]):
//!     def foo(self, other: Self[int]) -> None:  # E — Self is not subscriptable
//!         pass
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
const CODE: ErrorCode = ErrorCode {
    code: "generics_self_basic",
    docs_url: "https://www.basilisk-python.dev/errors/generics_self_basic",
};

/// Emits `generics_self_basic` for `Self` return type mismatches and `Self` subscript usage.
pub(crate) struct SelfTypeViolation;

impl Rule for SelfTypeViolation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
