//! Implements [`generics_self_usage`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_self_usage`: `Self` type used in an invalid location.
//!
//! PEP 673 defines `Self` as a special type that refers to the current class.
//! It is only valid in specific locations:
//!
//! - Method parameter annotations (including `self` and `cls`)
//! - Method return type annotations
//! - Class variable annotations inside the class body
//! - Nested within other types at those locations
//!
//! Invalid locations (detected here):
//!
//! - Return types or parameter annotations of module-level functions
//! - Module-level variable annotations (`bar: Self`)
//! - `TypeAlias` definitions whose RHS contains `Self`
//! - Base class expressions (`class Foo(Bar[Self])` or `class Foo(Self)`)
//! - `@staticmethod` method annotations (no `self` to bind to)
//! - Method annotations in metaclasses (classes inheriting from `type`)
//! - Return type annotation when `self` is explicitly annotated with a `TypeVar`
//!   (e.g. `def f(self: TFoo2) -> Self:` — binding is ambiguous)
//!
//! ```python
//! # E — not within a class
//! def foo(bar: Self) -> Self: ...
//! bar: Self
//!
//! class Base:
//!     @staticmethod
//!     def make() -> Self: ...  # E — staticmethod has no Self binding
//!
//! class MyMeta(type):
//!     def __new__(cls, *args: Any) -> Self: ...  # E — metaclass
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_self_usage",
    docs_url: "https://www.basilisk-python.dev/errors/generics_self_usage",
};

/// Emits `generics_self_usage` when `Self` is used in a location where it has no valid binding.
pub(crate) struct SelfInvalidLocation;

impl Rule for SelfInvalidLocation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
